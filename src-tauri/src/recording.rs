use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;
use zscribe_audio::{Level, RecordOptions, Session};
use zscribe_core::{
    clean_model_output, summary::extract_action_items, Plan, PromptOptions, Summary, TokenUsage,
    Transcript,
};
use zscribe_providers::{CompletionRequest, ProviderErrorInfo};
use zscribe_store::NewRecording;

use crate::events;
use crate::privacy;
use crate::state::AppState;
use crate::windows;

#[derive(Default)]
pub enum Pipeline {
    #[default]
    Idle,

    Recording {
        id: String,

        tracks: Vec<RecordedTrack>,
    },

    Working {
        id: String,
        cancel: Arc<AtomicBool>,
    },
}

struct Wanted {
    speaker: String,
    device_id: Option<String>,
    system_source: Option<String>,
}

pub struct RecordedTrack {
    pub speaker: String,
    pub session: Session,
    pub path: std::path::PathBuf,
}

impl Pipeline {
    pub fn is_recording(&self) -> bool {
        matches!(self, Pipeline::Recording { .. })
    }

    pub fn recording_id(&self) -> Option<&str> {
        match self {
            Pipeline::Recording { id, .. } | Pipeline::Working { id, .. } => Some(id),
            Pipeline::Idle => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecordingStatus {
    pub active: bool,
    pub paused: bool,
    pub duration_ms: u32,
    pub level: Level,

    pub problem: Option<String>,

    pub rewind_ms: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StageProgress {
    pub recording_id: String,

    pub stage: String,

    pub percent: u8,

    pub step: Option<u32>,
    pub steps: Option<u32>,

    pub on_this_machine: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PipelineFailure {
    pub recording_id: String,
    pub stage: String,
    pub error: ProviderErrorInfo,
}

pub fn status(app: &AppHandle) -> RecordingStatus {
    let state = app.state::<AppState>();
    let pipeline = state.pipeline.lock().expect("pipeline lock poisoned");

    match &*pipeline {
        Pipeline::Recording { tracks, .. } => {
            let Some(first) = tracks.first() else {
                return RecordingStatus::default();
            };

            let level = tracks.iter().map(|track| track.session.level()).fold(
                zscribe_audio::Level::default(),
                |best, next| {
                    if next.rms > best.rms {
                        next
                    } else {
                        best
                    }
                },
            );

            RecordingStatus {
                active: true,
                paused: first.session.is_paused(),
                duration_ms: tracks
                    .iter()
                    .map(|track| track.session.duration_ms())
                    .max()
                    .unwrap_or(0),
                level,

                problem: tracks.iter().find_map(|track| {
                    track.session.failure().map(|why| {
                        if track.speaker.is_empty() {
                            why
                        } else {
                            format!("{}: {why}", track.speaker)
                        }
                    })
                }),

                rewind_ms: 0,
            }
        }

        _ => RecordingStatus {
            rewind_ms: crate::rewind::buffered_ms(app),
            ..RecordingStatus::default()
        },
    }
}

pub fn toggle(app: AppHandle) {
    let recording = app
        .state::<AppState>()
        .pipeline
        .lock()
        .expect("pipeline lock poisoned")
        .is_recording();

    if recording {
        stop(app);
    } else if let Err(err) = start(&app) {
        tracing::error!(%err, "could not start recording");
        crate::feedback::notify_failure(&app, "Recording could not start", &err.to_string());
    }
}

pub fn start(app: &AppHandle) -> Result<String, zscribe_audio::AudioError> {
    let state = app.state::<AppState>();
    let settings = state.settings();

    {
        let pipeline = state.pipeline.lock().expect("pipeline lock poisoned");
        if pipeline.is_recording() {
            return Ok(pipeline.recording_id().unwrap_or_default().to_owned());
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let output = state.paths.audio_file(&id);
    let started_at = time::OffsetDateTime::now_utc().unix_timestamp();

    let single_source = settings
        .recording
        .sources
        .iter()
        .filter(|source| source.enabled)
        .count()
        <= 1;

    let buffered = crate::rewind::take(app);
    let mut preroll = if single_source { buffered } else { Vec::new() };

    if settings.recording.announce_tone {
        zscribe_audio::play_start_tone();
    }

    let wanted: Vec<Wanted> = {
        let named: Vec<&zscribe_store::SourceProfile> = settings
            .recording
            .sources
            .iter()
            .filter(|source| source.enabled)
            .collect();

        if named.is_empty() {
            vec![Wanted {
                speaker: String::new(),
                device_id: settings.recording.input_device.clone(),
                system_source: None,
            }]
        } else {
            named
                .iter()
                .map(|source| match source.kind {
                    zscribe_store::SourceKind::SystemAudio => Wanted {
                        speaker: source.name.clone(),
                        device_id: None,
                        system_source: Some(source.device_id.clone()),
                    },
                    zscribe_store::SourceKind::Microphone => Wanted {
                        speaker: source.name.clone(),
                        device_id: Some(source.device_id.clone()),
                        system_source: None,
                    },
                })
                .collect()
        }
    };

    let multi = wanted.len() > 1;
    let mut tracks: Vec<RecordedTrack> = Vec::with_capacity(wanted.len());

    for (index, want) in wanted.into_iter().enumerate() {
        let path = if multi {
            state.paths.track_file(&id, index)
        } else {
            output.clone()
        };

        match Session::start(RecordOptions {
            device_id: want.device_id,
            system_source: want.system_source,

            exact_device: !want.speaker.is_empty(),
            output: path.clone(),

            preroll: if index == 0 {
                preroll.clone()
            } else {
                Vec::new()
            },
        }) {
            Ok(session) => {
                if index == 0 {
                    preroll = Vec::new();
                }
                tracks.push(RecordedTrack {
                    speaker: want.speaker,
                    session,
                    path,
                });
            }
            Err(err) => {
                tracing::error!(%err, speaker = %want.speaker, "a source could not be opened");
                crate::feedback::notify_failure(
                    app,
                    &format!(
                        "{} could not be recorded",
                        if want.speaker.is_empty() {
                            "The microphone"
                        } else {
                            &want.speaker
                        }
                    ),
                    &format!("{err} Recording continues with the other sources."),
                );
            }
        }
    }

    if tracks.is_empty() {
        crate::rewind::restore(app, std::mem::take(&mut preroll));

        let named: Vec<String> = settings
            .recording
            .sources
            .iter()
            .filter(|source| source.enabled && !source.name.is_empty())
            .map(|source| source.name.clone())
            .collect();

        if !named.is_empty() {
            return Err(zscribe_audio::AudioError::Device(format!(
                "none of the configured sources could be opened ({}). Check them in Audio \
                 sources — a device that has been unplugged or is in use by another \
                 application cannot be recorded",
                named.join(", ")
            )));
        }
        return Err(zscribe_audio::AudioError::NoInputDevice);
    }

    let output = tracks[0].path.clone();

    let row = NewRecording {
        id: id.clone(),
        started_at,
        duration_ms: 0,
        source: settings.recording.source_label(),
        template_id: settings.template_id.clone(),
        title: default_title(started_at),
        audio_path: Some(output.display().to_string()),
    };

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .insert(&row)
    {
        tracing::error!(%err, "could not record the new session in the database");
    }

    tracing::info!(recording = %id, microphones = tracks.len(), "recording started");

    *state.pipeline.lock().expect("pipeline lock poisoned") = Pipeline::Recording {
        id: id.clone(),
        tracks,
    };

    crate::tray::set_recording(app, true);
    windows::show_recorder(app);
    let _ = app.emit(events::RECORDING_STARTED, &id);

    spawn_level_ticker(app.clone());

    if settings.transcription.live_transcript {
        spawn_live_transcriber(app.clone(), id.clone());
    }

    Ok(id)
}

pub fn pause(app: &AppHandle) {
    if let Pipeline::Recording { tracks, .. } = &*app
        .state::<AppState>()
        .pipeline
        .lock()
        .expect("pipeline lock poisoned")
    {
        for track in tracks {
            track.session.pause();
        }
    }
}

pub fn resume(app: &AppHandle) {
    if let Pipeline::Recording { tracks, .. } = &*app
        .state::<AppState>()
        .pipeline
        .lock()
        .expect("pipeline lock poisoned")
    {
        for track in tracks {
            track.session.resume();
        }
    }
}

fn resume_listening(app: &AppHandle) {
    crate::rewind::reconcile(app);
}

pub fn stop(app: AppHandle) {
    let state = app.state::<AppState>();

    let taken = {
        let mut pipeline = state.pipeline.lock().expect("pipeline lock poisoned");
        match std::mem::take(&mut *pipeline) {
            Pipeline::Recording { id, tracks } => Some((id, tracks)),
            other => {
                *pipeline = other;
                None
            }
        }
    };

    let Some((id, tracks)) = taken else { return };

    crate::tray::set_recording(&app, false);

    let mut finished: Vec<FinishedTrack> = Vec::with_capacity(tracks.len());
    for track in tracks {
        let speaker = track.speaker.clone();
        let hiccups = track.session.hiccups();
        if hiccups > 0 {
            tracing::warn!(
                %speaker,
                hiccups,
                "the recording had audio glitches; a few fractions of a second may be missing"
            );
        }

        match track.session.stop() {
            Ok(done) => finished.push(FinishedTrack {
                speaker,
                path: done.path,
                duration_ms: done.duration_ms,
            }),
            Err(err) => {
                tracing::error!(%err, recording = %id, %speaker, "a track could not be closed");
            }
        }
    }

    resume_listening(&app);

    let Some(longest) = finished.iter().map(|t| t.duration_ms).max() else {
        crate::feedback::notify_failure(
            &app,
            "Recording failed",
            "No microphone produced a usable file.",
        );
        return;
    };

    tracing::info!(
        recording = %id,
        duration_ms = longest,
        tracks = finished.len(),
        "recording stopped"
    );

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .set_duration(&id, longest)
    {
        tracing::error!(%err, "could not save the recording's duration");
    }

    let _ = app.emit(events::RECORDING_STOPPED, &id);

    let cancel = Arc::new(AtomicBool::new(false));
    *state.pipeline.lock().expect("pipeline lock poisoned") = Pipeline::Working {
        id: id.clone(),
        cancel: Arc::clone(&cancel),
    };

    spawn_processing(app.clone(), id, finished, cancel);
}

pub fn busy_with(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let pipeline = state.pipeline.lock().expect("pipeline lock poisoned");

    match &*pipeline {
        Pipeline::Idle => None,
        Pipeline::Recording { .. } => Some("Stop the recording first, then import.".to_owned()),
        Pipeline::Working { .. } => {
            Some("Wait for the current transcript to finish, then try again.".to_owned())
        }
    }
}

pub fn process_existing(app: AppHandle, id: String, path: std::path::PathBuf, duration_ms: u32) {
    let state = app.state::<AppState>();

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .set_duration(&id, duration_ms)
    {
        tracing::error!(%err, "could not save the imported duration");
    }

    let cancel = Arc::new(AtomicBool::new(false));
    *state.pipeline.lock().expect("pipeline lock poisoned") = Pipeline::Working {
        id: id.clone(),
        cancel: Arc::clone(&cancel),
    };

    let track = FinishedTrack {
        speaker: String::new(),
        path: path.display().to_string(),
        duration_ms,
    };

    spawn_processing(app, id, vec![track], cancel);
}

#[derive(Debug, Clone)]
pub struct FinishedTrack {
    pub speaker: String,
    pub path: String,
    pub duration_ms: u32,
}

pub fn resummarise(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let transcript = {
            let state = app.state::<AppState>();
            let stored = state
                .recordings
                .lock()
                .expect("recordings lock poisoned")
                .transcript(&id);

            match stored {
                Ok(Some(transcript)) => transcript,
                Ok(None) => {
                    tracing::warn!(recording = %id, "cannot summarise: there is no transcript yet");
                    return;
                }
                Err(err) => {
                    tracing::error!(%err, "could not read the transcript");
                    return;
                }
            }
        };

        if !transcript.has_speech() {
            let _ = app.emit(
                events::PIPELINE_FAILED,
                &PipelineFailure {
                    recording_id: id.clone(),
                    stage: "summarising".to_owned(),
                    error: ProviderErrorInfo {
                        code: "no_speech".to_owned(),
                        message: "No speech was found in this recording".to_owned(),
                        remedy: "There is nothing here to summarise. Transcribe it again with a \
                                 larger model if you believe something was said."
                            .to_owned(),
                        retryable: false,
                    },
                },
            );
            return;
        }

        if let Err(err) = run_summary(&app, &id, &transcript).await {
            let _ = app.emit(
                events::PIPELINE_FAILED,
                &PipelineFailure {
                    recording_id: id.clone(),
                    stage: "summarising".to_owned(),
                    error: err,
                },
            );
        }
        crate::sync::write_note(&app, &id);
        let _ = app.emit(events::RECORDING_READY, &id);
    });
}

pub fn retranscribe(app: AppHandle, id: String) -> Result<(), zscribe_stt::SttError> {
    let audio_path = {
        let state = app.state::<AppState>();
        let detail = state
            .recordings
            .lock()
            .expect("recordings lock poisoned")
            .get(&id)
            .ok()
            .flatten();

        detail
            .and_then(|detail| detail.recording.audio_path)
            .ok_or(zscribe_stt::SttError::EmptyAudio)?
    };

    let cancel = Arc::new(AtomicBool::new(false));
    *app.state::<AppState>()
        .pipeline
        .lock()
        .expect("pipeline lock poisoned") = Pipeline::Working {
        id: id.clone(),
        cancel: Arc::clone(&cancel),
    };

    spawn_processing(
        app,
        id,
        vec![FinishedTrack {
            speaker: String::new(),
            path: audio_path,
            duration_ms: 0,
        }],
        cancel,
    );
    Ok(())
}

pub fn cancel(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.cancel_in_flight();

    let mut pipeline = state.pipeline.lock().expect("pipeline lock poisoned");
    if let Pipeline::Working { cancel, .. } = &*pipeline {
        cancel.store(true, Ordering::Relaxed);
    }
    if matches!(*pipeline, Pipeline::Working { .. }) {
        *pipeline = Pipeline::Idle;
    }
}

fn spawn_level_ticker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(zscribe_audio::LEVEL_INTERVAL).await;

            let status = status(&app);
            if !status.active {
                break;
            }

            let _ = app.emit(events::RECORDING_LEVEL, &status);

            if let Some(problem) = &status.problem {
                tracing::error!(%problem, "the microphone stopped during recording");
                crate::feedback::notify_failure(
                    &app,
                    "The microphone stopped",
                    "Recording ended early. What was captured has been kept.",
                );
                stop(app.clone());
                break;
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveTranscript {
    pub recording_id: String,

    pub text: String,

    pub working: bool,
}

const LIVE_INTERVAL: Duration = Duration::from_secs(6);

fn spawn_live_transcriber(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let (model_id, language, use_gpu) = {
            let state = app.state::<AppState>();
            let settings = state.settings();
            (
                settings.transcription.model_id.clone(),
                settings.transcription.language.clone(),
                settings.transcription.use_gpu,
            )
        };

        let (models_dir, threads, accelerated) = {
            let state = app.state::<AppState>();
            let dir = state.paths.models_dir();
            let machine = zscribe_platform::Machine::probe(&dir);
            (dir, machine.whisper_threads(), machine.can_accelerate())
        };

        let Ok(options) = zscribe_stt::options_for(
            &model_id,
            &models_dir,
            &language,
            threads,
            use_gpu && accelerated,
        ) else {
            tracing::warn!("live transcript is on but no model is installed");
            return;
        };

        loop {
            tokio::time::sleep(LIVE_INTERVAL).await;

            let samples = {
                let state = app.state::<AppState>();
                let pipeline = state.pipeline.lock().expect("pipeline lock poisoned");

                match &*pipeline {
                    Pipeline::Recording {
                        id: current,
                        tracks,
                    } if current == &id => tracks.first().map(|track| track.session.recent_audio()),
                    _ => None,
                }
            };

            let Some(samples) = samples else { break };
            if samples.len() < zscribe_audio::SAMPLE_RATE as usize {
                continue;
            }

            if zscribe_audio::Level::of(&samples).is_silent() {
                continue;
            }

            let _ = app.emit(
                events::LIVE_TRANSCRIPT,
                &LiveTranscript {
                    recording_id: id.clone(),
                    text: String::new(),
                    working: true,
                },
            );

            let options = options.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                zscribe_stt::transcribe(
                    &samples,
                    &options,
                    Arc::new(AtomicBool::new(false)),
                    |_| {},
                )
            })
            .await;

            match result {
                Ok(Ok(transcript)) => {
                    let _ = app.emit(
                        events::LIVE_TRANSCRIPT,
                        &LiveTranscript {
                            recording_id: id.clone(),
                            text: transcript.text(),
                            working: false,
                        },
                    );
                }

                Ok(Err(zscribe_stt::SttError::EmptyAudio)) => {}
                Ok(Err(err)) => tracing::debug!(%err, "live transcription pass failed"),
                Err(err) => tracing::debug!(%err, "the live transcription thread panicked"),
            }
        }

        tracing::debug!(recording = %id, "live transcript stopped");
    });
}

fn spawn_processing(
    app: AppHandle,
    id: String,
    tracks: Vec<FinishedTrack>,
    cancel: Arc<AtomicBool>,
) {
    tauri::async_runtime::spawn(async move {
        let transcript = match run_tracks(&app, &id, &tracks, Arc::clone(&cancel)).await {
            Ok(transcript) => transcript,
            Err(()) => {
                clear_working(&app, &id);
                return;
            }
        };

        let transcript = name_the_voices(&app, &id, &tracks, transcript);

        let has_speech = transcript.has_speech();
        let transcript = with_consent_note(&app, transcript);

        {
            let state = app.state::<AppState>();
            let saved = state
                .recordings
                .lock()
                .expect("recordings lock poisoned")
                .set_transcript(&id, &transcript);

            if let Err(err) = saved {
                tracing::error!(%err, "could not save the transcript");
            }
        }
        let _ = app.emit(events::RECORDING_READY, &id);

        drop_audio_if_asked(&app, &id);

        if !has_speech {
            tracing::info!(recording = %id, "no speech in this recording; not summarising it");
            let _ = app.emit(
                events::PIPELINE_FAILED,
                &PipelineFailure {
                    recording_id: id.clone(),
                    stage: "summarising".to_owned(),
                    error: ProviderErrorInfo {
                        code: "no_speech".to_owned(),
                        message: "No speech was found in this recording".to_owned(),
                        remedy: "Check that the right microphone is selected in Audio sources, \
                                 and watch the waveform on the recording bar — it moves when \
                                 sound is arriving."
                            .to_owned(),
                        retryable: false,
                    },
                },
            );
            clear_working(&app, &id);
            crate::sync::write_note(&app, &id);
            let _ = app.emit(events::RECORDING_READY, &id);
            return;
        }

        if let Err(err) = run_summary(&app, &id, &transcript).await {
            tracing::warn!(recording = %id, error = %err.message, "summarising failed");
            let _ = app.emit(
                events::PIPELINE_FAILED,
                &PipelineFailure {
                    recording_id: id.clone(),
                    stage: "summarising".to_owned(),
                    error: err,
                },
            );
        }

        clear_working(&app, &id);

        crate::sync::write_note(&app, &id);

        let _ = app.emit(events::RECORDING_READY, &id);
    });
}

fn name_the_voices(
    app: &AppHandle,
    id: &str,
    tracks: &[FinishedTrack],
    mut transcript: Transcript,
) -> Transcript {
    let state = app.state::<AppState>();
    if !state.settings().transcription.detect_speakers {
        return transcript;
    }

    let [track] = tracks else {
        return transcript;
    };

    let started = std::time::Instant::now();
    let heard = zscribe_stt::label_speakers(
        std::path::Path::new(&track.path),
        &mut transcript,
        &zscribe_core::voices::VoiceOptions::default(),
    );

    tracing::info!(
        recording = %id,
        speakers = heard.speakers,
        unattributed = heard.unattributed,
        took_ms = started.elapsed().as_millis(),
        "listened for separate voices"
    );

    transcript
}

async fn run_tracks(
    app: &AppHandle,
    id: &str,
    tracks: &[FinishedTrack],
    cancel: Arc<AtomicBool>,
) -> Result<Transcript, ()> {
    let named = tracks.iter().any(|track| !track.speaker.is_empty());

    if tracks.len() == 1 && !named {
        return run_transcription(app, id, &tracks[0].path, cancel).await;
    }

    let mut merged: Vec<zscribe_core::Track> = Vec::with_capacity(tracks.len());

    for track in tracks {
        let transcript = run_transcription(app, id, &track.path, Arc::clone(&cancel)).await?;

        let windows: Vec<(u32, u32)> = transcript
            .segments
            .iter()
            .map(|segment| (segment.start_ms, segment.end_ms))
            .collect();

        let levels = match zscribe_audio::read_mono(std::path::Path::new(&track.path)) {
            Ok(samples) => {
                zscribe_audio::levels_for(&samples, &windows, zscribe_audio::SAMPLE_RATE)
            }
            Err(err) => {
                tracing::warn!(%err, speaker = %track.speaker, "could not measure track levels");
                Vec::new()
            }
        };

        merged.push(zscribe_core::Track {
            speaker: track.speaker.clone(),
            transcript,
            levels,
        });
    }

    let transcript = zscribe_core::merge_tracks(&merged);

    tracing::info!(
        recording = %id,
        tracks = merged.len(),
        speakers = ?zscribe_core::speakers(&transcript),
        segments = transcript.segments.len(),
        "tracks merged"
    );

    Ok(transcript)
}

async fn run_transcription(
    app: &AppHandle,
    id: &str,
    audio_path: &str,
    cancel: Arc<AtomicBool>,
) -> Result<Transcript, ()> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let models_dir = state.paths.models_dir();

    let machine = zscribe_platform::Machine::probe(&models_dir);
    let threads = machine.whisper_threads();
    let use_gpu = settings.transcription.use_gpu && machine.can_accelerate();

    let options = match zscribe_stt::options_for(
        &settings.transcription.model_id,
        &models_dir,
        &settings.transcription.language,
        threads,
        use_gpu,
    ) {
        Ok(options) => options,
        Err(err) => {
            report_stt_failure(app, id, &err);
            return Err(());
        }
    };

    let progress_app = app.clone();
    let progress_id = id.to_owned();
    let audio_path = audio_path.to_owned();

    let result = tauri::async_runtime::spawn_blocking(move || {
        zscribe_stt::transcribe_file(
            std::path::Path::new(&audio_path),
            &options,
            cancel,
            move |percent| {
                let _ = progress_app.emit(
                    events::TRANSCRIBE_PROGRESS,
                    &StageProgress {
                        recording_id: progress_id.clone(),
                        stage: "transcribing".to_owned(),
                        percent,
                        step: None,
                        steps: None,

                        on_this_machine: true,
                    },
                );
            },
        )
    })
    .await;

    match result {
        Ok(Ok(transcript)) => Ok(transcript),
        Ok(Err(err)) => {
            if !matches!(err, zscribe_stt::SttError::Cancelled) {
                report_stt_failure(app, id, &err);
            }
            Err(())
        }
        Err(err) => {
            tracing::error!(%err, "the transcription thread panicked");
            Err(())
        }
    }
}

async fn run_summary(
    app: &AppHandle,
    id: &str,
    transcript: &Transcript,
) -> Result<(), ProviderErrorInfo> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let profile = settings.active_provider_profile();
    let template = settings.active_template();

