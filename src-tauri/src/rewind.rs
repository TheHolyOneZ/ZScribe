use std::time::Duration;

use tauri::{AppHandle, Manager};
use zscribe_audio::{RecordOptions, Session};

use crate::state::AppState;

pub fn reconcile(app: &AppHandle) {
    open(app, Vec::new());
}

pub fn restore(app: &AppHandle, buffered: Vec<f32>) {
    if !buffered.is_empty() {
        tracing::info!(
            ms = (buffered.len() * 1000) / 16_000,
            "the recording never started; keeping what was buffered"
        );
    }
    open(app, buffered);
}

fn open(app: &AppHandle, holding: Vec<f32>) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let wanted = settings.recording.rewind_seconds;

    let recording = state
        .pipeline
        .lock()
        .expect("pipeline lock poisoned")
        .is_recording();

    let mut slot = state.rewind.lock().expect("rewind lock poisoned");

    if wanted == 0 || recording {
        if let Some(session) = slot.take() {
            let _ = session.stop();
            tracing::info!("the rewind buffer is off; the microphone is closed");
            drop(slot);
            crate::tray::set_recording(app, recording);
        }
        return;
    }

    if slot.is_some() {
        return;
    }

    let options = RecordOptions {
        device_id: settings.recording.input_device.clone(),
        system_source: None,
        exact_device: false,

        output: std::path::PathBuf::new(),

        preroll: holding,
    };

    match Session::listen(options, Duration::from_secs(u64::from(wanted))) {
        Ok(session) => {
            tracing::info!(seconds = wanted, "listening, in memory only");
            *slot = Some(session);
            drop(slot);
            crate::tray::set_recording(app, false);
        }

        Err(err) => tracing::warn!(%err, "could not open the microphone for the rewind buffer"),
    }
}

pub fn take(app: &AppHandle) -> Vec<f32> {
    let state = app.state::<AppState>();
    let Some(session) = state.rewind.lock().expect("rewind lock poisoned").take() else {
        return Vec::new();
    };

    let buffered = session.buffered();

    let _ = session.stop();

    if !buffered.is_empty() {
        tracing::info!(
            ms = (buffered.len() * 1000) / 16_000,
            "starting with what was already said"
        );
    }
    buffered
}

pub fn buffered_ms(app: &AppHandle) -> u32 {
    app.state::<AppState>()
        .rewind
        .lock()
        .expect("rewind lock poisoned")
        .as_ref()
        .map(Session::buffered_ms)
        .unwrap_or(0)
}
