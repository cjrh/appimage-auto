//! AppImage Auto Settings GUI
//!
//! A GTK4/libadwaita graphical interface for managing AppImage integrations.

use appimage_auto::gui::AppModel;
use relm4::RelmApp;

fn main() {
    // Initialize Relm4 with libadwaita
    let app = RelmApp::new("io.github.appimage-auto.settings");
    app.run::<AppModel>(());
}
