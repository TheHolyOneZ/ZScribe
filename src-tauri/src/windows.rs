use crate::events::{MAIN_WINDOW, PLAYER_WINDOW, RECORDER_WINDOW};
use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow};

pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        tracing::error!("the main window is missing");
        return;
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.hide();
    }
}

pub fn recorder(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(RECORDER_WINDOW)
}

pub fn show_player(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PLAYER_WINDOW) else {
        tracing::error!("the player window is missing");
        return;
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

pub fn show_recorder(app: &AppHandle) {
    let Some(window) = recorder(app) else {
        tracing::error!("the recording bar is missing");
        return;
    };

    if let Err(err) = position_top_centre(app, &window) {
        tracing::debug!(%err, "could not place the recording bar; using its last position");
    }

    let _ = window.show();
}

pub fn hide_recorder(app: &AppHandle) {
    if let Some(window) = recorder(app) {
        let _ = window.hide();
    }
}

const TOP_MARGIN: f64 = 24.0;

fn position_top_centre(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = app
        .get_webview_window(MAIN_WINDOW)
        .and_then(|main| main.current_monitor().ok().flatten())
        .or(app.primary_monitor()?);

    let Some(monitor) = monitor else {
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let size = window.outer_size()?.to_logical::<f64>(scale);
    let screen = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);

    let x = origin.x + (screen.width - size.width) / 2.0;
    let y = origin.y + TOP_MARGIN;

    window.set_position(LogicalPosition::new(x, y))
}
