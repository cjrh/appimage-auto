//! AppImage Auto-Integration Library
//!
//! This library provides the core functionality for automatically integrating
//! AppImages into the Linux desktop environment.

pub mod appimage;
pub mod config;
pub mod daemon;
pub mod desktop;
pub mod state;
pub mod watcher;

pub use config::Config;
pub use daemon::Daemon;
pub use state::State;
