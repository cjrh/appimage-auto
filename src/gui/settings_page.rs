//! Settings page component.

use super::autostart;
use super::watch_dir_row::{WatchDirRow, WatchDirRowOutput};
use crate::config::Config;
use relm4::adw::prelude::*;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::gtk::glib;
use relm4::gtk;
use relm4::prelude::*;
use relm4::{adw, ComponentParts, ComponentSender, RelmWidgetExt};
use std::path::PathBuf;

/// The settings page model.
pub struct SettingsPage {
    /// The current configuration.
    config: Config,
    /// Factory for watch directory rows.
    watch_dirs: FactoryVecDeque<WatchDirRow>,
    /// Autostart enabled status.
    autostart_enabled: bool,
}

/// Messages for the settings page.
#[derive(Debug)]
pub enum SettingsPageMsg {
    /// Reload settings from config file.
    Reload,
    /// Add a new watch directory.
    AddWatchDir,
    /// Remove a watch directory by index.
    RemoveWatchDir(DynamicIndex),
    /// Handle directory selected from chooser.
    DirectorySelected(PathBuf),
    /// Toggle notifications enabled.
    ToggleNotifications(bool),
    /// Toggle notify on integrate.
    ToggleNotifyOnIntegrate(bool),
    /// Toggle notify on unintegrate.
    ToggleNotifyOnUnintegrate(bool),
    /// Set log level.
    SetLogLevel(u32),
    /// Toggle autostart.
    ToggleAutostart(bool),
    /// Toggle scan on startup.
    ToggleScanOnStartup(bool),
    /// Set debounce delay.
    SetDebounceMs(f64),
}