    let api_key = state.secrets.get(profile.id.slug()).ok().flatten();
    let provider = zscribe_providers::build(&profile, api_key)?;

    let redaction = privacy::wanted(&settings, profile.id, transcript);
    let (redacted, removed) = zscribe_core::redact_transcript(transcript, &redaction);
    if removed.total() > 0 {
        tracing::info!(
            recording = %id,
            provider = %profile.id.slug(),
            emails = removed.emails,
            numbers = removed.numbers,
            names = removed.names,
            "redacted details before sending the transcript"
        );
    }
    let transcript = &redacted;

    let summary_language = match settings.summary_language.trim() {
        "" | "auto" => None,
        language => Some(language.to_owned()),
    };

    let plan = zscribe_core::prompt::plan(
        &template,
        transcript,
        PromptOptions {
            timestamps: settings.transcription.timestamps,
            language: summary_language,
            ..PromptOptions::default()
        },
    );

    let steps = zscribe_core::prompt::request_count(&plan) as u32;
    let token = state.begin_request();
    let started = std::time::Instant::now();

    let local_model = profile.id == zscribe_core::ProviderId::Ollama;

    let emit_step = |step: u32| {
        let _ = app.emit(
            events::SUMMARIZE_PROGRESS,
            &StageProgress {
                recording_id: id.to_owned(),
                stage: "summarising".to_owned(),
                percent: ((step.saturating_sub(1) * 100) / steps.max(1)) as u8,
                step: Some(step),
                steps: Some(steps),
                on_this_machine: local_model,
            },
        );
    };

