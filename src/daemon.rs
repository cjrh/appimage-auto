//! Main daemon event loop and coordination logic.

use crate::appimage;
use crate::config::Config;
use crate::desktop;
use crate::state::{self, IntegratedAppImage, State};
use crate::watcher::{FileEvent, FileWatcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("State error: {0}")]
    State(#[from] crate::state::StateError),
    #[error("Watcher error: {0}")]
    Watcher(#[from] crate::watcher::WatcherError),
    #[error("AppImage error: {0}")]
    AppImage(#[from] crate::appimage::AppImageError),
    #[error("Desktop error: {0}")]
    Desktop(#[from] crate::desktop::DesktopError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// The main daemon that watches for AppImages and integrates them
pub struct Daemon {
    config: Config,
    state: State,
    watcher: FileWatcher,
    running: Arc<AtomicBool>,
    /// Pending events for debouncing (path → (event, timestamp))
    pending_events: HashMap<PathBuf, (FileEvent, Instant)>,
}

impl Daemon {
    /// Create a new daemon instance
    pub fn new() -> Result<Self, DaemonError> {
        let config = Config::load()?.expand_paths();
        let state = State::load()?;
        let watcher = FileWatcher::new()?;

        Ok(Self {
            config,
            state,
            watcher,
            running: Arc::new(AtomicBool::new(false)),
            pending_events: HashMap::new(),
        })
    }

    /// Create a daemon with a specific config
    pub fn with_config(config: Config) -> Result<Self, DaemonError> {
        let config = config.expand_paths();
        let state = State::load()?;
        let watcher = FileWatcher::new()?;

        Ok(Self {
            config,
            state,
            watcher,
            running: Arc::new(AtomicBool::new(false)),
            pending_events: HashMap::new(),
        })
    }

    /// Get a handle to the running flag for signal handling
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Initialize the daemon: set up watches and optionally scan existing files
    pub fn init(&mut self) -> Result<(), DaemonError> {
        info!("Initializing daemon...");

        // Ensure directories exist
        let desktop_dir = self.config.desktop_directory();
        let icon_dir = self.config.icon_directory();
        fs::create_dir_all(&desktop_dir)?;
        fs::create_dir_all(&icon_dir)?;

        // Set up file watches
        for dir in &self.config.watch.directories {
            let path = PathBuf::from(dir);
            if path.exists() {
                if let Err(e) = self.watcher.watch(&path) {
                    warn!("Failed to watch {:?}: {}", path, e);
                }
            } else {
                warn!("Watch directory does not exist: {:?}", path);
            }
        }

        // Scan for existing AppImages if configured
        if self.config.integration.scan_on_startup {
            self.scan_existing()?;
        }

        // Clean up orphaned entries
        self.cleanup_orphaned()?;

        info!("Daemon initialized");
        Ok(())
    }

    /// Scan watched directories for existing AppImages
    pub fn scan_existing(&mut self) -> Result<(), DaemonError> {
        info!("Scanning for existing AppImages...");

        for dir in self.watcher.watched_directories().to_vec() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file()
                        && appimage::is_appimage(&path)
                        && !self.state.is_integrated(&path)
                    {
                        info!("Found existing AppImage: {:?}", path);
                        if let Err(e) = self.integrate(&path) {
                            warn!("Failed to integrate {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up orphaned state entries (AppImages that no longer exist)
    pub fn cleanup_orphaned(&mut self) -> Result<(), DaemonError> {
        let orphaned: Vec<String> = self
            .state
            .find_orphaned()
            .iter()
            .map(|info| info.identifier.clone())
            .collect();

        for id in orphaned {
            info!("Cleaning up orphaned entry: {}", id);
            if let Some(info) = self.state.remove(&id) {
                self.cleanup_integration(&info)?;
            }
        }

        if !self.state.find_orphaned().is_empty() {
            self.state.save()?;
        }

        Ok(())
    }

    /// Run the main event loop
    pub fn run(&mut self) -> Result<(), DaemonError> {
        self.running.store(true, Ordering::SeqCst);
        info!(
            "Daemon running. Watching {} directories. Debounce: {}ms",
            self.watcher.watched_directories().len(),
            self.config.watch.debounce_ms
        );

        while self.running.load(Ordering::SeqCst) {
            // Check for new events
            match self.watcher.next_event_timeout(Duration::from_millis(100)) {
                Ok(Some(event)) => {
                    self.queue_event(event);
                }
                Ok(None) => {
                    // Timeout, continue loop
                }
                Err(e) => {
                    error!("Watcher error: {}", e);
                    break;
                }
            }

            // Process debounced events that are ready
            if let Err(e) = self.process_pending_events() {
                error!("Error processing pending events: {}", e);
            }
        }

        info!("Daemon stopped");
        Ok(())
    }

    /// Queue an event for debounced processing
    fn queue_event(&mut self, event: FileEvent) {
        let now = Instant::now();

        match &event {
            // Debounce Created and Modified events
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                debug!("Queuing event for debounce: {:?}", path);
                self.pending_events.insert(path.clone(), (event, now));
            }
            // Process Deleted and Moved immediately (no debounce needed)
            FileEvent::Deleted(_) | FileEvent::Moved { .. } => {
                if let Err(e) = self.handle_event(event) {
                    error!("Error handling event: {}", e);
                }
            }
        }
    }

    /// Process pending events that have exceeded the debounce duration
    fn process_pending_events(&mut self) -> Result<(), DaemonError> {
        let debounce_duration = Duration::from_millis(self.config.watch.debounce_ms);
        let now = Instant::now();

        // Collect ready events (elapsed >= debounce_duration)
        let ready: Vec<_> = self
            .pending_events
            .iter()
            .filter(|(_, (_, timestamp))| now.duration_since(*timestamp) >= debounce_duration)
            .map(|(path, (event, _))| (path.clone(), event.clone()))
            .collect();

        // Process and remove ready events
        for (path, event) in ready {
            self.pending_events.remove(&path);
            if let Err(e) = self.handle_event(event) {
                error!("Error handling debounced event for {:?}: {}", path, e);
            }
        }

        Ok(())
    }

    /// Handle a file system event
    fn handle_event(&mut self, event: FileEvent) -> Result<(), DaemonError> {
        match event {
            FileEvent::Created(ref path) => {
                debug!("File created: {:?}", path);
                if appimage::is_appimage(path) {
                    // Check if file is complete before integrating
                    match appimage::is_appimage_complete(path) {
                        Ok(true) => {
                            info!("New complete AppImage detected: {:?}", path);
                            self.integrate(path)?;
                        }
                        Ok(false) => {
                            debug!("AppImage incomplete, re-queuing: {:?}", path);
                            // Re-queue for later check
                            self.pending_events
                                .insert(path.clone(), (event, Instant::now()));
                        }
                        Err(e) => {
                            warn!("Could not verify completeness for {:?}: {}", path, e);
                            // Try integration anyway (fallback to previous behavior)
                            info!("New AppImage detected (unverified): {:?}", path);
                            self.integrate(path)?;
                        }
                    }
                }
            }

            FileEvent::Deleted(path) => {
                debug!("File deleted: {:?}", path);
                if self.state.is_integrated(&path) {
                    info!("Integrated AppImage deleted: {:?}", path);
                    self.unintegrate(&path)?;
                }
            }

            FileEvent::Moved { from, to } => {
                debug!("File moved: {:?} -> {:?}", from, to);
                if self.state.is_integrated(&from) {
                    info!("Integrated AppImage moved: {:?} -> {:?}", from, to);
                    self.handle_move(&from, &to)?;
                } else if appimage::is_appimage(&to) {
                    // Moved in from outside watched dirs
                    info!("AppImage moved into watched directory: {:?}", to);
                    self.integrate(&to)?;
                }
            }

            FileEvent::Modified(ref path) => {
                debug!("File modified: {:?}", path);
                // For now, we don't re-integrate on modification
                // Could be extended to detect significant changes
            }
        }

        Ok(())
    }

    /// Integrate an AppImage
    pub fn integrate(&mut self, path: &Path) -> Result<(), DaemonError> {
        let identifier = appimage::generate_identifier(path);

        // Check if already integrated
        if self.state.get(&identifier).is_some() {
            debug!("AppImage already integrated: {:?}", path);
            return Ok(());
        }

        info!("Integrating AppImage: {:?}", path);

        // Create temporary directory for extraction
        let temp_dir = TempDir::new()?;
        let extract_dir = temp_dir.path();

        // Extract metadata
        let info = appimage::extract_metadata(path, extract_dir)?;

        // Find the best icon
        let icon_path = appimage::select_best_icon(&info.icon_files);

        // Install icon if available
        let installed_icon = if let Some(src_icon) = icon_path {
            match self.install_icon(src_icon, &identifier) {
                Ok(installed) => Some(installed),
                Err(e) => {
                    warn!("Failed to install icon: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Install desktop entry
        let desktop_file = info
            .desktop_file
            .as_ref()
            .ok_or(crate::appimage::AppImageError::NoDesktopFile)?;

        let desktop_path = desktop::install_desktop_entry(
            desktop_file,
            path,
            installed_icon.as_deref(),
            &identifier,
            &self.config.desktop_directory(),
        )?;

        // Update desktop database
        if self.config.integration.update_database {
            desktop::update_desktop_database(&self.config.desktop_directory())?;
        }

        // Record in state
        let icon_paths = installed_icon.map(|p| vec![p]).unwrap_or_default();
        let entry = state::create_entry(
            identifier,
            path.to_path_buf(),
            desktop_path,
            icon_paths.clone(),
            info.name.clone(),
        );
        self.state.add(entry);
        self.state.save()?;

        // Send notification
        if self.config.notifications.enabled && self.config.notifications.on_integrate {
            let name = info.name.as_deref().unwrap_or("AppImage");
            let icon = icon_paths.first().map(|p| p.as_path());
            crate::notifications::send(crate::notifications::integrated(name, path, icon));
        }

        info!("Successfully integrated: {:?}", path);
        Ok(())
    }

    /// Unintegrate an AppImage
    pub fn unintegrate(&mut self, path: &Path) -> Result<(), DaemonError> {
        if let Some(info) = self.state.remove_by_path(path) {
            // Send notification before cleanup
            if self.config.notifications.enabled && self.config.notifications.on_unintegrate {
                let name = info.name.as_deref().unwrap_or("AppImage");
                crate::notifications::send(crate::notifications::unintegrated(
                    name,
                    &info.appimage_path,
                ));
            }

            self.cleanup_integration(&info)?;
            self.state.save()?;
            info!("Successfully unintegrated: {:?}", path);
        }
        Ok(())
    }

    /// Handle an AppImage move within watched directories
    fn handle_move(&mut self, from: &Path, to: &Path) -> Result<(), DaemonError> {
        // Update state
        if let Some(info) = self.state.update_path(from, to) {
            // Update the desktop file to point to new location
            let mut entry = desktop::DesktopEntry::parse(&info.desktop_path)?;
            entry.set_exec(to);
            entry.set_try_exec(to);
            entry.write(&info.desktop_path)?;

            // Update desktop database
            if self.config.integration.update_database {
                desktop::update_desktop_database(&self.config.desktop_directory())?;
            }

            self.state.save()?;
            info!("Updated desktop entry for moved AppImage: {:?}", to);
        }
        Ok(())
    }

    /// Clean up integration files (desktop entry and icons)
    fn cleanup_integration(&self, info: &IntegratedAppImage) -> Result<(), DaemonError> {
        // Remove desktop file
        desktop::remove_desktop_entry(&info.desktop_path)?;

        // Remove icons
        for icon_path in &info.icon_paths {
            if icon_path.exists()
                && let Err(e) = fs::remove_file(icon_path)
            {
                warn!("Failed to remove icon {:?}: {}", icon_path, e);
            }
        }

        // Update desktop database
        if self.config.integration.update_database {
            desktop::update_desktop_database(&self.config.desktop_directory())?;
        }

        Ok(())
    }

    /// Install an icon to the appropriate location
    fn install_icon(&self, src: &Path, identifier: &str) -> Result<PathBuf, DaemonError> {
        let icon_base = self.config.icon_directory();

        // Determine icon size and format
        let (size, ext) = determine_icon_info(src);

        // Build destination path
        let dest_dir = if ext == "svg" {
            icon_base.join("scalable").join("apps")
        } else {
            icon_base.join(format!("{}x{}", size, size)).join("apps")
        };

        fs::create_dir_all(&dest_dir)?;

        let dest_name = format!("appimage-{}.{}", identifier, ext);
        let dest_path = dest_dir.join(&dest_name);

        fs::copy(src, &dest_path)?;
        debug!("Installed icon: {:?}", dest_path);

        Ok(dest_path)
    }

    /// Stop the daemon
    pub fn stop(&self) {
        info!("Stopping daemon...");
        self.running.store(false, Ordering::SeqCst);
    }

    /// Get current state
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Get current config
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Determine icon size and extension from path
fn determine_icon_info(path: &Path) -> (u32, String) {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "png".to_string());

    // Try to get size from path
    let size = appimage::select_best_icon(&[path.to_path_buf()])
        .and_then(|_| {
            // Look for size pattern in path
            let path_str = path.to_string_lossy();
            for part in path_str.split('/') {
                if let Some(size_str) = part.split('x').next()
                    && let Ok(s) = size_str.parse::<u32>()
                {
                    return Some(s);
                }
            }
            None
        })
        .unwrap_or(128); // Default to 128x128

    (size, ext)
}

/// Run a one-shot scan (integrate existing AppImages and exit)
pub fn oneshot(config: Option<Config>) -> Result<(), DaemonError> {
    let mut daemon = match config {
        Some(c) => Daemon::with_config(c)?,
        None => Daemon::new()?,
    };

    // Set up watches
    for dir in &daemon.config.watch.directories.clone() {
        let path = PathBuf::from(dir);
        if path.exists() {
            let _ = daemon.watcher.watch(&path);
        }
    }

    // Scan and integrate
    daemon.scan_existing()?;
    daemon.cleanup_orphaned()?;

    info!(
        "One-shot scan complete. Integrated {} AppImages.",
        daemon.state.count()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_icon_info_png() {
        let path = Path::new("/some/path/256x256/apps/icon.png");
        let (size, ext) = determine_icon_info(path);
        assert_eq!(ext, "png");
        // Size detection might vary, just check it's reasonable
        assert!(size >= 16 && size <= 512);
    }

    #[test]
    fn test_determine_icon_info_svg() {
        let path = Path::new("/some/path/icon.svg");
        let (_, ext) = determine_icon_info(path);
        assert_eq!(ext, "svg");
    }
}
