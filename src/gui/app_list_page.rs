//! AppImage list page component.

use super::app_row::{AppImageRow, AppImageRowOutput};
use crate::state::{IntegratedAppImage, State};
use relm4::adw::prelude::*;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::gtk;
use relm4::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, RelmWidgetExt};
use std::path::PathBuf;
use std::process::Command;

/// The app list page model.
pub struct AppListPage {
    /// Factory for AppImage rows.
    app_rows: FactoryVecDeque<AppImageRow>,
    /// Count of integrated apps.
    app_count: usize,
}

/// Messages for the app list page.
#[derive(Debug)]
pub enum AppListPageMsg {
    /// Reload the app list from state.
    Reload,
    /// Remove an app by factory index.
    RemoveApp(DynamicIndex),
    /// Open a file location in the file manager.
    OpenLocation(PathBuf),
}

/// Output messages from the app list page.
#[derive(Debug)]
pub enum AppListPageOutput {
    /// Request to show a toast message.
    ShowToast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for AppListPage {
    type Init = ();
    type Input = AppListPageMsg;
    type Output = AppListPageOutput;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &adw::WindowTitle {
                    set_title: "Integrated Apps",
                },

                pack_start = &gtk::Button {
                    set_icon_name: "view-refresh-symbolic",
                    set_tooltip_text: Some("Refresh list"),
                    connect_clicked => AppListPageMsg::Reload,
                },
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                adw::Clamp {
                    set_maximum_size: 600,
                    set_margin_all: 12,

                    if model.app_count == 0 {
                        adw::StatusPage {
                            set_icon_name: Some("application-x-executable-symbolic"),
                            set_title: "No Integrated Apps",
                            set_description: Some("AppImages you integrate will appear here.\nDrop an AppImage into a watched directory to get started."),
                        }
                    } else {
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,

                            gtk::Label {
                                #[watch]
                                set_label: &format!("{} integrated app{}", model.app_count, if model.app_count == 1 { "" } else { "s" }),
                                set_halign: gtk::Align::Start,
                                add_css_class: "dim-label",
                            },

                            #[local_ref]
                            app_list_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                add_css_class: "boxed-list",
                            },
                        }
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let app_rows = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                AppImageRowOutput::Remove(index) => AppListPageMsg::RemoveApp(index),
                AppImageRowOutput::OpenLocation(path) => AppListPageMsg::OpenLocation(path),
            });

        let model = Self {
            app_rows,
            app_count: 0,
        };

        let app_list_box = model.app_rows.widget();
        let widgets = view_output!();

        // Initial load
        sender.input(AppListPageMsg::Reload);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            AppListPageMsg::Reload => {
                self.reload_apps();
            }
            AppListPageMsg::RemoveApp(index) => {
                if let Some(row) = self.app_rows.get(index.current_index()) {
                    let path = row.appimage_path.clone();
                    let path_str = path.to_string_lossy().to_string();

                    // Spawn CLI to remove integration
                    match Command::new("appimage-auto")
                        .args(["remove", &path_str])
                        .spawn()
                    {
                        Ok(mut child) => {
                            // Wait for completion
                            let _ = child.wait();
                            sender.input(AppListPageMsg::Reload);
                            sender
                                .output(AppListPageOutput::ShowToast(
                                    "Integration removed".to_string(),
                                ))
                                .unwrap();
                        }
                        Err(e) => {
                            sender
                                .output(AppListPageOutput::ShowToast(format!(
                                    "Failed to remove: {}",
                                    e
                                )))
                                .unwrap();
                        }
                    }
                }
            }
            AppListPageMsg::OpenLocation(path) => {
                // Open file manager at location
                let _ = Command::new("xdg-open").arg(&path).spawn();
            }
        }
    }
}

impl AppListPage {
    /// Reload the app list from state.
    fn reload_apps(&mut self) {
        let mut guard = self.app_rows.guard();
        guard.clear();

        if let Ok(state) = State::load() {
            let mut apps: Vec<IntegratedAppImage> = state.all().cloned().collect();
            // Sort by name
            apps.sort_by(|a, b| {
                let name_a = a.name.as_deref().unwrap_or("");
                let name_b = b.name.as_deref().unwrap_or("");
                name_a.to_lowercase().cmp(&name_b.to_lowercase())
            });

            self.app_count = apps.len();
            for app in apps {
                guard.push_back(app);
            }
        } else {
            self.app_count = 0;
        }
    }
}