    let mut usage = TokenUsage::default();

    let body = match plan {
        Plan::Single(prompt) => {
            emit_step(1);
            let completion = provider
                .complete(&CompletionRequest::new(&profile.model, prompt), &token)
                .await
                .inspect_err(|_| state.finish_request(&token))?;
            usage = usage.merge(completion.usage);
            completion.text
        }

        Plan::MapReduce { parts, reduce } => {
            tracing::info!(recording = %id, requests = steps, "the recording is long enough to need chunking");

            let mut partials = Vec::with_capacity(parts.len());
            for (index, prompt) in parts.into_iter().enumerate() {
                emit_step(index as u32 + 1);

                let completion = provider
                    .complete(&CompletionRequest::new(&profile.model, prompt), &token)
                    .await
                    .inspect_err(|_| state.finish_request(&token))?;
                usage = usage.merge(completion.usage);
                partials.push(clean_model_output(&completion.text));
            }

            emit_step(steps);
            let completion = provider
                .complete(
                    &CompletionRequest::new(&profile.model, reduce.build(&partials)),
                    &token,
                )
                .await
                .inspect_err(|_| state.finish_request(&token))?;
            usage = usage.merge(completion.usage);
            completion.text
        }
    };

    state.finish_request(&token);

    let body_md = clean_model_output(&body);
    let summary = Summary {
        provider: profile.id.slug().to_owned(),
        model: profile.model.clone(),
        template_id: template.id.clone(),
        action_items: extract_action_items(&body_md),
        body_md,
        usage,
        elapsed_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        redacted: removed.total().min(u32::MAX as usize) as u32,
    };

