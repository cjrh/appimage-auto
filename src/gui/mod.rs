//! GTK4 Settings GUI for appimage-auto
//!
//! This module provides a graphical user interface for managing AppImage integrations,
//! watch directories, and daemon settings using Relm4 and libadwaita.

mod app;
mod app_list_page;
mod app_row;
mod autostart;
mod dialogs;
mod settings_page;
mod status_page;
mod watch_dir_row;

pub use app::AppModel;
