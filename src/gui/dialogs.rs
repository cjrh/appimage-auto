//! File chooser dialogs for the GUI.

use relm4::gtk::glib;
use relm4::gtk::{self, gio, prelude::*};
use std::path::PathBuf;

/// Show a file chooser dialog for selecting an AppImage file.
pub fn show_appimage_chooser<F>(parent: &impl IsA<gtk::Window>, callback: F)
where
    F: Fn(PathBuf) + 'static,
{
    let dialog = gtk::FileChooserNative::builder()
        .title("Select AppImage")
        .modal(true)
        .transient_for(parent)
        .action(gtk::FileChooserAction::Open)
        .accept_label("Select")
        .cancel_label("Cancel")
        .build();

    // Create filter for AppImage files
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("AppImage Files"));
    filter.add_pattern("*.AppImage");
    filter.add_pattern("*.appimage");
    dialog.add_filter(&filter);
    dialog.set_filter(&filter);

    // Set initial folder to Downloads if it exists
    if let Some(downloads) = glib::user_special_dir(glib::UserDirectory::Downloads) {
        let file = gio::File::for_path(&downloads);
        let _ = dialog.set_current_folder(Some(&file));
    }

    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    callback(path);
                }
            }
        }
    });

    dialog.show();
}

/// Show a folder chooser dialog for selecting a watch directory.
pub fn show_directory_chooser<F>(parent: &impl IsA<gtk::Window>, callback: F)
where
    F: Fn(PathBuf) + 'static,
{
    let dialog = gtk::FileChooserNative::builder()
        .title("Select Watch Directory")
        .modal(true)
        .transient_for(parent)
        .action(gtk::FileChooserAction::SelectFolder)
        .accept_label("Select")
        .cancel_label("Cancel")
        .build();

    // Set initial folder to home directory
    let home = glib::home_dir();
    let file = gio::File::for_path(&home);
    let _ = dialog.set_current_folder(Some(&file));

    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    callback(path);
                }
            }
        }
    });

    dialog.show();
}
