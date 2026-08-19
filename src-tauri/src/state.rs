use std::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zscribe_platform::Capabilities;
use zscribe_store::{AppSettings, Paths, Recordings, SecretStore, SettingsError};

use crate::hotkeys::HotkeyStatus;
use crate::recording::Pipeline;

pub struct AppState {
    pub paths: Paths,
    pub settings: Mutex<AppSettings>,
    pub secrets: SecretStore,
    pub recordings: Mutex<Recordings>,
    pub capabilities: Capabilities,

    pub pipeline: Mutex<Pipeline>,

    pub in_flight: Mutex<Option<CancellationToken>>,

    pub hotkey_status: Mutex<HotkeyStatus>,

    pub tray_toggle: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,

    pub media: std::sync::OnceLock<crate::media::Media>,

    pub player_recording: Mutex<Option<crate::commands::PlayerOpen>>,

    pub rewind: Mutex<Option<zscribe_audio::Session>>,

    pub model_download: Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,

    pub llm_download: Mutex<Option<CancellationToken>>,

    #[cfg(target_os = "linux")]
    pub portal_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl AppState {
    pub fn load() -> anyhow::Result<Self> {
        let paths = Paths::from_env()?;
        let settings = AppSettings::load(&paths.settings_file())?;
        let recordings = Recordings::open(&paths.recordings_db())?;
        let secrets = SecretStore::new(paths.fallback_key_file(), paths.fallback_secrets_file());
        let capabilities = Capabilities::detect();

        tracing::info!(
            display_server = ?capabilities.display_server,
            hotkey = ?capabilities.hotkey,
            secrets = ?secrets.backend(),
            data_dir = %paths.data_dir().display(),
            "session ready"
        );

        let hotkey_status = HotkeyStatus {
            accelerator: settings.hotkey.clone(),
            display: settings.hotkey.clone(),
            registered: false,
            problem: Some("not registered yet".to_owned()),
        };

        Ok(Self {
            paths,
            settings: Mutex::new(settings),
            secrets,
            recordings: Mutex::new(recordings),
            capabilities,
            pipeline: Mutex::new(Pipeline::default()),
            in_flight: Mutex::new(None),
            hotkey_status: Mutex::new(hotkey_status),
            tray_toggle: Mutex::new(None),
            media: std::sync::OnceLock::new(),
            player_recording: Mutex::new(None),
            rewind: Mutex::new(None),
            model_download: Mutex::new(None),
            llm_download: Mutex::new(None),
            #[cfg(target_os = "linux")]
            portal_task: Mutex::new(None),
        })
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .expect("settings lock poisoned")
            .clone()
    }

    pub fn save_settings(&self, mut next: AppSettings) -> Result<AppSettings, SettingsError> {
        next.normalize();
        next.save(&self.paths.settings_file())?;
        *self.settings.lock().expect("settings lock poisoned") = next.clone();
        Ok(next)
    }

    pub fn begin_request(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let mut slot = self.in_flight.lock().expect("in-flight lock poisoned");
        if let Some(previous) = slot.replace(token.clone()) {
            tracing::debug!("superseding the summary already in flight");
            previous.cancel();
        }
        token
    }

    pub fn cancel_in_flight(&self) {
        if let Some(token) = self
            .in_flight
            .lock()
            .expect("in-flight lock poisoned")
            .take()
        {
            token.cancel();
        }
    }

    pub fn finish_request(&self, token: &CancellationToken) {
        let mut slot = self.in_flight.lock().expect("in-flight lock poisoned");
        if slot.as_ref().is_some_and(|current| current == token) {
            *slot = None;
        }
    }
}
