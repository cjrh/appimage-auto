//! Main application component for the GUI.

use super::app_list_page::{AppListPage, AppListPageMsg, AppListPageOutput};
use super::dialogs;
use super::settings_page::{SettingsPage, SettingsPageMsg, SettingsPageOutput};
use super::status_page::{StatusPage, StatusPageMsg, StatusPageOutput};
use relm4::adw::prelude::*;
use relm4::gtk::{self, gio};
use relm4::prelude::*;
use relm4::{adw, ComponentController, ComponentParts, ComponentSender, Controller};
use std::path::PathBuf;
use std::process::Command;

/// The main application model.
pub struct AppModel {
    /// Status page component.
    status_page: Controller<StatusPage>,
    /// App list page component.
    app_list_page: Controller<AppListPage>,
    /// Settings page component.
    settings_page: Controller<SettingsPage>,
}

/// Messages for the main application.
#[derive(Debug)]
pub enum AppMsg {
    /// Navigate to a page by tag.
    NavigateTo(String),
    /// Show a toast message.
    ShowToast(String),
    /// Integrate a new AppImage via file chooser.
    IntegrateAppImage,
    /// Handle AppImage file selected.
    AppImageSelected(PathBuf),
    /// Refresh all pages.
    RefreshAll,
    /// Show directory chooser for settings.
    ShowDirectoryChooser,
    /// Directory selected for settings.
    DirectorySelected(PathBuf),
    /// Handle status page output.
    StatusPageOutput(StatusPageOutput),
    /// Handle app list page output.
    AppListPageOutput(AppListPageOutput),
    /// Handle settings page output.
    SettingsPageOutput(SettingsPageOutput),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("AppImage Auto Settings"),
            set_default_width: 700,
            set_default_height: 500,

            #[name(toast_overlay)]
            adw::ToastOverlay {
                #[name(view_stack)]
                adw::ViewStack {}
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Create child components
        let status_page = StatusPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::StatusPageOutput);

        let app_list_page = AppListPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::AppListPageOutput);

        let settings_page = SettingsPage::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::SettingsPageOutput);

        let model = Self {
            status_page,
            app_list_page,
            settings_page,
        };

        let widgets = view_output!();

        // Add pages to the view stack
        let status_page_widget = model.status_page.widget().clone();
        let apps_page_widget = model.app_list_page.widget().clone();
        let settings_page_widget = model.settings_page.widget().clone();

        let status_stack_page = widgets.view_stack.add_titled(&status_page_widget, Some("status"), "Overview");
        status_stack_page.set_icon_name(Some("go-home-symbolic"));

        let apps_stack_page = widgets.view_stack.add_titled(&apps_page_widget, Some("apps"), "Apps");
        apps_stack_page.set_icon_name(Some("application-x-executable-symbolic"));

        let settings_stack_page = widgets.view_stack.add_titled(&settings_page_widget, Some("settings"), "Settings");
        settings_stack_page.set_icon_name(Some("emblem-system-symbolic"));

        // Set up actions
        let app = relm4::main_adw_application();

        // Integrate action
        let sender_clone = sender.clone();
        let integrate_action = gio::ActionEntry::builder("integrate")
            .activate(move |_, _, _| {
                sender_clone.input(AppMsg::IntegrateAppImage);
            })
            .build();

        // Refresh action
        let sender_clone = sender.clone();
        let refresh_action = gio::ActionEntry::builder("refresh")
            .activate(move |_, _, _| {
                sender_clone.input(AppMsg::RefreshAll);
            })
            .build();

        // About action
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |_, _, _| {
                show_about_dialog();
            })
            .build();

        app.add_action_entries([integrate_action, refresh_action, about_action]);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppMsg::NavigateTo(page) => {
                match page.as_str() {
                    "status" => {
                        self.status_page.emit(StatusPageMsg::Refresh);
                    }
                    "apps" => {
                        self.app_list_page.emit(AppListPageMsg::Reload);
                    }
                    "settings" => {
                        self.settings_page.emit(SettingsPageMsg::Reload);
                    }
                    _ => {}
                }
            }
            AppMsg::ShowToast(message) => {
                eprintln!("Toast: {}", message);
            }
            AppMsg::IntegrateAppImage => {
                let app = relm4::main_adw_application();
                if let Some(window) = app.active_window() {
                    let sender_clone = sender.input_sender().clone();
                    dialogs::show_appimage_chooser(&window, move |path| {
                        sender_clone.emit(AppMsg::AppImageSelected(path));
                    });
                }
            }
            AppMsg::AppImageSelected(path) => {
                let path_str = path.to_string_lossy().to_string();
                match Command::new("appimage-auto")
                    .args(["integrate", &path_str])
                    .spawn()
                {
                    Ok(mut child) => {
                        let _ = child.wait();
                        sender.input(AppMsg::ShowToast("AppImage integrated".to_string()));
                        self.app_list_page.emit(AppListPageMsg::Reload);
                        self.status_page.emit(StatusPageMsg::Refresh);
                    }
                    Err(e) => {
                        sender.input(AppMsg::ShowToast(format!("Failed to integrate: {}", e)));
                    }
                }
            }
            AppMsg::RefreshAll => {
                self.status_page.emit(StatusPageMsg::Refresh);
                self.app_list_page.emit(AppListPageMsg::Reload);
                self.settings_page.emit(SettingsPageMsg::Reload);
            }
            AppMsg::ShowDirectoryChooser => {
                let app = relm4::main_adw_application();
                if let Some(window) = app.active_window() {
                    let sender_clone = sender.input_sender().clone();
                    dialogs::show_directory_chooser(&window, move |path| {
                        sender_clone.emit(AppMsg::DirectorySelected(path));
                    });
                }
            }
            AppMsg::DirectorySelected(path) => {
                self.settings_page.emit(SettingsPageMsg::DirectorySelected(path));
            }
            AppMsg::StatusPageOutput(output) => match output {
                StatusPageOutput::NavigateTo(_page) => {
                    // View stack switching handled by user clicking tabs
                }
            },
            AppMsg::AppListPageOutput(output) => match output {
                AppListPageOutput::ShowToast(msg) => {
                    sender.input(AppMsg::ShowToast(msg));
                }
            },
            AppMsg::SettingsPageOutput(output) => match output {
                SettingsPageOutput::ShowToast(msg) => {
                    sender.input(AppMsg::ShowToast(msg));
                }
                SettingsPageOutput::ShowDirectoryChooser => {
                    sender.input(AppMsg::ShowDirectoryChooser);
                }
            },
        }
    }
}

/// Show the about dialog.
fn show_about_dialog() {
    let dialog = adw::AboutWindow::builder()
        .application_name("AppImage Auto Settings")
        .application_icon("appimage-auto")
        .developer_name("Caleb")
        .version(env!("CARGO_PKG_VERSION"))
        .website("https://github.com/youruser/appimage-auto")
        .issue_url("https://github.com/youruser/appimage-auto/issues")
        .license_type(gtk::License::MitX11)
        .comments("Configure automatic AppImage integration")
        .build();

    let app = relm4::main_adw_application();
    if let Some(window) = app.active_window() {
        dialog.set_transient_for(Some(&window));
    }
    dialog.present();
}