    tracing::info!(
        recording = %id,
        provider = profile.id.slug(),
        model = %profile.model,
        tokens = usage.total(),
        action_items = summary.action_items.len(),
        elapsed_ms = summary.elapsed_ms,
        "summary ready"
    );

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .set_summary(id, &summary)
    {
        tracing::error!(%err, "could not save the summary");
    }

    if let Some(title) = title_from(&summary.body_md) {
        let recordings = state.recordings.lock().expect("recordings lock poisoned");

        let untouched = recordings.get(id).ok().flatten().is_some_and(|detail| {
            detail.recording.title == default_title(detail.recording.started_at)
        });

        if untouched {
            let _ = recordings.set_title(id, &title);
        }
    }

    Ok(())
}

pub const CONSENT_NOTE: &str =
    "[Recorded with the agreement of everyone present, as stated by the person recording.]";

fn with_consent_note(app: &AppHandle, transcript: Transcript) -> Transcript {
    if !app.state::<AppState>().settings().recording.consent_note {
        return transcript;
    }

    let mut segments = Vec::with_capacity(transcript.segments.len() + 1);
    segments.push(zscribe_core::Segment::new(0, 0, CONSENT_NOTE));
    segments.extend(transcript.segments);

    Transcript {
        segments,
        ..transcript
    }
}

