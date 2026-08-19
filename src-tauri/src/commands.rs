use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;
use zscribe_audio::InputDevice;
use zscribe_core::{summary::to_markdown, ProviderId, Template};
use zscribe_platform::{Capabilities, Machine};
use zscribe_providers::{ModelInfo, ProviderErrorInfo};
use zscribe_store::{AppSettings, Recording, RecordingDetail, SecretBackend};
use zscribe_stt::{InstalledModel, Recommendation};

use crate::events;
use crate::hotkeys::HotkeyStatus;
use crate::recording::{self, RecordingStatus};
use crate::state::AppState;
use crate::windows;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub remedy: String,
    pub retryable: bool,
}

impl CommandError {
    pub(crate) fn new(code: &str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            remedy: remedy.into(),
            retryable: false,
        }
    }
}

impl From<ProviderErrorInfo> for CommandError {
    fn from(info: ProviderErrorInfo) -> Self {
        Self {
            code: info.code,
            message: info.message,
            remedy: info.remedy,
            retryable: info.retryable,
        }
    }
}

impl From<zscribe_providers::ProviderError> for CommandError {
    fn from(error: zscribe_providers::ProviderError) -> Self {
        ProviderErrorInfo::from(&error).into()
    }
}

impl From<zscribe_store::SettingsError> for CommandError {
    fn from(error: zscribe_store::SettingsError) -> Self {
        Self::new(
            "settings",
            error.to_string(),
            "Check that ZScribe's configuration directory is writable. The path is in the About \
             panel.",
        )
    }
}

impl From<zscribe_store::RecordingsError> for CommandError {
    fn from(error: zscribe_store::RecordingsError) -> Self {
        Self::new(
            "database",
            error.to_string(),
            "The recordings database could not be read. If this persists, the file is named in \
             the Storage panel.",
        )
    }
}

impl From<zscribe_store::SecretError> for CommandError {
    fn from(error: zscribe_store::SecretError) -> Self {
        Self::new(
            "keychain",
            error.to_string(),
            "Your system keychain refused the request. ZScribe falls back to an encrypted file \
             if no keychain is available.",
        )
    }
}

impl From<zscribe_audio::AudioError> for CommandError {
    fn from(error: zscribe_audio::AudioError) -> Self {
        Self::new("audio", error.to_string(), error.remedy())
    }
}

impl From<zscribe_stt::DownloadError> for CommandError {
    fn from(error: zscribe_stt::DownloadError) -> Self {
        Self::new("download", error.to_string(), error.remedy())
    }
}

impl From<zscribe_stt::SttError> for CommandError {
    fn from(error: zscribe_stt::SttError) -> Self {
        Self::new(error.code(), error.to_string(), error.remedy())
    }
}

type Response<T> = Result<T, CommandError>;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Response<AppSettings> {
    let previous = state.settings();
    let saved = state.save_settings(settings)?;

    if saved.hotkey != previous.hotkey {
        if let Err(err) = crate::hotkeys::register_and_record(&app, &saved.hotkey) {
            tracing::warn!(%err, "the new hotkey could not be registered");
        }
    }

    if saved.recording.rewind_seconds != previous.recording.rewind_seconds
        || saved.recording.input_device != previous.recording.input_device
    {
        crate::rewind::reconcile(&app);
    }

    let _ = app.emit(events::SETTINGS_CHANGED, &saved);
    Ok(saved)
}

#[tauri::command]
pub fn get_templates(state: State<'_, AppState>) -> Vec<Template> {
    state.settings().all_templates()
}

#[tauri::command]
pub fn derive_template(
    state: State<'_, AppState>,
    from_id: String,
    name: String,
) -> Response<Template> {
    let settings = state.settings();
    let from = settings
        .all_templates()
        .into_iter()
        .find(|t| t.id == from_id)
        .ok_or_else(|| {
            CommandError::new(
                "not_found",
                "that template no longer exists",
                "Pick one from the list and try again.",
            )
        })?;

    Ok(zscribe_core::template::derive_custom(&from, name.trim()))
}

#[tauri::command]
pub fn get_capabilities(state: State<'_, AppState>) -> Capabilities {
    state.capabilities.clone()
}

#[tauri::command]
pub fn scan_machine(state: State<'_, AppState>) -> Machine {
    Machine::probe(&state.paths.models_dir())
}

