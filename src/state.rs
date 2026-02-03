//! State management for tracking integrated AppImages.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No data directory found")]
    NoDataDir,
}

/// Information about an integrated AppImage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedAppImage {
    /// Unique identifier (MD5 hash of original path)
    pub identifier: String,
    /// Current path to the AppImage file
    pub appimage_path: PathBuf,
    /// Path to the installed .desktop file
    pub desktop_path: PathBuf,
    /// Paths to installed icon files
    pub icon_paths: Vec<PathBuf>,
    /// Application name from .desktop file
    pub name: Option<String>,
    /// When the AppImage was integrated
    pub integrated_at: u64,
    /// When the entry was last updated
    pub updated_at: u64,
}

/// State storage for the daemon
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    /// Map from identifier to integrated AppImage info
    pub integrated: HashMap<String, IntegratedAppImage>,
    /// Map from AppImage path to identifier (for quick lookup)
    #[serde(skip)]
    path_index: HashMap<PathBuf, String>,
}

impl State {
    /// Load state from the default location
    pub fn load() -> Result<Self, StateError> {
        let state_path = Self::state_path()?;

        if state_path.exists() {
            Self::load_from(&state_path)
        } else {
            Ok(State::default())
        }
    }

    /// Load state from a specific path
    pub fn load_from(path: &Path) -> Result<Self, StateError> {
        let content = fs::read_to_string(path)?;
        let mut state: State = serde_json::from_str(&content)?;
        state.rebuild_index();
        Ok(state)
    }

    /// Save state to the default location
    pub fn save(&self) -> Result<(), StateError> {
        let state_path = Self::state_path()?;

        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&state_path, content)?;
        debug!("Saved state to {:?}", state_path);
        Ok(())
    }

    /// Get the default state file path
    pub fn state_path() -> Result<PathBuf, StateError> {
        let dirs =
            directories::ProjectDirs::from("", "", "appimage-auto").ok_or(StateError::NoDataDir)?;
        Ok(dirs.data_dir().join("state.json"))
    }

    /// Rebuild the path index from the integrated map
    fn rebuild_index(&mut self) {
        self.path_index.clear();
        for (id, info) in &self.integrated {
            self.path_index
                .insert(info.appimage_path.clone(), id.clone());
        }
    }

    /// Add or update an integrated AppImage
    pub fn add(&mut self, info: IntegratedAppImage) {
        let id = info.identifier.clone();
        let path = info.appimage_path.clone();

        self.integrated.insert(id.clone(), info);
        self.path_index.insert(path, id);
    }

    /// Remove an integrated AppImage by identifier
    pub fn remove(&mut self, identifier: &str) -> Option<IntegratedAppImage> {
        if let Some(info) = self.integrated.remove(identifier) {
            self.path_index.remove(&info.appimage_path);
            Some(info)
        } else {
            None
        }
    }

    /// Remove an integrated AppImage by path
    pub fn remove_by_path(&mut self, path: &Path) -> Option<IntegratedAppImage> {
        if let Some(id) = self.path_index.remove(path) {
            self.integrated.remove(&id)
        } else {
            None
        }
    }

    /// Get an integrated AppImage by identifier
    pub fn get(&self, identifier: &str) -> Option<&IntegratedAppImage> {
        self.integrated.get(identifier)
    }

    /// Get an integrated AppImage by path
    pub fn get_by_path(&self, path: &Path) -> Option<&IntegratedAppImage> {
        self.path_index
            .get(path)
            .and_then(|id| self.integrated.get(id))
    }

    /// Check if a path is integrated
    pub fn is_integrated(&self, path: &Path) -> bool {
        self.path_index.contains_key(path)
    }

    /// Update the path of an integrated AppImage (for move handling)
    pub fn update_path(&mut self, old_path: &Path, new_path: &Path) -> Option<&IntegratedAppImage> {
        if let Some(id) = self.path_index.remove(old_path)
            && let Some(info) = self.integrated.get_mut(&id)
        {
            info.appimage_path = new_path.to_path_buf();
            info.updated_at = current_timestamp();
            self.path_index.insert(new_path.to_path_buf(), id.clone());
            return self.integrated.get(&id);
        }
        None
    }

    /// Get all integrated AppImages
    pub fn all(&self) -> impl Iterator<Item = &IntegratedAppImage> {
        self.integrated.values()
    }

    /// Get the number of integrated AppImages
    pub fn count(&self) -> usize {
        self.integrated.len()
    }

    /// Find AppImages that no longer exist on disk
    pub fn find_orphaned(&self) -> Vec<&IntegratedAppImage> {
        self.integrated
            .values()
            .filter(|info| !info.appimage_path.exists())
            .collect()
    }

    /// Find AppImages in a specific directory
    pub fn find_in_directory(&self, dir: &Path) -> Vec<&IntegratedAppImage> {
        self.integrated
            .values()
            .filter(|info| info.appimage_path.starts_with(dir))
            .collect()
    }
}

/// Get the current Unix timestamp
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Create a new IntegratedAppImage entry
pub fn create_entry(
    identifier: String,
    appimage_path: PathBuf,
    desktop_path: PathBuf,
    icon_paths: Vec<PathBuf>,
    name: Option<String>,
) -> IntegratedAppImage {
    let now = current_timestamp();
    IntegratedAppImage {
        identifier,
        appimage_path,
        desktop_path,
        icon_paths,
        name,
        integrated_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_add_remove() {
        let mut state = State::default();

        let entry = create_entry(
            "test123".to_string(),
            PathBuf::from("/home/user/test.AppImage"),
            PathBuf::from("/home/user/.local/share/applications/appimage-test123.desktop"),
            vec![],
            Some("Test App".to_string()),
        );

        state.add(entry);
        assert_eq!(state.count(), 1);
        assert!(state.is_integrated(Path::new("/home/user/test.AppImage")));

        let removed = state.remove("test123");
        assert!(removed.is_some());
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn test_state_update_path() {
        let mut state = State::default();

        let entry = create_entry(
            "test123".to_string(),
            PathBuf::from("/home/user/Downloads/test.AppImage"),
            PathBuf::from("/home/user/.local/share/applications/appimage-test123.desktop"),
            vec![],
            None,
        );

        state.add(entry);

        state.update_path(
            Path::new("/home/user/Downloads/test.AppImage"),
            Path::new("/home/user/Applications/test.AppImage"),
        );

        assert!(!state.is_integrated(Path::new("/home/user/Downloads/test.AppImage")));
        assert!(state.is_integrated(Path::new("/home/user/Applications/test.AppImage")));
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut state = State::default();

        let entry = create_entry(
            "test123".to_string(),
            PathBuf::from("/home/user/test.AppImage"),
            PathBuf::from("/home/user/.local/share/applications/appimage-test123.desktop"),
            vec![PathBuf::from("/home/user/.local/share/icons/test.png")],
            Some("Test App".to_string()),
        );

        state.add(entry);

        let json = serde_json::to_string(&state).unwrap();
        let mut loaded: State = serde_json::from_str(&json).unwrap();
        loaded.rebuild_index();

        assert_eq!(loaded.count(), 1);
        assert!(loaded.is_integrated(Path::new("/home/user/test.AppImage")));
    }
}