fn drop_audio_if_asked(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    if state.settings().recording.keep_audio {
        return;
    }

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .forget_audio(id)
    {
        tracing::warn!(%err, "could not clear the audio path");
        return;
    }

    let mut removed = 0;
    for path in state.paths.audio_files(id) {
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(err) => tracing::warn!(%err, path = %path.display(), "could not delete the audio"),
        }
    }

    if removed > 0 {
        tracing::info!(recording = %id, files = removed, "audio deleted; the transcript was kept");
    }
}

fn clear_working(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let mut pipeline = state.pipeline.lock().expect("pipeline lock poisoned");

    if pipeline.recording_id() == Some(id) && !pipeline.is_recording() {
        *pipeline = Pipeline::Idle;
        drop(pipeline);
        windows::hide_recorder(app);
    }
}

fn report_stt_failure(app: &AppHandle, id: &str, err: &zscribe_stt::SttError) {
    tracing::warn!(recording = %id, error = %err, "transcription failed");

    let _ = app.emit(
        events::PIPELINE_FAILED,
        &PipelineFailure {
            recording_id: id.to_owned(),
            stage: "transcribing".to_owned(),
            error: ProviderErrorInfo {
                code: err.code().to_owned(),
                message: err.to_string(),
                remedy: err.remedy(),
                retryable: false,
            },
        },
    );
}