#[tauri::command]
pub fn recommend_model(state: State<'_, AppState>) -> Recommendation {
    zscribe_stt::recommend(&Machine::probe(&state.paths.models_dir()))
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppPaths {
    pub config_dir: String,
    pub data_dir: String,
    pub recordings_dir: String,
    pub models_dir: String,
    pub logs_dir: String,

    #[ts(type = "number")]
    pub recordings_bytes: u64,

    #[ts(type = "number")]
    pub models_bytes: u64,
}

#[tauri::command]
pub fn get_paths(state: State<'_, AppState>) -> AppPaths {
    AppPaths {
        config_dir: state.paths.config_dir().display().to_string(),
        data_dir: state.paths.data_dir().display().to_string(),
        recordings_dir: state.paths.recordings_dir().display().to_string(),
        models_dir: state.paths.models_dir().display().to_string(),
        logs_dir: state.paths.logs_dir().display().to_string(),
        recordings_bytes: directory_size(&state.paths.recordings_dir()),
        models_bytes: directory_size(&state.paths.models_dir()),
    }
}

fn directory_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum()
}

#[tauri::command]
pub fn app_version() -> String {
    zscribe_core::VERSION.to_owned()
}

#[tauri::command]
pub fn gpu_support() -> bool {
    zscribe_stt::gpu_support_compiled_in()
}

#[tauri::command]
pub fn validate_hotkey(accelerator: String) -> Response<String> {
    zscribe_platform::Hotkey::parse(&accelerator)
        .map(|hotkey| hotkey.to_string())
        .map_err(|err| {
            CommandError::new(
                "hotkey",
                err.to_string(),
                "Pick a combination with at least one modifier and one key.",
            )
        })
}

#[tauri::command]
pub fn get_hotkey_status(state: State<'_, AppState>) -> HotkeyStatus {
    state
        .hotkey_status
        .lock()
        .expect("hotkey-status lock poisoned")
        .clone()
}

#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
    provider: ProviderId,
) -> Response<Vec<ModelInfo>> {
    let settings = state.settings();
    let profile = settings
        .providers
        .iter()
        .find(|p| p.id == provider)
        .cloned()
        .unwrap_or_else(|| zscribe_core::ProviderProfile::new(provider));

    let api_key = state.secrets.get(provider.slug()).ok().flatten();
    let backend = zscribe_providers::build(&profile, api_key)?;

    Ok(backend.list_models().await?)
}

#[tauri::command]
pub fn set_api_key(state: State<'_, AppState>, provider: ProviderId, key: String) -> Response<()> {
    state.secrets.set(provider.slug(), key.trim())?;
    tracing::info!(provider = provider.slug(), "API key stored");
    Ok(())
}

#[tauri::command]
pub fn has_api_key(state: State<'_, AppState>, provider: ProviderId) -> bool {
    state.secrets.has(provider.slug())
}

#[tauri::command]
pub fn get_secret_backend(state: State<'_, AppState>) -> SecretBackend {
    state.secrets.backend()
}

#[tauri::command]
pub fn list_catalogue() -> Vec<zscribe_providers::catalogue::CatalogueEntry> {
    zscribe_providers::catalogue::CATALOGUE.to_vec()
}

#[tauri::command]
pub fn suggest_llm(state: State<'_, AppState>) -> zscribe_providers::catalogue::Suggestion {
    let machine = Machine::probe(&state.paths.models_dir());
    let gpu = machine.best_gpu();

    zscribe_providers::catalogue::suggest(
        gpu.map_or(0, |gpu| gpu.vram_mb),
        machine.available_ram_mb,
        gpu.map(|gpu| gpu.name.as_str()),
    )
}

#[tauri::command]
pub async fn llm_acceleration(state: State<'_, AppState>) -> Response<Option<bool>> {
    let profile = state
        .settings()
        .providers
        .into_iter()
        .find(|p| p.id == ProviderId::Ollama)
        .unwrap_or_else(|| zscribe_core::ProviderProfile::new(ProviderId::Ollama));

    let ollama = zscribe_providers::ollama::Ollama::new(profile.base_url().to_owned())?;

    Ok(ollama
        .running()
        .await
        .ok()
        .and_then(|running| running.first().map(|loaded| loaded.on_gpu())))
}

