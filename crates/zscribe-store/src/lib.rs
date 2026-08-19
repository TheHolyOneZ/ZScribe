#![forbid(unsafe_code)]

pub mod paths;
pub mod recordings;
pub mod secrets;
pub mod settings;

pub use paths::{Paths, PathsError};
pub use recordings::{
    NewRecording, Recording, RecordingDetail, Recordings, RecordingsError, SearchHit, MATCH_CLOSE,
    MATCH_OPEN,
};
pub use secrets::{SecretBackend, SecretError, SecretStore};
pub use settings::{
    AppSettings, AppearanceSettings, ArchiveSettings, AudioSource, RecordingSettings,
    SettingsError, SidebarCategory, SidebarLayout, SourceKind, SourceProfile, SystemSettings,
    Theme, TranscriptionSettings, AUTO_LANGUAGE, CURRENT_SCHEMA_VERSION, DEFAULT_HOTKEY,
};
