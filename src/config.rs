//! Configuration file parsing for appimage-auto daemon.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
    #[error("No config directory found")]
    NoConfigDir,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub watch: WatchConfig,
    pub integration: IntegrationConfig,
    pub logging: LoggingConfig,
}

/// Watch directory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    /// Directories to watch for AppImages
    pub directories: Vec<String>,
    /// File patterns to match (in addition to magic byte check)
    pub patterns: Vec<String>,
    /// Debounce delay in milliseconds
    pub debounce_ms: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            directories: vec![
                "~/Downloads".to_string(),
                "~/Applications".to_string(),
                "~/.local/bin".to_string(),
            ],
            patterns: vec!["*.AppImage".to_string(), "*.appimage".to_string()],
            debounce_ms: 1000,
        }
    }
}

/// Integration behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationConfig {
    /// Directory for .desktop files
    pub desktop_dir: String,
    /// Directory for icons
    pub icon_dir: String,
    /// Whether to run update-desktop-database after changes
    pub update_database: bool,
    /// Whether to scan existing AppImages on startup
    pub scan_on_startup: bool,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            desktop_dir: "~/.local/share/applications".to_string(),
            icon_dir: "~/.local/share/icons/hicolor".to_string(),
            update_database: true,
            scan_on_startup: true,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Whether to log to file
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
        }
    }
}

impl Config {
    /// Load configuration from the default location or create default if not exists
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            Self::load_from(&config_path)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// Get the default config file path
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let dirs = directories::ProjectDirs::from("", "", "appimage-auto")
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    /// Expand all paths in the configuration (resolve ~ and environment variables)
    pub fn expand_paths(&self) -> Self {
        let mut config = self.clone();

        config.watch.directories = config
            .watch
            .directories
            .iter()
            .map(|d| shellexpand::tilde(d).to_string())
            .collect();

        config.integration.desktop_dir =
            shellexpand::tilde(&config.integration.desktop_dir).to_string();
        config.integration.icon_dir = shellexpand::tilde(&config.integration.icon_dir).to_string();

        if let Some(ref file) = config.logging.file {
            config.logging.file = Some(shellexpand::tilde(file).to_string());
        }

        config
    }

    /// Get expanded watch directories as PathBufs
    pub fn watch_directories(&self) -> Vec<PathBuf> {
        self.watch
            .directories
            .iter()
            .map(|d| PathBuf::from(shellexpand::tilde(d).as_ref()))
            .collect()
    }

    /// Get expanded desktop directory
    pub fn desktop_directory(&self) -> PathBuf {
        PathBuf::from(shellexpand::tilde(&self.integration.desktop_dir).as_ref())
    }

    /// Get expanded icon directory
    pub fn icon_directory(&self) -> PathBuf {
        PathBuf::from(shellexpand::tilde(&self.integration.icon_dir).as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.watch.directories.len(), 3);
        assert!(
            config
                .watch
                .directories
                .contains(&"~/Downloads".to_string())
        );
    }

    #[test]
    fn test_expand_paths() {
        let config = Config::default();
        let expanded = config.expand_paths();

        // Should not contain ~ after expansion
        for dir in &expanded.watch.directories {
            assert!(!dir.starts_with("~"));
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(config.watch.directories, deserialized.watch.directories);
    }
}
