use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use ts_rs::TS;
use zscribe_core::{ProviderId, ProviderProfile, Template, DEFAULT_TEMPLATE_ID};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+R";

pub const AUTO_LANGUAGE: &str = "auto";

fn default_summary_language() -> String {
    AUTO_LANGUAGE.to_owned()
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not read settings from {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write settings to {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("settings file is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error(
        "settings were written by a newer version of ZScribe (schema {found}, this build \
         understands {CURRENT_SCHEMA_VERSION}); upgrade the app rather than downgrading the file"
    )]
    FromTheFuture { found: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AudioSource {
    #[default]
    Microphone,
    System,
    Both,
}

impl AudioSource {
    pub const fn is_implemented(self) -> bool {
        matches!(self, AudioSource::Microphone)
    }

    pub const fn label(self) -> &'static str {
        match self {
            AudioSource::Microphone => "Microphone",
            AudioSource::System => "System audio",
            AudioSource::Both => "Microphone and system audio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceKind {
    #[default]
    Microphone,

    SystemAudio,
}

impl SourceKind {
    pub const fn label(self) -> &'static str {
        match self {
            SourceKind::Microphone => "Microphone",
            SourceKind::SystemAudio => "System audio",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceProfile {
    pub device_id: String,

    pub name: String,

    #[serde(default)]
    pub kind: SourceKind,

    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RecordingSettings {
    pub source: AudioSource,

    pub input_device: Option<String>,

    pub sources: Vec<SourceProfile>,

    pub announce_tone: bool,

    pub consent_note: bool,

    pub keep_audio: bool,

    #[serde(default)]
    pub rewind_seconds: u32,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        Self {
            source: AudioSource::default(),
            input_device: None,
            sources: Vec::new(),
            announce_tone: false,
            consent_note: false,
            keep_audio: true,
            rewind_seconds: 0,
        }
    }
}

impl RecordingSettings {
    pub fn source_label(&self) -> String {
        let enabled: Vec<&SourceProfile> = self.sources.iter().filter(|s| s.enabled).collect();

        match enabled.as_slice() {
            [] => self.source.label().to_owned(),
            [only] => {
                if only.name.is_empty() {
                    only.kind.label().to_owned()
                } else {
                    only.name.clone()
                }
            }
            many => format!("{} sources", many.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct TranscriptionSettings {
    pub model_id: String,

    pub language: String,

    pub use_gpu: bool,

    pub timestamps: bool,

    pub detect_speakers: bool,

    pub live_transcript: bool,
}

impl Default for TranscriptionSettings {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            language: AUTO_LANGUAGE.to_owned(),
            use_gpu: true,
            timestamps: true,
            detect_speakers: true,
            live_transcript: false,
        }
    }
}

impl TranscriptionSettings {
    pub fn has_model(&self) -> bool {
        !self.model_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ArchiveSettings {
    pub embedding_model: String,
}

impl Default for ArchiveSettings {
    fn default() -> Self {
        Self {
            embedding_model: "nomic-embed-text".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct FolderSettings {
    pub watch: Option<String>,

    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PrivacySettings {
    pub redact_contacts: bool,

    pub redact_speakers: bool,

    pub redact_terms: Vec<String>,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            redact_contacts: true,
            redact_speakers: false,
            redact_terms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SystemSettings {
    pub start_with_os: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub show_notifications: bool,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            start_with_os: false,
            start_minimized: false,
            minimize_to_tray: true,
            show_notifications: true,
        }
    }
}

pub const OPACITY_MIN: u8 = 40;

pub const MAX_REWIND_SECONDS: u32 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppearanceSettings {
    pub theme: Theme,
    pub opacity: u8,

    pub recorder_opacity: u8,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            opacity: 100,
            recorder_opacity: 92,
        }
    }
}

pub const SIDEBAR_SECTIONS: [&str; 11] = [
    "library",
    "ask",
    "hotkeys",
    "recording",
    "transcription",
    "templates",
    "providers",
    "appearance",
    "storage",
    "system",
    "about",
];

pub const DEFAULT_CATEGORY_ID: &str = "recordings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SidebarCategory {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SidebarLayout {
    pub categories: Vec<SidebarCategory>,
}

impl Default for SidebarLayout {
    fn default() -> Self {
        let group = |id: &str, name: &str, items: &[&str]| SidebarCategory {
            id: id.to_owned(),
            name: name.to_owned(),
            collapsed: false,
            items: items.iter().map(|s| (*s).to_owned()).collect(),
        };

        Self {
            categories: vec![
                group(DEFAULT_CATEGORY_ID, "Recordings", &["library", "ask"]),
                group("capture", "Capture", &["hotkeys", "recording"]),
                group(
                    "understanding",
                    "Understanding",
                    &["transcription", "templates", "providers"],
                ),
                group(
                    "application",
                    "Application",
                    &["appearance", "storage", "system", "about"],
                ),
            ],
        }
    }
}

impl SidebarLayout {
    fn normalize(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        for category in &mut self.categories {
            category
                .items
                .retain(|item| SIDEBAR_SECTIONS.contains(&item.as_str()) && !seen.contains(item));
            seen.extend(category.items.iter().cloned());
        }

        if self.categories.is_empty() {
            self.categories.push(SidebarCategory {
                id: DEFAULT_CATEGORY_ID.to_owned(),
                name: "Recordings".to_owned(),
                collapsed: false,
                items: Vec::new(),
            });
        }

        let missing: Vec<String> = SIDEBAR_SECTIONS
            .iter()
            .filter(|section| !seen.iter().any(|s| s == *section))
            .map(|section| (*section).to_owned())
            .collect();
        if let Some(first) = self.categories.first_mut() {
            first.items.extend(missing);
        }

        for category in &mut self.categories {
            if category.name.trim().is_empty() {
                category.name = "Untitled".to_owned();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppSettings {
    pub schema_version: u32,

    pub hotkey: String,

    pub template_id: String,
    pub custom_templates: Vec<Template>,

    #[serde(default = "default_summary_language")]
    pub summary_language: String,

    pub active_provider: ProviderId,
    pub providers: Vec<ProviderProfile>,

    pub recording: RecordingSettings,
    pub transcription: TranscriptionSettings,

    #[serde(default)]
    pub archive: ArchiveSettings,

    #[serde(default)]
    pub privacy: PrivacySettings,

    #[serde(default)]
    pub folders: FolderSettings,

    pub system: SystemSettings,
    pub appearance: AppearanceSettings,
    pub sidebar: SidebarLayout,

    pub consent_acknowledged: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            hotkey: DEFAULT_HOTKEY.to_owned(),
            template_id: DEFAULT_TEMPLATE_ID.to_owned(),
            custom_templates: Vec::new(),
            summary_language: default_summary_language(),
            active_provider: ProviderId::default(),
            providers: ProviderId::ALL
                .iter()
                .copied()
                .map(ProviderProfile::new)
                .collect(),
            recording: RecordingSettings::default(),
            transcription: TranscriptionSettings::default(),
            archive: ArchiveSettings::default(),
            privacy: PrivacySettings::default(),
            folders: FolderSettings::default(),
            system: SystemSettings::default(),
            appearance: AppearanceSettings::default(),
            sidebar: SidebarLayout::default(),
            consent_acknowledged: false,
        }
    }
}

impl AppSettings {
    pub fn all_templates(&self) -> Vec<Template> {
        let mut templates = zscribe_core::builtin_templates();
        templates.extend(self.custom_templates.iter().cloned());
        templates
    }

    pub fn active_template(&self) -> Template {
        zscribe_core::template::resolve(&self.template_id, &self.custom_templates)
    }

    pub fn active_provider_profile(&self) -> ProviderProfile {
        self.providers
            .iter()
            .find(|p| p.id == self.active_provider)
            .cloned()
            .unwrap_or_else(|| ProviderProfile::new(self.active_provider))
    }

    pub fn normalize(&mut self) {
        self.appearance.opacity = self.appearance.opacity.clamp(OPACITY_MIN, 100);
        self.appearance.recorder_opacity = self.appearance.recorder_opacity.clamp(OPACITY_MIN, 100);
        self.sidebar.normalize();

        if self.hotkey.trim().is_empty() {
            self.hotkey = DEFAULT_HOTKEY.to_owned();
        }

        if self.transcription.language.trim().is_empty() {
            self.transcription.language = AUTO_LANGUAGE.to_owned();
        }

        if self.summary_language.trim().is_empty() {
            self.summary_language = AUTO_LANGUAGE.to_owned();
        }

        if !self.recording.source.is_implemented() {
            self.recording.source = AudioSource::Microphone;
        }

        if self
            .recording
            .input_device
            .as_ref()
            .is_some_and(|d| d.trim().is_empty())
        {
            self.recording.input_device = None;
        }

        let mut seen_devices: Vec<String> = Vec::new();
        self.recording.sources.retain(|source| {
            let id = source.device_id.trim().to_owned();
            if id.is_empty() || seen_devices.contains(&id) {
                return false;
            }
            seen_devices.push(id);
            true
        });
        for source in &mut self.recording.sources {
            source.name = source.name.trim().to_owned();
        }

        for id in ProviderId::ALL {
            if !self.providers.iter().any(|p| p.id == id) {
                self.providers.push(ProviderProfile::new(id));
            }
        }
        for profile in &mut self.providers {
            if profile.model.trim().is_empty() {
                profile.model = profile.id.default_model().to_owned();
            }
        }

        if self.recording.rewind_seconds > MAX_REWIND_SECONDS {
            self.recording.rewind_seconds = MAX_REWIND_SECONDS;
        }

        for folder in [&mut self.folders.watch, &mut self.folders.notes] {
            if folder.as_ref().is_some_and(|path| path.trim().is_empty()) {
                *folder = None;
            }
        }

        let mut seen_terms: Vec<String> = Vec::new();
        self.privacy.redact_terms.retain(|term| {
            let term = term.trim().to_lowercase();
            if term.is_empty() || seen_terms.contains(&term) {
                return false;
            }
            seen_terms.push(term);
            true
        });
        for term in &mut self.privacy.redact_terms {
            *term = term.trim().to_owned();
        }

        let builtin_ids: Vec<String> = zscribe_core::builtin_templates()
            .into_iter()
            .map(|t| t.id)
            .collect();
        self.custom_templates
            .retain(|t| !builtin_ids.contains(&t.id));
    }

    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(SettingsError::Read {
                    path: path.display().to_string(),
                    source,
                })
            }
        };

        let value: serde_json::Value = serde_json::from_str(&raw)?;
        let mut settings = migrate(value)?;
        settings.normalize();
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let json = serde_json::to_string_pretty(self)?;
        let temp = path.with_extension("json.tmp");

        std::fs::write(&temp, json).map_err(|source| SettingsError::Write {
            path: temp.display().to_string(),
            source,
        })?;

        std::fs::rename(&temp, path).map_err(|source| SettingsError::Write {
            path: path.display().to_string(),
            source,
        })
    }
}

pub fn migrate(mut value: serde_json::Value) -> Result<AppSettings, SettingsError> {
    let found = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32;

    if found > CURRENT_SCHEMA_VERSION {
        return Err(SettingsError::FromTheFuture { found });
    }

    for version in found..CURRENT_SCHEMA_VERSION {
        match version {
            0 => value["schemaVersion"] = serde_json::json!(CURRENT_SCHEMA_VERSION),
            _ => unreachable!("no migration defined for schema version {version}"),
        }
    }

    let mut settings: AppSettings = serde_json::from_value(value)?;
    settings.schema_version = CURRENT_SCHEMA_VERSION;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        (dir, path)
    }

    #[test]
    fn defaults_are_usable_without_any_configuration() {
        let settings = AppSettings::default();
        assert_eq!(settings.active_provider, ProviderId::Ollama);
        assert_eq!(settings.providers.len(), ProviderId::ALL.len());
        assert!(!settings.hotkey.is_empty());
        assert_eq!(settings.active_template().id, DEFAULT_TEMPLATE_ID);
    }

    #[test]
    fn a_fresh_install_has_no_model_and_has_not_seen_the_notice() {
        let settings = AppSettings::default();
        assert!(!settings.transcription.has_model());
        assert!(!settings.consent_acknowledged);
    }

    #[test]
    fn a_missing_file_loads_the_defaults_rather_than_failing() {
        let (_dir, path) = temp_file();
        assert_eq!(
            AppSettings::load(&path).expect("load"),
            AppSettings::default()
        );
    }

    #[test]
    fn settings_round_trip_through_disk() {
        let (_dir, path) = temp_file();
        let mut settings = AppSettings::default();
        settings.transcription.model_id = "large-v3-turbo".to_owned();
        settings.hotkey = "Ctrl+Shift+R".to_owned();

        settings.save(&path).expect("save");
        assert_eq!(AppSettings::load(&path).expect("load"), settings);
    }

    #[test]
    fn an_interrupted_save_leaves_no_stray_temporary_file() {
        let (_dir, path) = temp_file();
        AppSettings::default().save(&path).expect("save");
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_file_from_a_newer_build_is_refused_rather_than_silently_downgraded() {
        let value = serde_json::json!({ "schemaVersion": CURRENT_SCHEMA_VERSION + 1 });
        assert!(matches!(
            migrate(value),
            Err(SettingsError::FromTheFuture { .. })
        ));
    }

    #[test]
    fn a_file_with_no_schema_version_is_migrated_to_the_current_one() {
        let settings = migrate(serde_json::json!({ "hotkey": "Ctrl+Alt+X" })).expect("migrate");
        assert_eq!(settings.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(settings.hotkey, "Ctrl+Alt+X");
    }

    #[test]
    fn unknown_fields_from_a_newer_build_do_not_break_the_load() {
        let value = serde_json::json!({
            "schemaVersion": CURRENT_SCHEMA_VERSION,
            "somethingFromTheFuture": { "nested": true },
        });
        assert!(migrate(value).is_ok());
    }

    #[test]
    fn an_emptied_hotkey_falls_back_to_the_default() {
        let mut settings = AppSettings {
            hotkey: "   ".to_owned(),
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
    }

    #[test]
    fn a_source_this_build_cannot_record_falls_back_to_the_microphone() {
        let mut settings = AppSettings {
            recording: RecordingSettings {
                source: AudioSource::Both,
                ..Default::default()
            },
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.recording.source, AudioSource::Microphone);
    }

    #[test]
    fn opacity_is_clamped_so_the_window_can_never_become_invisible() {
        let mut settings = AppSettings {
            appearance: AppearanceSettings {
                opacity: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.appearance.opacity, OPACITY_MIN);
    }

    #[test]
    fn the_recording_bar_can_never_be_made_invisible_either() {
        let mut settings = AppSettings {
            appearance: AppearanceSettings {
                recorder_opacity: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.appearance.recorder_opacity, OPACITY_MIN);
    }

    #[test]
    fn the_recording_bar_is_slightly_see_through_by_default() {
        let appearance = AppearanceSettings::default();
        assert!(appearance.recorder_opacity < 100);
        assert!(appearance.recorder_opacity >= OPACITY_MIN);
    }

    #[test]
    fn a_missing_provider_profile_is_restored() {
        let mut settings = AppSettings {
            providers: vec![ProviderProfile::new(ProviderId::Ollama)],
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.providers.len(), ProviderId::ALL.len());
    }

    #[test]
    fn an_emptied_model_field_falls_back_to_the_provider_default() {
        let mut settings = AppSettings::default();
        settings.providers[0].model = "  ".to_owned();
        settings.normalize();
        assert_eq!(
            settings.providers[0].model,
            settings.providers[0].id.default_model()
        );
    }

    #[test]
    fn a_custom_template_may_not_shadow_a_builtin_id() {
        let mut settings = AppSettings {
            custom_templates: vec![Template {
                id: "meeting".to_owned(),
                name: "Mine".to_owned(),
                description: String::new(),
                instructions: String::new(),
            }],
            ..Default::default()
        };
        settings.normalize();
        assert!(settings.custom_templates.is_empty());
        assert_eq!(settings.active_template().id, DEFAULT_TEMPLATE_ID);
    }

    #[test]
    fn configured_sources_survive_a_save_and_reload() {
        let (_dir, path) = temp_file();

        let settings = AppSettings {
            recording: RecordingSettings {
                sources: vec![
                    SourceProfile {
                        device_id: "alsa_output.usb-FiiO.monitor".to_owned(),
                        name: "YouTube".to_owned(),
                        kind: SourceKind::SystemAudio,
                        enabled: true,
                    },
                    SourceProfile {
                        device_id: "alsa:pulse".to_owned(),
                        name: "Max Kruger".to_owned(),
                        kind: SourceKind::Microphone,
                        enabled: true,
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        settings.save(&path).expect("save");
        let loaded = AppSettings::load(&path).expect("load");

        assert_eq!(loaded.recording.sources.len(), 2);
        assert_eq!(loaded.recording.sources[0].name, "YouTube");
        assert_eq!(loaded.recording.sources[0].kind, SourceKind::SystemAudio);
        assert_eq!(loaded.recording.sources[1].name, "Max Kruger");
        assert_eq!(loaded.recording.sources[1].kind, SourceKind::Microphone);
    }

    #[test]
    fn a_source_whose_device_is_unplugged_is_kept_rather_than_deleted() {
        let mut settings = AppSettings {
            recording: RecordingSettings {
                sources: vec![SourceProfile {
                    device_id: "alsa:a-device-that-is-not-here-today".to_owned(),
                    name: "Max Kruger".to_owned(),
                    kind: SourceKind::Microphone,
                    enabled: true,
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        settings.normalize();

        assert_eq!(
            settings.recording.sources.len(),
            1,
            "the row is the user's, not the device's"
        );
    }

    #[test]
    fn a_recording_is_labelled_with_what_was_actually_recorded() {
        let profile = |name: &str, kind| SourceProfile {
            device_id: "dev".to_owned(),
            name: name.to_owned(),
            kind,
            enabled: true,
        };

        let mut settings = RecordingSettings::default();
        assert_eq!(settings.source_label(), "Microphone");

        settings.sources = vec![profile("", SourceKind::SystemAudio)];
        assert_eq!(settings.source_label(), "System audio");

        settings.sources = vec![profile("Speaker", SourceKind::SystemAudio)];
        assert_eq!(settings.source_label(), "Speaker");

        settings.sources = vec![
            profile("Etienne", SourceKind::Microphone),
            profile("Max Kruger", SourceKind::Microphone),
            profile("Zoom call", SourceKind::SystemAudio),
        ];
        assert_eq!(settings.source_label(), "3 sources");

        settings.sources[2].enabled = false;
        assert_eq!(settings.source_label(), "2 sources");
        settings.sources[1].enabled = false;
        assert_eq!(settings.source_label(), "Etienne");
    }

    #[test]
    fn the_recording_list_is_reachable_from_the_sidebar() {
        assert!(SIDEBAR_SECTIONS.contains(&"library"));

        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![SidebarCategory {
                    id: "general".to_owned(),
                    name: "General".to_owned(),
                    collapsed: false,
                    items: vec!["library".to_owned(), "hotkeys".to_owned()],
                }],
            },
            ..Default::default()
        };
        settings.normalize();

        assert!(settings.sidebar.categories[0]
            .items
            .contains(&"library".to_owned()));
    }

    #[test]
    fn the_recording_list_comes_first_in_a_fresh_sidebar() {
        let layout = SidebarLayout::default();
        assert_eq!(layout.categories[0].items[0], "library");
        assert_eq!(layout.categories[0].name, "Recordings");
    }

    #[test]
    fn every_section_appears_exactly_once_in_the_default_sidebar() {
        let layout = SidebarLayout::default();
        let mut items: Vec<&str> = layout
            .categories
            .iter()
            .flat_map(|c| c.items.iter().map(String::as_str))
            .collect();
        items.sort_unstable();

        let mut expected = SIDEBAR_SECTIONS.to_vec();
        expected.sort_unstable();
        assert_eq!(items, expected);
    }

    #[test]
    fn a_layout_from_an_older_build_gains_the_new_sections() {
        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![SidebarCategory {
                    id: "general".to_owned(),
                    name: "General".to_owned(),
                    collapsed: false,
                    items: vec!["hotkeys".to_owned()],
                }],
            },
            ..Default::default()
        };
        settings.normalize();

        let items: Vec<&str> = settings.sidebar.categories[0]
            .items
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(items.len(), SIDEBAR_SECTIONS.len());
        assert_eq!(items[0], "hotkeys", "the user's own order is preserved");
    }

    #[test]
    fn a_section_this_build_does_not_have_is_dropped() {
        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![SidebarCategory {
                    id: "general".to_owned(),
                    name: "General".to_owned(),
                    collapsed: false,
                    items: vec!["hotkeys".to_owned(), "diarization".to_owned()],
                }],
            },
            ..Default::default()
        };
        settings.normalize();
        assert!(!settings.sidebar.categories[0]
            .items
            .iter()
            .any(|i| i == "diarization"));
    }

    #[test]
    fn the_removed_logs_panel_disappears_from_a_saved_layout() {
        assert!(!SIDEBAR_SECTIONS.contains(&"logs"));

        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![SidebarCategory {
                    id: "application".to_owned(),
                    name: "Application".to_owned(),
                    collapsed: false,
                    items: vec![
                        "appearance".to_owned(),
                        "logs".to_owned(),
                        "about".to_owned(),
                    ],
                }],
            },
            ..Default::default()
        };
        settings.normalize();

        let items = &settings.sidebar.categories[0].items;
        assert!(!items.iter().any(|i| i == "logs"));
        assert!(items.iter().any(|i| i == "appearance"));
        assert!(items.iter().any(|i| i == "about"));
    }

    #[test]
    fn a_section_duplicated_across_categories_survives_only_once() {
        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![
                    SidebarCategory {
                        id: "a".to_owned(),
                        name: "A".to_owned(),
                        collapsed: false,
                        items: vec!["hotkeys".to_owned()],
                    },
                    SidebarCategory {
                        id: "b".to_owned(),
                        name: "B".to_owned(),
                        collapsed: false,
                        items: vec!["hotkeys".to_owned()],
                    },
                ],
            },
            ..Default::default()
        };
        settings.normalize();

        let count = settings
            .sidebar
            .categories
            .iter()
            .flat_map(|c| &c.items)
            .filter(|i| *i == "hotkeys")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn an_emptied_category_name_gets_a_placeholder_rather_than_rendering_blank() {
        let mut settings = AppSettings {
            sidebar: SidebarLayout {
                categories: vec![SidebarCategory {
                    id: "a".to_owned(),
                    name: "  ".to_owned(),
                    collapsed: false,
                    items: Vec::new(),
                }],
            },
            ..Default::default()
        };
        settings.normalize();
        assert_eq!(settings.sidebar.categories[0].name, "Untitled");
    }
}
