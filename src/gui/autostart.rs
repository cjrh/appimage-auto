//! Autostart helpers for managing XDG autostart entries.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

/// Get the path to the autostart desktop file.
fn autostart_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| {
        dirs.config_dir()
            .join("autostart/appimage-auto.desktop")
    })
}

/// Check if autostart is currently enabled.
pub fn is_autostart_enabled() -> bool {
    autostart_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Enable or disable autostart.
pub fn set_autostart(enabled: bool) -> io::Result<()> {
    let autostart_file = autostart_path()
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "No config directory found"))?;

    if enabled {
        // Create autostart directory if it doesn't exist
        if let Some(parent) = autostart_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the autostart desktop entry
        let desktop_content = include_str!("../../autostart/appimage-auto.desktop");
        fs::write(&autostart_file, desktop_content)?;
    } else if autostart_file.exists() {
        fs::remove_file(&autostart_file)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_path() {
        let path = autostart_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("autostart/appimage-auto.desktop"));
    }
}
