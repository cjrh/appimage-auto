//! Status page component showing overview information.

use crate::config::Config;
use crate::state::State;
use relm4::adw::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, RelmWidgetExt};
use std::process::Command;

/// The status page model.
pub struct StatusPage {
    /// Number of integrated apps.
    integrated_count: usize,
    /// Number of watch directories.
    watch_dir_count: usize,
    /// Daemon running status.
    daemon_running: bool,
    /// Watch directories (for display).
    watch_dirs: Vec<String>,
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

                        // Status banner
                        adw::StatusPage {
                            set_icon_name: Some("emblem-system-symbolic"),
                            set_title: "AppImage Auto",
                            #[watch]
                            set_description: Some(&format!(
                                "Daemon: {}",
                                if model.daemon_running { "Running" } else { "Stopped" }
                            )),
                        },

                        // Quick stats
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 12,
                            set_halign: gtk::Align::Center,

                            gtk::Button {
                                add_css_class: "card",
                                set_width_request: 150,
                                connect_clicked => StatusPageMsg::NavigateToApps,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    set_margin_all: 12,

                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.integrated_count.to_string(),
                                        add_css_class: "title-1",
                                    },
                                    gtk::Label {
                                        set_label: "Integrated Apps",
                                        add_css_class: "dim-label",
                                    },
                                }
                            },

                            gtk::Button {
                                add_css_class: "card",
                                set_width_request: 150,
                                connect_clicked => StatusPageMsg::NavigateToSettings,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 6,
                                    set_margin_all: 12,

                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.watch_dir_count.to_string(),
                                        add_css_class: "title-1",
                                    },
                                    gtk::Label {
                                        set_label: "Watch Directories",
                                        add_css_class: "dim-label",
                                    },
                                }
                            },
                        },

                        // Watch directories list
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,

                            gtk::Label {
                                set_label: "Watched Directories",
                                set_halign: gtk::Align::Start,
                                add_css_class: "heading",
                            },

                            gtk::ListBox {
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
        let model = Self {
            integrated_count: 0,
            watch_dir_count: 0,
            daemon_running: false,
            watch_dirs: Vec::new(),
        };

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
    /// Refresh all status information.
    fn refresh_status(&mut self) {
        // Load state
        if let Ok(state) = State::load() {
            self.integrated_count = state.count();
        } else {
            self.integrated_count = 0;
        }

        // Load config
        if let Ok(config) = Config::load() {
            self.watch_dir_count = config.watch.directories.len();
            self.watch_dirs = config.watch.directories.clone();
        } else {
            self.watch_dir_count = 0;
            self.watch_dirs = Vec::new();
        }

        // Check daemon status (via systemctl or pgrep)
        self.daemon_running = is_daemon_running();
    }
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
