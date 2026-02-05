//! Desktop notification support (optional feature).

use std::path::Path;

#[cfg(feature = "notifications")]
use tracing::warn;
#[cfg(not(feature = "notifications"))]
use tracing::debug;

/// Events that can trigger a desktop notification.
pub enum NotificationEvent {
    /// An AppImage was successfully integrated.
    Integrated {
        name: String,
        path: String,
        icon: Option<String>,
    },
    /// An AppImage was unintegrated (removed from menu).
    Unintegrated { name: String, path: String },
}

/// Send a desktop notification for an event.
#[cfg(feature = "notifications")]
pub fn send(event: NotificationEvent) {
    use notify_rust::Notification;

    let result = match &event {
        NotificationEvent::Integrated { name, path, icon } => {
            let mut n = Notification::new();
            n.appname("AppImage Auto")
                .summary(&format!("{} integrated", name))
                .body(&format!("Ready in application menu\n{}", path));
            if let Some(i) = icon {
                n.icon(i);
            } else {
                n.icon("appimage-auto");
            }
            n.show()
        }
        NotificationEvent::Unintegrated { name, path } => Notification::new()
            .appname("AppImage Auto")
            .summary(&format!("{} removed", name))
            .body(path)
            .icon("appimage-auto")
            .show(),
    };

    if let Err(e) = result {
        warn!("Notification failed: {}", e);
    }
}

/// Send a desktop notification for an event (no-op when feature disabled).
#[cfg(not(feature = "notifications"))]
pub fn send(_event: NotificationEvent) {
    debug!("Notifications disabled at compile time");
}

/// Create an integration notification event.
pub fn integrated(name: &str, path: &Path, icon: Option<&Path>) -> NotificationEvent {
    NotificationEvent::Integrated {
        name: name.to_string(),
        path: path.display().to_string(),
        icon: icon.map(|p| p.display().to_string()),
    }
}

/// Create an unintegration notification event.
pub fn unintegrated(name: &str, path: &Path) -> NotificationEvent {
    NotificationEvent::Unintegrated {
        name: name.to_string(),
        path: path.display().to_string(),
    }
}
