//! File system watcher using inotify via the `notify` crate.

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info};

#[derive(Error, Debug)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("Channel receive error: {0}")]
    Receive(#[from] mpsc::RecvError),
    #[error("Directory does not exist: {0}")]
    DirectoryNotFound(PathBuf),
}

/// Events emitted by the file watcher
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A new file was created or moved into a watched directory
    Created(PathBuf),
    /// A file was deleted or moved out of a watched directory
    Deleted(PathBuf),
    /// A file was moved within watched directories
    Moved { from: PathBuf, to: PathBuf },
    /// A file was modified
    Modified(PathBuf),
}

/// File system watcher that monitors directories for changes
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event, notify::Error>>,
    watched_dirs: Vec<PathBuf>,
    /// Track rename events to match FROM and TO
    pending_renames: HashMap<u64, (PathBuf, std::time::Instant)>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new() -> Result<Self, WatcherError> {
        let (tx, rx) = mpsc::channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        Ok(Self {
            watcher,
            receiver: rx,
            watched_dirs: Vec::new(),
            pending_renames: HashMap::new(),
        })
    }

    /// Add a directory to watch
    pub fn watch(&mut self, path: &Path) -> Result<(), WatcherError> {
        if !path.exists() {
            return Err(WatcherError::DirectoryNotFound(path.to_path_buf()));
        }

        // Watch non-recursively - we only care about direct children
        self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        self.watched_dirs.push(path.to_path_buf());
        info!("Watching directory: {:?}", path);
        Ok(())
    }

    /// Remove a directory from watching
    pub fn unwatch(&mut self, path: &Path) -> Result<(), WatcherError> {
        self.watcher.unwatch(path)?;
        self.watched_dirs.retain(|p| p != path);
        info!("Stopped watching directory: {:?}", path);
        Ok(())
    }

    /// Check if a path is within any watched directory
    pub fn is_in_watched_dir(&self, path: &Path) -> bool {
        self.watched_dirs
            .iter()
            .any(|dir| path.parent() == Some(dir.as_path()))
    }

    /// Get the next file event (blocking)
    pub fn next_event(&mut self) -> Result<Option<FileEvent>, WatcherError> {
        // Clean up old pending renames (older than 1 second)
        let now = std::time::Instant::now();
        self.pending_renames
            .retain(|_, (_, time)| now.duration_since(*time) < Duration::from_secs(1));

        match self.receiver.recv()? {
            Ok(event) => Ok(self.process_event(event)),
            Err(e) => {
                error!("Watch error: {:?}", e);
                Ok(None)
            }
        }
    }

    /// Get the next file event with timeout
    pub fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<FileEvent>, WatcherError> {
        // Clean up old pending renames
        let now = std::time::Instant::now();
        self.pending_renames
            .retain(|_, (_, time)| now.duration_since(*time) < Duration::from_secs(1));

        match self.receiver.recv_timeout(timeout) {
            Ok(Ok(event)) => Ok(self.process_event(event)),
            Ok(Err(e)) => {
                error!("Watch error: {:?}", e);
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(WatcherError::Receive(mpsc::RecvError))
            }
        }
    }

    /// Process a raw notify event into our FileEvent type
    fn process_event(&mut self, event: Event) -> Option<FileEvent> {
        debug!("Raw event: {:?}", event);

        match event.kind {
            // File created
            EventKind::Create(CreateKind::File) => {
                if let Some(path) = event.paths.first()
                    && self.is_in_watched_dir(path)
                {
                    return Some(FileEvent::Created(path.clone()));
                }
            }

            // File removed
            EventKind::Remove(RemoveKind::File) => {
                if let Some(path) = event.paths.first()
                    && self.is_in_watched_dir(path)
                {
                    return Some(FileEvent::Deleted(path.clone()));
                }
            }

            // File renamed/moved - FROM part
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                if let Some(path) = event.paths.first() {
                    // Store this for potential matching with TO event
                    // The cookie helps match FROM and TO events
                    if let Some(tracker) = event.attrs.tracker() {
                        self.pending_renames
                            .insert(tracker as u64, (path.clone(), std::time::Instant::now()));
                    } else {
                        // No cookie, treat as deletion
                        if self.is_in_watched_dir(path) {
                            return Some(FileEvent::Deleted(path.clone()));
                        }
                    }
                }
            }

            // File renamed/moved - TO part
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                if let Some(to_path) = event.paths.first() {
                    // Check if we have a matching FROM event
                    if let Some(tracker) = event.attrs.tracker()
                        && let Some((from_path, _)) = self.pending_renames.remove(&(tracker as u64))
                    {
                        let from_watched = self.is_in_watched_dir(&from_path);
                        let to_watched = self.is_in_watched_dir(to_path);

                        match (from_watched, to_watched) {
                            (true, true) => {
                                // Moved within watched directories
                                return Some(FileEvent::Moved {
                                    from: from_path,
                                    to: to_path.clone(),
                                });
                            }
                            (true, false) => {
                                // Moved out of watched directory
                                return Some(FileEvent::Deleted(from_path));
                            }
                            (false, true) => {
                                // Moved into watched directory
                                return Some(FileEvent::Created(to_path.clone()));
                            }
                            (false, false) => {
                                // Neither in watched dirs, ignore
                            }
                        }
                    }

                    // No matching FROM, treat as creation if in watched dir
                    if self.is_in_watched_dir(to_path) {
                        return Some(FileEvent::Created(to_path.clone()));
                    }
                }
            }

            // Both FROM and TO in single event
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                if event.paths.len() >= 2 {
                    let from_path = &event.paths[0];
                    let to_path = &event.paths[1];

                    let from_watched = self.is_in_watched_dir(from_path);
                    let to_watched = self.is_in_watched_dir(to_path);

                    match (from_watched, to_watched) {
                        (true, true) => {
                            return Some(FileEvent::Moved {
                                from: from_path.clone(),
                                to: to_path.clone(),
                            });
                        }
                        (true, false) => {
                            return Some(FileEvent::Deleted(from_path.clone()));
                        }
                        (false, true) => {
                            return Some(FileEvent::Created(to_path.clone()));
                        }
                        (false, false) => {}
                    }
                }
            }

            // File modified (content changed)
            EventKind::Modify(ModifyKind::Data(_)) => {
                if let Some(path) = event.paths.first()
                    && self.is_in_watched_dir(path)
                {
                    return Some(FileEvent::Modified(path.clone()));
                }
            }

            // Catch-all for other create events (e.g., CreateKind::Any)
            EventKind::Create(_) => {
                if let Some(path) = event.paths.first()
                    && self.is_in_watched_dir(path)
                    && path.is_file()
                {
                    return Some(FileEvent::Created(path.clone()));
                }
            }

            // Catch-all for other remove events
            EventKind::Remove(_) => {
                if let Some(path) = event.paths.first()
                    && self.is_in_watched_dir(path)
                {
                    return Some(FileEvent::Deleted(path.clone()));
                }
            }

            _ => {
                debug!("Ignoring event kind: {:?}", event.kind);
            }
        }

        None
    }

    /// Get list of watched directories
    pub fn watched_directories(&self) -> &[PathBuf] {
        &self.watched_dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileWatcher::new();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = FileWatcher::new().unwrap();

        let result = watcher.watch(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(watcher.watched_directories().len(), 1);
    }

    #[test]
    fn test_watch_nonexistent_directory() {
        let mut watcher = FileWatcher::new().unwrap();
        let result = watcher.watch(Path::new("/nonexistent/path/12345"));
        assert!(matches!(result, Err(WatcherError::DirectoryNotFound(_))));
    }

    #[test]
    fn test_is_in_watched_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = FileWatcher::new().unwrap();
        watcher.watch(temp_dir.path()).unwrap();

        // File directly in watched dir should return true
        let file_in_dir = temp_dir.path().join("test.AppImage");
        assert!(watcher.is_in_watched_dir(&file_in_dir));

        // File in subdirectory should return false (we watch non-recursively)
        let subdir = temp_dir.path().join("subdir");
        let file_in_subdir = subdir.join("test.AppImage");
        assert!(!watcher.is_in_watched_dir(&file_in_subdir));

        // File in different directory should return false
        assert!(!watcher.is_in_watched_dir(Path::new("/tmp/other/test.AppImage")));
    }
}
