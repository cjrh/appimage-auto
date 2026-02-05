//! Status page component showing overview information.

use crate::config::Config;
use crate::state::State;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, RelmWidgetExt};
use std::path::PathBuf;
use std::process::Command;

/// The status page model.
pub struct StatusPage {
    /// Daemon running status.
    daemon_running: bool,
    /// Number of integrated apps (for heading display).
    integrated_count: usize,
    /// Number of watch directories (for heading display).
    watch_dir_count: usize,
    /// ListBox for integrated app rows.
    apps_list: gtk::ListBox,
    /// ListBox for watch directory rows.
    dirs_list: gtk::ListBox,
}

/// Messages for the status page.
#[derive(Debug)]
pub enum StatusPageMsg {
    /// Refresh status information.
    Refresh,
    /// Navigate to apps page.
    NavigateToApps,
    /// Navigate to settings page.
    NavigateToSettings,
}

/// Output messages from the status page.
#[derive(Debug)]
pub enum StatusPageOutput {
    /// Navigate to a page by tag.
    NavigateTo(String),
}

#[relm4::component(pub)]
impl SimpleComponent for StatusPage {
    type Init = ();
    type Input = StatusPageMsg;
    type Output = StatusPageOutput;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &adw::WindowTitle {
                    set_title: "Overview",
                },

                pack_start = &gtk::Button {
                    set_icon_name: "view-refresh-symbolic",
                    set_tooltip_text: Some("Refresh"),
                    connect_clicked => StatusPageMsg::Refresh,
                },
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                adw::Clamp {
                    set_maximum_size: 600,
                    set_margin_all: 12,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 24,

                        // Status banner (compact)
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_halign: gtk::Align::Center,
                            set_margin_top: 12,
                            set_margin_bottom: 12,

                            gtk::Image {
                                set_icon_name: Some("emblem-system-symbolic"),
                                set_pixel_size: 64,
                                add_css_class: "dim-label",
                            },

                            gtk::Label {
                                set_label: "AppImage Auto",
                                add_css_class: "title-1",
                            },

                            gtk::Label {
                                #[watch]
                                set_label: &format!(
                                    "Daemon: {}",
                                    if model.daemon_running { "Running" } else { "Stopped" }
                                ),
                                add_css_class: "dim-label",
                            },
                        },

                        // Integrated Apps section
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,

                                gtk::Label {
                                    #[watch]
                                    set_label: &format!(
                                        "Integrated Apps ({})",
                                        model.integrated_count
                                    ),
                                    set_halign: gtk::Align::Start,
                                    set_hexpand: true,
                                    add_css_class: "heading",
                                },

                                gtk::Button {
                                    set_label: "View All",
                                    add_css_class: "flat",
                                    set_valign: gtk::Align::Center,
                                    connect_clicked => StatusPageMsg::NavigateToApps,
                                },
                            },

                            #[local_ref]
                            apps_list_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                add_css_class: "boxed-list",
                            },
                        },

                        // Watch Directories section
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,

                                gtk::Label {
                                    #[watch]
                                    set_label: &format!(
                                        "Watched Directories ({})",
                                        model.watch_dir_count
                                    ),
                                    set_halign: gtk::Align::Start,
                                    set_hexpand: true,
                                    add_css_class: "heading",
                                },

                                gtk::Button {
                                    set_label: "Settings",
                                    add_css_class: "flat",
                                    set_valign: gtk::Align::Center,
                                    connect_clicked => StatusPageMsg::NavigateToSettings,
                                },
                            },

                            #[local_ref]
                            dirs_list_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                add_css_class: "boxed-list",
                            },
                        },
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
        let apps_list = gtk::ListBox::new();
        let dirs_list = gtk::ListBox::new();

        let model = Self {
            daemon_running: false,
            integrated_count: 0,
            watch_dir_count: 0,
            apps_list: apps_list.clone(),
            dirs_list: dirs_list.clone(),
        };

        let apps_list_box = &model.apps_list;
        let dirs_list_box = &model.dirs_list;
        let widgets = view_output!();

        // Initial refresh
        sender.input(StatusPageMsg::Refresh);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            StatusPageMsg::Refresh => {
                self.refresh_status();
            }
            StatusPageMsg::NavigateToApps => {
                sender
                    .output(StatusPageOutput::NavigateTo("apps".to_string()))
                    .unwrap();
            }
            StatusPageMsg::NavigateToSettings => {
                sender
                    .output(StatusPageOutput::NavigateTo("settings".to_string()))
                    .unwrap();
            }
        }
    }
}

impl StatusPage {
    fn refresh_status(&mut self) {
        clear_list(&self.apps_list);
        clear_list(&self.dirs_list);

        // Load and populate integrated apps
        if let Ok(state) = State::load() {
            let mut apps: Vec<_> = state.all().cloned().collect();
            apps.sort_by(|a, b| {
                let name_a = a.name.as_deref().unwrap_or("");
                let name_b = b.name.as_deref().unwrap_or("");
                name_a.to_lowercase().cmp(&name_b.to_lowercase())
            });

            self.integrated_count = apps.len();

            if apps.is_empty() {
                add_placeholder(&self.apps_list, "No integrated apps");
            } else {
                for app in &apps {
                    let name = app.name.clone().unwrap_or_else(|| {
                        app.appimage_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".to_string())
                    });
                    let exists = app.appimage_path.exists();
                    let icon = if exists {
                        "application-x-executable-symbolic"
                    } else {
                        "dialog-warning-symbolic"
                    };

                    let row = adw::ActionRow::new();
                    row.set_title(&name);
                    row.set_subtitle(&app.appimage_path.display().to_string());
                    row.add_prefix(&gtk::Image::from_icon_name(icon));
                    self.apps_list.append(&row);
                }
            }
        } else {
            self.integrated_count = 0;
            add_placeholder(&self.apps_list, "No integrated apps");
        }

        // Load and populate watch directories
        if let Ok(config) = Config::load() {
            self.watch_dir_count = config.watch.directories.len();

            if config.watch.directories.is_empty() {
                add_placeholder(&self.dirs_list, "No watched directories");
            } else {
                for dir in &config.watch.directories {
                    let expanded = shellexpand::tilde(dir);
                    let expanded_path = PathBuf::from(expanded.as_ref());
                    let exists = expanded_path.exists();
                    let icon = if exists {
                        "folder-symbolic"
                    } else {
                        "dialog-warning-symbolic"
                    };

                    let row = adw::ActionRow::new();
                    row.set_title(dir);
                    if dir != expanded.as_ref() {
                        row.set_subtitle(&expanded_path.display().to_string());
                    }
                    row.add_prefix(&gtk::Image::from_icon_name(icon));
                    self.dirs_list.append(&row);
                }
            }
        } else {
            self.watch_dir_count = 0;
            add_placeholder(&self.dirs_list, "No watched directories");
        }

        self.daemon_running = is_daemon_running();
    }
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn add_placeholder(list: &gtk::ListBox, title: &str) {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.add_css_class("dim-label");
    list.append(&row);
}

/// Check if the daemon is running.
fn is_daemon_running() -> bool {
    // Try systemctl first
    if let Ok(output) = Command::new("systemctl")
        .args(["--user", "is-active", "appimage-auto"])
        .output()
    {
        if output.status.success() {
            let status = String::from_utf8_lossy(&output.stdout);
            if status.trim() == "active" {
                return true;
            }
        }
    }

    // Fall back to pgrep
    if let Ok(output) = Command::new("pgrep")
        .args(["-f", "appimage-auto daemon"])
        .output()
    {
        return output.status.success();
    }

    false
}