#[tauri::command]
pub fn install_llm(app: AppHandle, state: State<'_, AppState>, model_id: String) -> Response<()> {
    let profile = state
        .settings()
        .providers
        .into_iter()
        .find(|p| p.id == ProviderId::Ollama)
        .unwrap_or_else(|| zscribe_core::ProviderProfile::new(ProviderId::Ollama));

    let base_url = profile.base_url().to_owned();
    let cancel = tokio_util::sync::CancellationToken::new();
    *state.llm_download.lock().expect("download lock poisoned") = Some(cancel.clone());

    tauri::async_runtime::spawn(async move {
        let ollama = match zscribe_providers::ollama::Ollama::new(base_url) {
            Ok(ollama) => ollama,
            Err(err) => {
                tracing::error!(%err, "could not reach ollama");
                let _ = app.emit(
                    events::PIPELINE_FAILED,
                    &crate::recording::PipelineFailure {
                        recording_id: String::new(),
                        stage: "installing".to_owned(),
                        error: ProviderErrorInfo::from(&err),
                    },
                );
                let _ = app.emit(events::LLM_PROGRESS, (&model_id, ()));
                return;
            }
        };

        let id = model_id.clone();
        let progress_app = app.clone();

        let result = ollama
            .pull(&model_id, &cancel, move |progress| {
                let _ = progress_app.emit(events::LLM_PROGRESS, (&id, progress));
            })
            .await;

        match result {
            Ok(()) => {
                crate::feedback::notify(&app, "Model installed", &format!("{model_id} is ready."));
            }
            Err(zscribe_providers::ProviderError::Cancelled) => {
                tracing::info!(model = %model_id, "model download cancelled");
            }
            Err(err) => {
                tracing::error!(model = %model_id, %err, "model download failed");
                let _ = app.emit(
                    events::PIPELINE_FAILED,
                    &crate::recording::PipelineFailure {
                        recording_id: String::new(),
                        stage: "installing".to_owned(),
                        error: ProviderErrorInfo::from(&err),
                    },
                );
            }
        }

        let _ = app.emit(events::LLM_PROGRESS, (&model_id, ()));
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_llm_install(state: State<'_, AppState>) {
    if let Some(cancel) = state
        .llm_download
        .lock()
        .expect("download lock poisoned")
        .take()
    {
        cancel.cancel();
    }
}

#[tauri::command]
pub fn list_whisper_models() -> Vec<zscribe_stt::ModelSpec> {
    zscribe_stt::MODELS.to_vec()
}

#[tauri::command]
pub fn installed_models(state: State<'_, AppState>) -> Vec<InstalledModel> {
    zscribe_stt::installed(&state.paths.models_dir())
}

#[tauri::command]
pub fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> Response<()> {
    let models_dir = state.paths.models_dir();
    let free_disk_mb = Machine::probe(&models_dir).free_disk_mb;

    let cancel = Arc::new(AtomicBool::new(false));
    *state.model_download.lock().expect("download lock poisoned") = Some(Arc::clone(&cancel));

    std::thread::spawn(move || {
        let id_for_event = model_id.clone();
        let progress_app = app.clone();

        let result = zscribe_stt::fetch(
            &model_id,
            &models_dir,
            free_disk_mb,
            cancel,
            move |progress| {
                let _ = progress_app.emit(events::MODEL_PROGRESS, (&id_for_event, progress));
            },
        );

        match result {
            Ok(_) => {
                tracing::info!(model = %model_id, "model ready");
                let _ = app.emit(
                    events::MODEL_PROGRESS,
                    (
                        &model_id,
                        zscribe_stt::Progress {
                            downloaded_bytes: 0,
                            total_bytes: 0,
                            percent: 100,
                            verifying: false,
                        },
                    ),
                );
                crate::feedback::notify(&app, "Model installed", &format!("{model_id} is ready."));
            }
            Err(zscribe_stt::DownloadError::Cancelled) => {
                tracing::info!(model = %model_id, "model download cancelled");
            }
            Err(err) => {
                tracing::error!(model = %model_id, %err, "model download failed");
                let _ = app.emit(
                    events::PIPELINE_FAILED,
                    &crate::recording::PipelineFailure {
                        recording_id: String::new(),
                        stage: "downloading".to_owned(),
                        error: ProviderErrorInfo {
                            code: "download".to_owned(),
                            message: err.to_string(),
                            remedy: err.remedy(),
                            retryable: true,
                        },
                    },
                );
            }
        }

        let _ = app.emit(events::MODEL_PROGRESS, (&model_id, ()));
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    if let Some(cancel) = state
        .model_download
        .lock()
        .expect("download lock poisoned")
        .take()
    {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn remove_model(state: State<'_, AppState>, model_id: String) -> Response<bool> {
    Ok(zscribe_stt::download::remove(
        &model_id,
        &state.paths.models_dir(),
    )?)
}

#[tauri::command]
pub fn list_input_devices() -> Vec<InputDevice> {
    zscribe_audio::input_devices()
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemAudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceAvailability {
    pub device_id: String,
    pub available: bool,
}

#[tauri::command]
pub fn check_sources(state: State<'_, AppState>) -> Vec<SourceAvailability> {
    let microphones: Vec<String> = zscribe_audio::input_devices()
        .into_iter()
        .map(|device| device.id)
        .collect();

    let system: Vec<String> = zscribe_audio::system_audio_sources()
        .into_iter()
        .map(|source| source.id)
        .collect();

    state
        .settings()
        .recording
        .sources
        .into_iter()
        .map(|source| {
            let pool = match source.kind {
                zscribe_store::SourceKind::SystemAudio => &system,
                zscribe_store::SourceKind::Microphone => &microphones,
            };

            SourceAvailability {
                available: pool.contains(&source.device_id),
                device_id: source.device_id,
            }
        })
        .collect()
}

#[tauri::command]
pub fn list_system_audio() -> Vec<SystemAudioDevice> {
    zscribe_audio::system_audio_sources()
        .into_iter()
        .map(|source| SystemAudioDevice {
            id: source.id,
            name: source.name,
        })
        .collect()
}

#[tauri::command]
pub fn start_recording(app: AppHandle) -> Response<String> {
    Ok(recording::start(&app)?)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle) {
    recording::stop(app);
}

#[tauri::command]
pub fn toggle_recording(app: AppHandle) {
    recording::toggle(app);
}

#[tauri::command]
pub fn pause_recording(app: AppHandle) {
    recording::pause(&app);
}

#[tauri::command]
pub fn resume_recording(app: AppHandle) {
    recording::resume(&app);
}

#[tauri::command]
pub fn recording_status(app: AppHandle) -> RecordingStatus {
    recording::status(&app)
}

#[tauri::command]
pub fn cancel_processing(app: AppHandle) {
    recording::cancel(&app);
}

#[tauri::command]
pub fn list_recordings(state: State<'_, AppState>, limit: u32) -> Response<Vec<Recording>> {
    Ok(state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .list(limit)?)
}

#[tauri::command]
pub fn get_recording(state: State<'_, AppState>, id: String) -> Response<Option<RecordingDetail>> {
    Ok(state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .get(&id)?)
}

#[tauri::command]
pub fn rename_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Response<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(CommandError::new(
            "empty_title",
            "a recording needs a name",
            "Type a name, or press Escape to keep the current one.",
        ));
    }

    state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .set_title(&id, title)?;

    crate::sync::write_note(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn delete_recording(state: State<'_, AppState>, id: String) -> Response<()> {
    state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .delete(&id)?;

    let mut removed = 0;
    for path in state.paths.audio_files(&id) {
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(err) => {
                tracing::warn!(%err, path = %path.display(), "could not delete the audio file")
            }
        }
    }

    tracing::info!(recording = %id, files = removed, "recording deleted");
    Ok(())
}

#[tauri::command]
pub fn delete_all_recordings(state: State<'_, AppState>) -> Response<u32> {
    let audio = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .delete_all()?;

    let count = audio.len() as u32;
    for path in audio {
        if let Err(err) = std::fs::remove_file(&path) {
            tracing::warn!(%err, path = %path.display(), "could not delete the audio file");
        }
    }

    let swept = sweep_orphaned_audio(&state.paths.recordings_dir());
    if swept > 0 {
        tracing::info!(swept, "removed audio files the database had lost track of");
    }

    tracing::info!(count, "all recordings deleted");
    Ok(count + swept)
}

fn sweep_orphaned_audio(dir: &std::path::Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .filter(|entry| match std::fs::remove_file(entry.path()) {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(%err, path = %entry.path().display(), "could not delete orphaned audio");
                false
            }
        })
        .count() as u32
}

#[tauri::command]
pub fn recording_markdown(state: State<'_, AppState>, id: String) -> Response<String> {
    markdown_for(&state, &id)
}

pub(crate) fn markdown_for(state: &AppState, id: &str) -> Response<String> {
    let detail = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .get(id)?
        .ok_or_else(|| {
            CommandError::new(
                "not_found",
                "that recording no longer exists",
                "It may have been deleted in another window.",
            )
        })?;

    let when = time::OffsetDateTime::from_unix_timestamp(detail.recording.started_at)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(crate::local_offset());

    let recorded_at = format!(
        "{} {} {}, {:02}:{:02}",
        when.day(),
        when.month(),
        when.year(),
        when.hour(),
        when.minute()
    );

    Ok(to_markdown(
        &detail.recording.title,
        &recorded_at,
        detail.summary.as_ref(),
        detail.transcript.as_ref(),
    ))
}

#[tauri::command]
pub async fn ask(
    state: State<'_, AppState>,
    id: String,
    history: Vec<zscribe_core::Turn>,
    question: String,
) -> Response<String> {
    let question = question.trim().to_owned();
    if question.is_empty() {
        return Err(CommandError::new(
            "empty_question",
            "there is no question to ask",
            "Type something first.",
        ));
    }

    let detail = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .get(&id)?
        .ok_or_else(|| {
            CommandError::new(
                "not_found",
                "that recording no longer exists",
                "It may have been deleted in another window.",
            )
        })?;

    let transcript = detail.transcript.ok_or_else(|| {
        CommandError::new(
            "no_transcript",
            "this recording has no transcript yet",
            "Transcribe it first — there is nothing to ask about until then.",
        )
    })?;

    let settings = state.settings();
    let profile = settings.active_provider_profile();
    let api_key = state.secrets.get(profile.id.slug()).ok().flatten();
    let provider = zscribe_providers::build(&profile, api_key)?;

    let redaction = crate::privacy::wanted(&settings, profile.id, &transcript);
    let (transcript, _) = zscribe_core::redact_transcript(&transcript, &redaction);
    let title = zscribe_core::redact(&detail.recording.title, &redaction).0;
    let summary = detail.summary.as_ref().map(|summary| {
        let mut copy = summary.clone();
        copy.body_md = zscribe_core::redact(&summary.body_md, &redaction).0;
        copy
    });
    let history: Vec<zscribe_core::Turn> = history
        .into_iter()
        .map(|turn| zscribe_core::Turn {
            content: zscribe_core::redact(&turn.content, &redaction).0,
            ..turn
        })
        .collect();
    let question = zscribe_core::redact(&question, &redaction).0;

    let prompt = zscribe_core::chat::prompt(
        &zscribe_core::chat::Context {
            transcript: &transcript,
            summary: summary.as_ref(),
            title: &title,
            timestamps: settings.transcription.timestamps,
        },
        &history,
        &question,
    );

    let token = state.begin_request();
    let completion = provider
        .complete(
            &zscribe_providers::CompletionRequest::new(&profile.model, prompt)
                .with_history(history),
            &token,
        )
        .await
        .inspect_err(|_| state.finish_request(&token))?;

    state.finish_request(&token);

    tracing::info!(
        recording = %id,
        model = %profile.model,
        tokens = completion.usage.total(),
        "answered a question about a recording"
    );

    Ok(zscribe_core::clean_model_output(&completion.text))
}

pub(crate) async fn complete_with_active_provider(
    app: &AppHandle,
    prompt: zscribe_core::Prompt,
) -> Response<String> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let profile = settings.active_provider_profile();
    let api_key = state.secrets.get(profile.id.slug()).ok().flatten();
    let provider = zscribe_providers::build(&profile, api_key)?;

    let token = state.begin_request();
    let completion = provider
        .complete(
            &zscribe_providers::CompletionRequest::new(&profile.model, prompt),
            &token,
        )
        .await
        .inspect_err(|_| state.finish_request(&token))?;

    state.finish_request(&token);
    Ok(zscribe_core::clean_model_output(&completion.text))
}

#[tauri::command]
pub async fn archive_status(app: AppHandle) -> Response<crate::archive::ArchiveStatus> {
    crate::archive::status(app).await
}

#[tauri::command]
pub async fn index_archive(app: AppHandle) -> Response<u32> {
    crate::archive::index(app).await
}

#[tauri::command]
pub async fn ask_archive(
    app: AppHandle,
    question: String,
) -> Response<crate::archive::ArchiveAnswer> {
    crate::archive::ask(app, question).await
}

#[tauri::command]
pub fn resummarise(app: AppHandle, id: String) -> Response<()> {
    recording::resummarise(app, id);
    Ok(())
}

#[tauri::command]
pub fn search_recordings(
    app: AppHandle,
    query: String,
    limit: u32,
) -> Response<Vec<zscribe_store::SearchHit>> {
    app.state::<AppState>()
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .search(&query, limit)
        .map_err(|err| {
            tracing::error!(%err, "the search failed");
            CommandError::new(
                "storage",
                "the search could not be run",
                "Check that ZScribe's data directory is readable.",
            )
        })
}

#[tauri::command]
pub fn edit_transcript_line(app: AppHandle, id: String, index: u32, text: String) -> Response<()> {
    let state = app.state::<AppState>();
    let recordings = state.recordings.lock().expect("recordings lock poisoned");

    let storage_error = || {
        CommandError::new(
            "storage",
            "the correction could not be saved",
            "Check that ZScribe's data directory exists and has free space.",
        )
    };

    let Some(mut transcript) = recordings.transcript(&id).map_err(|err| {
        tracing::error!(%err, "could not read the transcript to correct it");
        storage_error()
    })?
    else {
        return Err(CommandError::new(
            "no_transcript",
            "there is no transcript to correct",
            "Transcribe the recording first.",
        ));
    };

    let Some(segment) = transcript.segments.get_mut(index as usize) else {
        return Err(CommandError::new(
            "line_gone",
            "that line is no longer part of this transcript",
            "The transcript changed while you were editing. Look at it again.",
        ));
    };

    segment.text = text;

    recordings.set_transcript(&id, &transcript).map_err(|err| {
        tracing::error!(%err, "could not save the corrected transcript");
        storage_error()
    })?;

    tracing::info!(recording = %id, line = index, "corrected a transcript line");
    Ok(())
}

#[tauri::command]
pub fn recording_subtitles(app: AppHandle, id: String, vtt: bool) -> Response<String> {
    subtitles_for(&app.state::<AppState>(), &id, vtt)
}

fn subtitles_for(state: &AppState, id: &str, vtt: bool) -> Response<String> {
    let format = if vtt {
        zscribe_core::Subtitles::Vtt
    } else {
        zscribe_core::Subtitles::Srt
    };

    let transcript = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .transcript(id)?
        .ok_or_else(|| {
            CommandError::new(
                "no_transcript",
                "this recording has no transcript to export",
                "Transcribe it first — subtitles are the transcript with its timings.",
            )
        })?;

    Ok(zscribe_core::write_subtitles(&transcript, format))
}

#[derive(Debug, Clone, Copy, serde::Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExportFormat {
    Markdown,
    Srt,
    Vtt,
}

#[tauri::command]
pub async fn export_recording(
    app: AppHandle,
    id: String,
    path: String,
    format: ExportFormat,
) -> Response<()> {
    offload(move || {
        let state = app.state::<AppState>();

        let contents = match format {
            ExportFormat::Markdown => markdown_for(&state, &id)?,
            ExportFormat::Srt => subtitles_for(&state, &id, false)?,
            ExportFormat::Vtt => subtitles_for(&state, &id, true)?,
        };

        std::fs::write(&path, contents).map_err(|err| {
            CommandError::new(
                "write_failed",
                format!("could not write {path}: {err}"),
                "Check that the folder still exists and that you can write to it — a removable                  drive that has been unplugged, or a folder owned by another user, will refuse.",
            )
        })?;

        tracing::info!(recording = %id, file = %path, "exported");
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn set_tags(app: AppHandle, id: String, tags: Vec<String>) -> Response<Vec<String>> {
    let state = app.state::<AppState>();
    let recordings = state.recordings.lock().expect("recordings lock poisoned");

    recordings.set_tags(&id, &tags)?;

    Ok(recordings
        .get(&id)?
        .map(|detail| detail.recording.tags)
        .unwrap_or_default())
}

#[tauri::command]
pub fn list_tags(app: AppHandle) -> Response<Vec<(String, u32)>> {
    Ok(app
        .state::<AppState>()
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .tags()?)
}

#[tauri::command]
pub fn rename_speaker(app: AppHandle, id: String, from: String, to: String) -> Response<u32> {
    let to = to.trim().to_owned();
    if to.is_empty() {
        return Err(CommandError::new(
            "empty",
            "a speaker needs a name",
            "Type the name you want on those lines.",
        ));
    }

    let state = app.state::<AppState>();
    let recordings = state.recordings.lock().expect("recordings lock poisoned");

    let Some(mut transcript) = recordings.transcript(&id)? else {
        return Err(CommandError::new(
            "no_transcript",
            "there is no transcript to rename anybody in",
            "Transcribe the recording first.",
        ));
    };

    let mut renamed = 0u32;
    for segment in &mut transcript.segments {
        if segment.speaker.as_deref() == Some(from.as_str()) {
            segment.speaker = Some(to.clone());
            renamed += 1;
        }
    }

    if renamed > 0 {
        recordings.set_transcript(&id, &transcript)?;
        tracing::info!(recording = %id, %from, %to, lines = renamed, "renamed a speaker");
    }

    Ok(renamed)
}

#[tauri::command]
pub fn retranscribe(app: AppHandle, id: String) -> Response<()> {
    recording::retranscribe(app, id)?;
    Ok(())
}

async fn offload<T, F>(work: F) -> Response<T>
where
    T: Send + 'static,
    F: FnOnce() -> Response<T> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => result,
        Err(err) => {
            tracing::error!(%err, "an import thread ended unexpectedly");
            Err(CommandError::new(
                "panicked",
                "the import stopped unexpectedly",
                "Try again. If it keeps happening, report it with the log file — \
                 About says where it is.",
            ))
        }
    }
}

#[tauri::command]
pub async fn import_file(app: AppHandle, path: String) -> Response<String> {
    offload(move || crate::import::file(&app, std::path::PathBuf::from(path))).await
}

#[tauri::command]
pub async fn import_link(app: AppHandle, url: String) -> Response<String> {
    offload(move || crate::import::link(&app, url)).await
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkSupport {
    pub available: bool,

    pub path: Option<String>,

    pub version: Option<String>,

    pub version_age_days: Option<u32>,

    pub stale_after_days: u32,

    pub install_command: String,

    pub nightly_command: Option<String>,

    pub standalone_asset: String,

    pub tools_dir: String,

    pub tools_path: String,

    pub platform: Platform,

    pub js_runtime: Option<String>,

    pub js_runtime_command: String,
}

#[tauri::command]
pub async fn link_support(app: AppHandle) -> Response<LinkSupport> {
    offload(move || {
        let tools = crate::import::tools_dir(&app);
        let found = crate::downloader::find(Some(&tools));

        let version = found.as_deref().and_then(crate::downloader::version);

        let today = time::OffsetDateTime::now_utc().date();
        let version_age_days = version
            .as_deref()
            .and_then(|version| crate::downloader::release_age_days(version, today));

        Ok(LinkSupport {
            available: found.is_some(),
            path: found.map(|path| path.display().to_string()),
            version,
            version_age_days,
            stale_after_days: crate::downloader::STALE_AFTER_DAYS,
            install_command: crate::downloader::install_hint(),
            nightly_command: crate::downloader::nightly_install_command(Some(&tools)),
            standalone_asset: crate::downloader::standalone_asset().to_owned(),
            tools_dir: tools.display().to_string(),
            tools_path: crate::downloader::tools_target(&tools)
                .display()
                .to_string(),
            platform: Platform::current(),
            js_runtime: crate::downloader::js_runtime().map(str::to_owned),
            js_runtime_command: crate::downloader::js_runtime_install_hint(),
        })
    })
    .await
}

#[tauri::command]
pub fn audio_url(app: AppHandle, id: String) -> Option<String> {
    app.state::<AppState>()
        .media
        .get()
        .map(|media| media.url(&id))
}

#[tauri::command]
pub fn open_player(app: AppHandle, id: String, at_ms: Option<u32>) -> Response<()> {
    let opening = PlayerOpen {
        id: id.clone(),
        at_ms,
    };

    *app.state::<AppState>()
        .player_recording
        .lock()
        .expect("player lock poisoned") = Some(opening.clone());

    windows::show_player(&app);

    let _ = app.emit(events::PLAYER_OPEN, &opening);
    Ok(())
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayerOpen {
    pub id: String,
    pub at_ms: Option<u32>,
}

#[tauri::command]
pub fn player_recording(app: AppHandle) -> Option<PlayerOpen> {
    app.state::<AppState>()
        .player_recording
        .lock()
        .expect("player lock poisoned")
        .clone()
}

#[tauri::command]
pub async fn audio_peaks(app: AppHandle, id: String, buckets: u32) -> Response<Vec<u8>> {
    offload(move || {
        let state = app.state::<AppState>();

        let path = state
            .recordings
            .lock()
            .expect("recordings lock poisoned")
            .get(&id)
            .map_err(|err| {
                tracing::error!(%err, "could not read the recording");
                CommandError::new(
                    "storage",
                    "the recording could not be read",
                    "Check that ZScribe's data directory is readable.",
                )
            })?
            .and_then(|detail| detail.recording.audio_path);

        let Some(path) = path else {
            return Ok(Vec::new());
        };

        Ok(
            zscribe_audio::peaks(std::path::Path::new(&path), buckets as usize).unwrap_or_else(
                |err| {
                    tracing::warn!(%err, "could not read the waveform");
                    Vec::new()
                },
            ),
        )
    })
    .await
}

#[tauri::command]
pub fn importable_extensions() -> Vec<String> {
    zscribe_audio::IMPORTABLE_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_owned())
        .collect()
}

#[tauri::command]
pub fn report_error(message: String, source: String) {
    tracing::error!(source = %source, "the interface failed: {message}");
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    windows::show_main(&app);
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Response<()> {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };

    result.map_err(|err| {
        CommandError::new(
            "autostart",
            err.to_string(),
            "Your desktop environment refused the request. You can add ZScribe to your startup \
             applications manually instead.",
        )
    })
}

#[tauri::command]
pub fn is_autostart_enabled(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_conversion_carries_a_remedy() {
        let errors: Vec<CommandError> = vec![
            zscribe_audio::AudioError::NoInputDevice.into(),
            zscribe_stt::SttError::NoModel.into(),
            zscribe_stt::DownloadError::ChecksumMismatch.into(),
            zscribe_providers::ProviderError::Timeout.into(),
        ];

        for error in errors {
            assert!(!error.code.is_empty());
            assert!(!error.message.is_empty());
            assert!(!error.remedy.is_empty(), "{} has no remedy", error.code);
        }
    }

    #[test]
    fn sweeping_removes_stray_audio_but_nothing_else() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("orphan.wav"), b"audio").expect("write");
        std::fs::write(dir.path().join("another.WAV"), b"audio").expect("write");
        std::fs::write(dir.path().join("notes.txt"), b"keep me").expect("write");

        assert_eq!(sweep_orphaned_audio(dir.path()), 2);
        assert!(!dir.path().join("orphan.wav").exists());
        assert!(dir.path().join("notes.txt").exists(), "only audio is swept");
    }

    #[test]
    fn sweeping_an_empty_or_missing_directory_is_not_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(sweep_orphaned_audio(dir.path()), 0);
        assert_eq!(
            sweep_orphaned_audio(std::path::Path::new("/nonexistent-xyz")),
            0
        );
    }

    #[test]
    fn a_directory_that_does_not_exist_measures_as_zero_rather_than_failing() {
        assert_eq!(directory_size(std::path::Path::new("/nonexistent-xyz")), 0);
    }

    #[test]
    fn directory_size_adds_up_the_files_it_finds() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.wav"), vec![0u8; 100]).expect("write");
        std::fs::write(dir.path().join("b.wav"), vec![0u8; 250]).expect("write");
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");

        assert_eq!(directory_size(dir.path()), 350);
    }
}
