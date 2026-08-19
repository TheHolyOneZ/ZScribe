use tauri::{AppHandle, Manager};

use crate::state::AppState;

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if !app.state::<AppState>().settings().system.show_notifications {
        return;
    }
    show(app, title, body);
}

pub fn notify_failure(app: &AppHandle, title: &str, body: &str) {
    tracing::warn!(title, body, "notifying the user of a failure");
    show(app, title, body);
}

#[cfg(target_os = "linux")]
fn show(_app: &AppHandle, title: &str, body: &str) {
    let title = title.to_owned();
    let body = body.to_owned();

    std::thread::spawn(move || {
        if let Err(err) = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .appname("ZScribe")
            .show()
        {
            tracing::debug!(%err, "could not show a desktop notification");
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn show(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;

    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        tracing::debug!(%err, "could not show a desktop notification");
    }
}
