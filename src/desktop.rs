//! Desktop entry file handling according to freedesktop.org specification.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum DesktopError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid desktop entry")]
    Invalid,
}

/// Represents a parsed .desktop file
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    /// All key-value pairs from the [Desktop Entry] section
    pub entries: HashMap<String, String>,
    /// Other sections (like [Desktop Action X])
    pub actions: HashMap<String, HashMap<String, String>>,
    /// Original file path (if loaded from file)
    pub source_path: Option<PathBuf>,
}

impl DesktopEntry {
    /// Parse a .desktop file
    pub fn parse(path: &Path) -> Result<Self, DesktopError> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);

        let mut entries = HashMap::new();
        let mut actions: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut current_section: Option<String> = None;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for section header
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = Some(trimmed[1..trimmed.len() - 1].to_string());
                continue;
            }

            // Parse key=value
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let value = trimmed[eq_pos + 1..].trim().to_string();

                match &current_section {
                    Some(section) if section == "Desktop Entry" => {
                        entries.insert(key, value);
                    }
                    Some(section) if section.starts_with("Desktop Action ") => {
                        let action_name =
                            section.strip_prefix("Desktop Action ").unwrap().to_string();
                        actions.entry(action_name).or_default().insert(key, value);
                    }
                    _ => {
                        // Ignore other sections
                    }
                }
            }
        }

        // Validate required fields
        if !entries.contains_key("Type") {
            return Err(DesktopError::MissingField("Type".to_string()));
        }
        if !entries.contains_key("Name") {
            return Err(DesktopError::MissingField("Name".to_string()));
        }

        Ok(Self {
            entries,
            actions,
            source_path: Some(path.to_path_buf()),
        })
    }

    /// Get the application name
    pub fn name(&self) -> Option<&str> {
        self.entries.get("Name").map(|s| s.as_str())
    }

    /// Get the Exec command
    pub fn exec(&self) -> Option<&str> {
        self.entries.get("Exec").map(|s| s.as_str())
    }

    /// Get the Icon name
    pub fn icon(&self) -> Option<&str> {
        self.entries.get("Icon").map(|s| s.as_str())
    }

    /// Get the entry Type
    pub fn entry_type(&self) -> Option<&str> {
        self.entries.get("Type").map(|s| s.as_str())
    }

    /// Set the Exec command to point to the AppImage
    pub fn set_exec(&mut self, appimage_path: &Path) {
        // Get the original Exec line to preserve any arguments
        let original_exec = self.entries.get("Exec").cloned().unwrap_or_default();

        // Extract any arguments after the original executable
        // The original might be something like "app %F" or "./app --flag %u"
        let args = extract_exec_args(&original_exec);

        // Build new Exec line
        let new_exec = if args.is_empty() {
            format!("\"{}\"", appimage_path.display())
        } else {
            format!("\"{}\" {}", appimage_path.display(), args)
        };

        self.entries.insert("Exec".to_string(), new_exec);
    }

    /// Set the Icon to a specific path or name
    pub fn set_icon(&mut self, icon: &str) {
        self.entries.insert("Icon".to_string(), icon.to_string());
    }

    /// Add a custom identifier for tracking
    pub fn set_appimage_identifier(&mut self, identifier: &str) {
        self.entries
            .insert("X-AppImage-Identifier".to_string(), identifier.to_string());
    }

    /// Get the AppImage identifier if present
    pub fn appimage_identifier(&self) -> Option<&str> {
        self.entries
            .get("X-AppImage-Identifier")
            .map(|s| s.as_str())
    }

    /// Add StartupWMClass if not present (helps with taskbar grouping)
    pub fn ensure_startup_wm_class(&mut self) {
        if !self.entries.contains_key("StartupWMClass")
            && let Some(name) = self.name()
        {
            // Use a sanitized version of the name
            let wm_class = name
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>();
            if !wm_class.is_empty() {
                self.entries.insert("StartupWMClass".to_string(), wm_class);
            }
        }
    }

    /// Update TryExec to point to the AppImage
    pub fn set_try_exec(&mut self, appimage_path: &Path) {
        self.entries
            .insert("TryExec".to_string(), appimage_path.display().to_string());
    }

    /// Update actions' Exec lines
    pub fn update_action_exec(&mut self, appimage_path: &Path) {
        for (_action_name, action_entries) in self.actions.iter_mut() {
            if let Some(original_exec) = action_entries.get("Exec").cloned() {
                let args = extract_exec_args(&original_exec);
                let new_exec = if args.is_empty() {
                    format!("\"{}\"", appimage_path.display())
                } else {
                    format!("\"{}\" {}", appimage_path.display(), args)
                };
                action_entries.insert("Exec".to_string(), new_exec);
            }
        }
    }

    /// Write the desktop entry to a file
    pub fn write(&self, path: &Path) -> Result<(), DesktopError> {
        let mut file = fs::File::create(path)?;

        // Write [Desktop Entry] section
        writeln!(file, "[Desktop Entry]")?;

        // Write entries in a somewhat consistent order
        let priority_keys = [
            "Type",
            "Name",
            "GenericName",
            "Comment",
            "Icon",
            "Exec",
            "TryExec",
            "Terminal",
            "Categories",
            "MimeType",
            "StartupWMClass",
            "StartupNotify",
            "Actions",
        ];

        // Write priority keys first
        for key in &priority_keys {
            if let Some(value) = self.entries.get(*key) {
                writeln!(file, "{}={}", key, value)?;
            }
        }

        // Write remaining keys
        for (key, value) in &self.entries {
            if !priority_keys.contains(&key.as_str()) {
                writeln!(file, "{}={}", key, value)?;
            }
        }

        // Write action sections
        for (action_name, action_entries) in &self.actions {
            writeln!(file)?;
            writeln!(file, "[Desktop Action {}]", action_name)?;
            for (key, value) in action_entries {
                writeln!(file, "{}={}", key, value)?;
            }
        }

        info!("Wrote desktop entry: {:?}", path);
        Ok(())
    }
}

