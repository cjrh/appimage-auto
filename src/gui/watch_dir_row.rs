//! Watch directory row factory component.

use relm4::adw::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;
use relm4::adw;
use std::path::PathBuf;

/// A single watch directory entry in the list.
#[derive(Debug)]
pub struct WatchDirRow {
    /// The directory path (unexpanded, may contain ~).
    pub path: String,
    /// The expanded path for display.
    pub expanded_path: PathBuf,
    /// Whether the directory exists.
    pub exists: bool,
}

/// Output messages from the watch directory row.
#[derive(Debug)]
pub enum WatchDirRowOutput {
    Remove(DynamicIndex),
}

#[relm4::factory(pub)]
impl FactoryComponent for WatchDirRow {
    type Init = String;
    type Input = ();
    type Output = WatchDirRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        adw::ActionRow {
            set_title: &self.path,
            set_subtitle: &self.expanded_path.display().to_string(),

            add_prefix = &gtk::Image {
                set_icon_name: Some(if self.exists { "folder-symbolic" } else { "dialog-warning-symbolic" }),
            },

            add_suffix = &gtk::Button {
                set_icon_name: "user-trash-symbolic",
                set_valign: gtk::Align::Center,
                add_css_class: "flat",
                set_tooltip_text: Some("Remove watch directory"),
                connect_clicked[sender, index] => move |_| {
                    sender.output(WatchDirRowOutput::Remove(index.clone())).unwrap();
                },
            },
        }
    }

    fn init_model(path: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let expanded_path = PathBuf::from(shellexpand::tilde(&path).as_ref());
        let exists = expanded_path.exists();

        Self {
            path,
            expanded_path,
            exists,
        }
    }
}