fn default_title(started_at: i64) -> String {
    let when = time::OffsetDateTime::from_unix_timestamp(started_at)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(crate::local_offset());

    format!(
        "Recording, {} {} {:02}:{:02}",
        when.day(),
        when.month(),
        when.hour(),
        when.minute()
    )
}

pub fn title_from(body_md: &str) -> Option<String> {
    const MAX: usize = 70;

    for line in body_md.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(heading) = line.strip_prefix('#') {
            let heading = heading.trim_start_matches('#').trim();
            let lowered = heading.to_lowercase();

            if heading.is_empty()
                || ["summary", "decisions", "thoughts", "themes", "needs"]
                    .contains(&lowered.as_str())
            {
                continue;
            }
            return Some(truncate(heading, MAX));
        }

        let body = line.trim_start_matches(['-', '*', '>', ' ']);
        let sentence = body
            .split_once(". ")
            .map(|(first, _)| first)
            .unwrap_or(body)
            .trim();

        if sentence.len() >= 12 {
            return Some(truncate(sentence, MAX));
        }
    }
    None
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.trim_end_matches(['.', ':']).to_owned();
    }

    let mut out: String = text.chars().take(limit).collect();

    if let Some(space) = out.rfind(' ') {
        out.truncate(space);
    }
    format!("{}…", out.trim_end_matches([',', '.', ':', ';']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_pipeline_is_neither_recording_nor_named() {
        let pipeline = Pipeline::default();
        assert!(!pipeline.is_recording());
        assert_eq!(pipeline.recording_id(), None);
    }

    #[test]
    fn a_working_pipeline_still_knows_which_recording_it_belongs_to() {
        let pipeline = Pipeline::Working {
            id: "abc".to_owned(),
            cancel: Arc::new(AtomicBool::new(false)),
        };
        assert!(!pipeline.is_recording());
        assert_eq!(pipeline.recording_id(), Some("abc"));
    }

    #[test]
    fn a_title_comes_from_the_summarys_own_heading() {
        let body = "# Q3 planning with Anna\n\n## Decisions\n\n- Ship on Friday";
        assert_eq!(title_from(body).as_deref(), Some("Q3 planning with Anna"));
    }

    #[test]
    fn boilerplate_section_names_are_not_used_as_titles() {
        let body = "## Summary\n\nAnna and Ben agreed the Q3 launch date.\n";
        assert_eq!(
            title_from(body).as_deref(),
            Some("Anna and Ben agreed the Q3 launch date")
        );
    }

    #[test]
    fn a_summary_with_no_heading_uses_its_first_sentence() {
        let body = "Anna and Ben agreed the launch date. Ben will send the contract.";
        assert_eq!(
            title_from(body).as_deref(),
            Some("Anna and Ben agreed the launch date")
        );
    }

    #[test]
    fn a_bullet_can_supply_the_title_when_nothing_else_does() {
        let body = "- Agreed the Q3 launch date with Anna\n- Something else";
        assert_eq!(
            title_from(body).as_deref(),
            Some("Agreed the Q3 launch date with Anna")
        );
    }

    #[test]
    fn a_long_title_is_cut_on_a_word_boundary() {
        let body = format!("# {}", "word ".repeat(40));
        let title = title_from(&body).expect("a title");

        assert!(
            title.chars().count() <= 71,
            "got {} chars",
            title.chars().count()
        );
        assert!(title.ends_with('…'));
        assert!(!title.contains("wor…"), "must not cut mid-word: {title}");
    }

    #[test]
    fn a_summary_with_nothing_usable_yields_no_title() {
        assert_eq!(title_from(""), None);
        assert_eq!(title_from("## Summary\n\n## Decisions\n"), None);
        assert_eq!(title_from("- ok\n"), None, "too short to be a title");
    }

    #[test]
    fn trailing_punctuation_is_dropped_from_a_title() {
        assert_eq!(title_from("# Q3 planning:").as_deref(), Some("Q3 planning"));
    }

    #[test]
    fn the_default_title_carries_the_date_so_it_is_never_blank() {
        let title = default_title(1_754_575_320);
        assert!(title.starts_with("Recording, "), "got: {title}");
        assert!(title.len() > "Recording, ".len());
    }

    #[test]
    fn the_consent_note_does_not_claim_more_than_the_app_can_know() {
        assert!(CONSENT_NOTE.contains("as stated by the person recording"));
    }

    #[test]
    fn the_consent_note_is_long_enough_to_mask_a_silent_recording() {
        let with_note = Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments: vec![
                zscribe_core::Segment::new(0, 0, CONSENT_NOTE),
                zscribe_core::Segment::new(0, 9_000, "."),
            ],
        };

        assert!(
            with_note.has_speech(),
            "the note alone passes the speech check, so the check must run first"
        );
    }

    #[test]
    fn an_idle_status_reports_silence_rather_than_a_stale_level() {
        let status = RecordingStatus::default();
        assert!(!status.active);
        assert_eq!(status.level, Level::default());
        assert_eq!(status.duration_ms, 0);
    }
}