/// Output messages from the settings page.
#[derive(Debug)]
pub enum SettingsPageOutput {
    /// Request to show a toast message.
    ShowToast(String),
    /// Request to show directory chooser.
    ShowDirectoryChooser,
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsPage {
    type Init = ();
    type Input = SettingsPageMsg;
    type Output = SettingsPageOutput;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            adw::HeaderBar {
                #[wrap(Some)]
                set_title_widget = &adw::WindowTitle {
                    set_title: "Settings",
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

                        // Watch Directories Section
                        adw::PreferencesGroup {
                            set_title: "Watch Directories",
                            set_description: Some("Directories to monitor for AppImages"),

                            #[wrap(Some)]
                            set_header_suffix = &gtk::Button {
                                set_icon_name: "list-add-symbolic",
                                add_css_class: "flat",
                                set_tooltip_text: Some("Add watch directory"),
                                connect_clicked[sender] => move |_| {
                                    sender.output(SettingsPageOutput::ShowDirectoryChooser).unwrap();
                                },
                            },

                            #[local_ref]
                            watch_dirs_box -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                                add_css_class: "boxed-list",
                            }
                        },

                        // Notifications Section
                        adw::PreferencesGroup {
                            set_title: "Notifications",
                            set_description: Some("Desktop notification settings"),

                            adw::ActionRow {
                                set_title: "Enable Notifications",
                                set_subtitle: "Show desktop notifications for integration events",

                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.config.notifications.enabled,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(SettingsPageMsg::ToggleNotifications(state));
                                        glib::Propagation::Proceed
                                    },
                                },
                            },

                            adw::ActionRow {
                                set_title: "Notify on Integration",
                                set_subtitle: "Show notification when an AppImage is integrated",
                                #[watch]
                                set_sensitive: model.config.notifications.enabled,

                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.config.notifications.on_integrate,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(SettingsPageMsg::ToggleNotifyOnIntegrate(state));
                                        glib::Propagation::Proceed
                                    },
                                },
                            },

                            adw::ActionRow {
                                set_title: "Notify on Removal",
                                set_subtitle: "Show notification when integration is removed",
                                #[watch]
                                set_sensitive: model.config.notifications.enabled,

                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.config.notifications.on_unintegrate,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(SettingsPageMsg::ToggleNotifyOnUnintegrate(state));
                                        glib::Propagation::Proceed
                                    },
                                },
                            },
                        },

                        // Daemon Settings Section
                        adw::PreferencesGroup {
                            set_title: "Daemon",
                            set_description: Some("Daemon behavior settings"),

                            adw::ActionRow {
                                set_title: "Scan on Startup",
                                set_subtitle: "Integrate existing AppImages when daemon starts",

                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.config.integration.scan_on_startup,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(SettingsPageMsg::ToggleScanOnStartup(state));
                                        glib::Propagation::Proceed
                                    },
                                },
                            },

                            adw::ComboRow {
                                set_title: "Log Level",
                                set_subtitle: "Verbosity of daemon logging",
                                set_model: Some(&gtk::StringList::new(&["error", "warn", "info", "debug", "trace"])),
                                #[watch]
                                set_selected: match model.config.logging.level.as_str() {
                                    "error" => 0,
                                    "warn" => 1,
                                    "info" => 2,
                                    "debug" => 3,
                                    "trace" => 4,
                                    _ => 2,
                                },
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(SettingsPageMsg::SetLogLevel(row.selected()));
                                },
                            },

                            adw::ActionRow {
                                set_title: "Debounce Delay (ms)",
                                set_subtitle: "Wait time before processing file events",

                                add_suffix = &gtk::SpinButton::with_range(100.0, 10000.0, 100.0) {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_value: model.config.watch.debounce_ms as f64,
                                    connect_value_changed[sender] => move |btn| {
                                        sender.input(SettingsPageMsg::SetDebounceMs(btn.value()));
                                    },
                                },
                            },
                        },

                        // Autostart Section
                        adw::PreferencesGroup {
                            set_title: "Startup",
                            set_description: Some("Automatic startup settings"),

                            adw::ActionRow {
                                set_title: "Start on Login",
                                set_subtitle: "Automatically start daemon when you log in (XDG autostart)",

                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.autostart_enabled,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(SettingsPageMsg::ToggleAutostart(state));
                                        glib::Propagation::Proceed
                                    },
                                },
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
        let config = Config::load().unwrap_or_default();
        let autostart_enabled = autostart::is_autostart_enabled();

        let watch_dirs = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                WatchDirRowOutput::Remove(index) => SettingsPageMsg::RemoveWatchDir(index),
            });

        let mut model = Self {
            config,
            watch_dirs,
            autostart_enabled,
        };

        // Populate watch directories
        model.reload_watch_dirs();

        let watch_dirs_box = model.watch_dirs.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SettingsPageMsg::Reload => {
                if let Ok(config) = Config::load() {
                    self.config = config;
                    self.reload_watch_dirs();
                }
                self.autostart_enabled = autostart::is_autostart_enabled();
            }
            SettingsPageMsg::AddWatchDir => {
                sender.output(SettingsPageOutput::ShowDirectoryChooser).unwrap();
            }
            SettingsPageMsg::DirectorySelected(path) => {
                // Convert to string with ~ for home directory
                let path_str = if let Some(home) = dirs::home_dir() {
                    if path.starts_with(&home) {
                        format!("~/{}", path.strip_prefix(&home).unwrap().display())
                    } else {
                        path.display().to_string()
                    }
                } else {
                    path.display().to_string()
                };

                // Add to config if not already present
                if !self.config.watch.directories.contains(&path_str) {
                    self.config.watch.directories.push(path_str);
                    self.save_config(&sender);
                    self.reload_watch_dirs();
                }
            }
            SettingsPageMsg::RemoveWatchDir(index) => {
                let idx = index.current_index();
                if idx < self.config.watch.directories.len() {
                    self.config.watch.directories.remove(idx);
                    self.save_config(&sender);
                    self.reload_watch_dirs();
                }
            }
            SettingsPageMsg::ToggleNotifications(enabled) => {
                self.config.notifications.enabled = enabled;
                self.save_config(&sender);
            }
            SettingsPageMsg::ToggleNotifyOnIntegrate(enabled) => {
                self.config.notifications.on_integrate = enabled;
                self.save_config(&sender);
            }
            SettingsPageMsg::ToggleNotifyOnUnintegrate(enabled) => {
                self.config.notifications.on_unintegrate = enabled;
                self.save_config(&sender);
            }
            SettingsPageMsg::SetLogLevel(index) => {
                let level = match index {
                    0 => "error",
                    1 => "warn",
                    2 => "info",
                    3 => "debug",
                    4 => "trace",
                    _ => "info",
                };
                self.config.logging.level = level.to_string();
                self.save_config(&sender);
            }
            SettingsPageMsg::ToggleAutostart(enabled) => {
                match autostart::set_autostart(enabled) {
                    Ok(()) => {
                        self.autostart_enabled = enabled;
                        let msg = if enabled {
                            "Autostart enabled"
                        } else {
                            "Autostart disabled"
                        };
                        sender
                            .output(SettingsPageOutput::ShowToast(msg.to_string()))
                            .unwrap();
                    }
                    Err(e) => {
                        sender
                            .output(SettingsPageOutput::ShowToast(format!(
                                "Failed to set autostart: {}",
                                e
                            )))
                            .unwrap();
                        // Revert the UI toggle
                        self.autostart_enabled = autostart::is_autostart_enabled();
                    }
                }
            }
            SettingsPageMsg::ToggleScanOnStartup(enabled) => {
                self.config.integration.scan_on_startup = enabled;
                self.save_config(&sender);
            }
            SettingsPageMsg::SetDebounceMs(ms) => {
                self.config.watch.debounce_ms = ms as u64;
                self.save_config(&sender);
            }
        }
    }
}

impl SettingsPage {
    /// Reload watch directories from config.
    fn reload_watch_dirs(&mut self) {
        let mut guard = self.watch_dirs.guard();
        guard.clear();

        for dir in &self.config.watch.directories {
            guard.push_back(dir.clone());
        }
    }

    /// Save config to file.
    fn save_config(&self, sender: &ComponentSender<Self>) {
        if let Err(e) = self.config.save() {
            sender
                .output(SettingsPageOutput::ShowToast(format!(
                    "Failed to save config: {}",
                    e
                )))
                .unwrap();
        }
    }
}
