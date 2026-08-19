use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::windows;

const TOGGLE: &str = "toggle-recording";
const SHOW: &str = "show-window";
const QUIT: &str = "quit";

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();

    let toggle = MenuItem::with_id(handle, TOGGLE, "Start recording", true, None::<&str>)?;
    let show = MenuItem::with_id(handle, SHOW, "Open ZScribe", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, QUIT, "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        handle,
        &[
            &toggle,
            &PredefinedMenuItem::separator(handle)?,
            &show,
            &PredefinedMenuItem::separator(handle)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("the application icon is missing".to_owned())
        })?)
        .tooltip("ZScribe")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                windows::show_main(tray.app_handle());
            }
        })
        .build(app)?;

    app.state::<AppState>()
        .tray_toggle
        .lock()
        .expect("tray lock poisoned")
        .replace(toggle);

    Ok(())
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let app = app.clone();

    match event.id().as_ref() {
        TOGGLE => crate::recording::toggle(app),
        SHOW => windows::show_main(&app),
        QUIT => {
            crate::recording::stop(app.clone());
            app.exit(0);
        }
        other => tracing::debug!(item = other, "unhandled tray menu item"),
    }
}

pub fn set_recording(app: &AppHandle, recording: bool) {
    if let Some(item) = app
        .state::<AppState>()
        .tray_toggle
        .lock()
        .expect("tray lock poisoned")
        .as_ref()
    {
        let _ = item.set_text(if recording {
            "Stop recording"
        } else {
            "Start recording"
        });
    }

    if let Some(tray) = app.tray_by_id("main") {
        let listening = crate::rewind::buffered_ms(app) > 0;

        let _ = tray.set_tooltip(Some(match (recording, listening) {
            (true, _) => "ZScribe — recording",
            (false, true) => "ZScribe — microphone open, keeping the last few minutes",
            (false, false) => "ZScribe",
        }));
    }
}