/// Extract arguments from an Exec line, skipping the executable itself
fn extract_exec_args(exec: &str) -> String {
    let parts: Vec<&str> = exec.split_whitespace().collect();
    if parts.len() > 1 {
        parts[1..].join(" ")
    } else {
        String::new()
    }
}

/// Generate a desktop file name for an integrated AppImage
pub fn generate_desktop_filename(identifier: &str) -> String {
    format!("appimage-{}.desktop", identifier)
}

/// Install a desktop entry for an AppImage
pub fn install_desktop_entry(
    source_desktop: &Path,
    appimage_path: &Path,
    icon_path: Option<&Path>,
    identifier: &str,
    desktop_dir: &Path,
) -> Result<PathBuf, DesktopError> {
    // Parse the original desktop file
    let mut entry = DesktopEntry::parse(source_desktop)?;

    // Modify for our purposes
    entry.set_exec(appimage_path);
    entry.set_try_exec(appimage_path);
    entry.set_appimage_identifier(identifier);
    entry.ensure_startup_wm_class();
    entry.update_action_exec(appimage_path);

    // Set icon if provided
    if let Some(icon) = icon_path {
        // Use the icon name without path if it's in a standard location,
        // otherwise use full path
        let icon_str = if icon.starts_with("/usr/share/icons")
            || icon.to_string_lossy().contains("/.local/share/icons/")
        {
            // Extract just the icon name for theme lookup
            icon.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| icon.display().to_string())
        } else {
            icon.display().to_string()
        };
        entry.set_icon(&icon_str);
    }

    // Ensure desktop directory exists
    fs::create_dir_all(desktop_dir)?;

    // Write the desktop file
    let desktop_filename = generate_desktop_filename(identifier);
    let desktop_path = desktop_dir.join(&desktop_filename);
    entry.write(&desktop_path)?;

    Ok(desktop_path)
}

/// Remove a desktop entry
pub fn remove_desktop_entry(desktop_path: &Path) -> Result<(), DesktopError> {
    if desktop_path.exists() {
        fs::remove_file(desktop_path)?;
        info!("Removed desktop entry: {:?}", desktop_path);
    }
    Ok(())
}

/// Update the desktop database
pub fn update_desktop_database(desktop_dir: &Path) -> Result<(), DesktopError> {
    use std::process::Command;

    let output = Command::new("update-desktop-database")
        .arg(desktop_dir)
        .output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("update-desktop-database failed: {}", stderr);
            } else {
                debug!("Updated desktop database: {:?}", desktop_dir);
            }
        }
        Err(e) => {
            // Not fatal - the database will be updated eventually
            warn!("Could not run update-desktop-database: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exec_args() {
        assert_eq!(extract_exec_args("app"), "");
        assert_eq!(extract_exec_args("app %F"), "%F");
        assert_eq!(extract_exec_args("./app --flag %u"), "--flag %u");
        assert_eq!(extract_exec_args("/path/to/app arg1 arg2"), "arg1 arg2");
    }

    #[test]
    fn test_generate_desktop_filename() {
        let id = "abc123def456";
        let filename = generate_desktop_filename(id);
        assert_eq!(filename, "appimage-abc123def456.desktop");
    }
}
