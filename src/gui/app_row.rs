//! AppImage row factory component for the app list.

use crate::state::IntegratedAppImage;
use relm4::adw::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;
use relm4::adw;
use std::path::PathBuf;

/// A single AppImage entry in the list.
#[derive(Debug)]
pub struct AppImageRow {
    /// The integrated AppImage identifier.
    pub identifier: String,
    /// Application name.
    pub name: String,
    /// Path to the AppImage file.
    pub appimage_path: PathBuf,
    /// Whether the AppImage file still exists.
    pub exists: bool,
}

/// Messages for the AppImage row.
#[derive(Debug)]
pub enum AppImageRowMsg {
    OpenLocation,
}

/// Output messages from the AppImage row.
#[derive(Debug)]
pub enum AppImageRowOutput {
    Remove(DynamicIndex),
    OpenLocation(PathBuf),
}

#[relm4::factory(pub)]
impl FactoryComponent for AppImageRow {
    type Init = IntegratedAppImage;
    type Input = AppImageRowMsg;
    type Output = AppImageRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        adw::ActionRow {
            set_title: &self.name,
            set_subtitle: &self.appimage_path.display().to_string(),
            set_activatable: true,

            add_prefix = &gtk::Image {
                set_icon_name: Some(if self.exists { "application-x-executable-symbolic" } else { "dialog-warning-symbolic" }),
            },

            add_suffix = &gtk::Box {
                set_spacing: 6,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_icon_name: "folder-open-symbolic",
                    add_css_class: "flat",
                    set_tooltip_text: Some("Open file location"),
                    connect_clicked[sender] => move |_| {
                        sender.input(AppImageRowMsg::OpenLocation);
                    },
                },

                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    add_css_class: "flat",
                    set_tooltip_text: Some("Remove integration"),
                    connect_clicked[sender, index] => move |_| {
                        sender.output(AppImageRowOutput::Remove(index.clone())).unwrap();
                    },
                },
            },
        }
    }

    fn init_model(info: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let exists = info.appimage_path.exists();
        let name = info.name.clone().unwrap_or_else(|| {
            info.appimage_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        });

        Self {
            identifier: info.identifier,
            name,
            appimage_path: info.appimage_path,
            exists,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            AppImageRowMsg::OpenLocation => {
                if let Some(parent) = self.appimage_path.parent() {
                    sender
                        .output(AppImageRowOutput::OpenLocation(parent.to_path_buf()))
                        .unwrap();
                }
            }
        }
    }
}
