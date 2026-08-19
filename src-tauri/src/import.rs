use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager};
use zscribe_store::NewRecording;

use crate::commands::CommandError;
use crate::downloader;
use crate::events;
use crate::recording::{self, StageProgress};
use crate::state::AppState;

pub const IMPORTING: &str = "importing";

pub fn file(app: &AppHandle, source: PathBuf) -> Result<String, CommandError> {
    let state = app.state::<AppState>();

    if let Some(busy) = recording::busy_with(app) {
        return Err(CommandError::new(
            "pipeline_busy",
            "ZScribe is already working on a recording",
            &busy,
        ));
    }

    let info = zscribe_audio::inspect(&source).map_err(to_command_error)?;
    tracing::info!(file = %source.display(), codec = %info.codec, "importing");

    let id = uuid::Uuid::new_v4().to_string();
    let destination = state.paths.audio_file(&id);
    let started_at = time::OffsetDateTime::now_utc().unix_timestamp();

    let title = title_for(&source);
    let row = NewRecording {
        id: id.clone(),
        started_at,
        duration_ms: info.duration_ms.unwrap_or(0),
        source: source_label(&source),
        template_id: template_of(app),
        title,
        audio_path: Some(destination.display().to_string()),
    };

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .insert(&row)
    {
        tracing::error!(%err, "could not create the imported recording");
        return Err(CommandError::new(
            "storage",
            "the recording could not be created",
            "Check that ZScribe's data directory exists and has free space.",
        ));
    }

    let _ = app.emit(events::RECORDING_STARTED, &id);

    let progress_app = app.clone();
    let progress_id = id.clone();
    let decoded = zscribe_audio::decode_to_wav(&source, &destination, move |percent| {
        let _ = progress_app.emit(
            events::TRANSCRIBE_PROGRESS,
            &StageProgress {
                recording_id: progress_id.clone(),
                stage: IMPORTING.to_owned(),
                percent,
                step: None,
                steps: None,
                on_this_machine: true,
            },
        );
    });

    let duration_ms = match decoded {
        Ok(duration_ms) => duration_ms,
        Err(err) => {
            let _ = state
                .recordings
                .lock()
                .expect("recordings lock poisoned")
                .delete(&id);
            let _ = std::fs::remove_file(&destination);
            let _ = app.emit(events::RECORDING_READY, &id);

            return Err(to_command_error(err));
        }
    };

    recording::process_existing(app.clone(), id.clone(), destination, duration_ms);
    Ok(id)
}

pub fn link(app: &AppHandle, url: String) -> Result<String, CommandError> {
    let state = app.state::<AppState>();

    if let Some(busy) = recording::busy_with(app) {
        return Err(CommandError::new(
            "pipeline_busy",
            "ZScribe is already working on a recording",
            &busy,
        ));
    }

    let tools = tools_dir(app);
    let info = downloader::inspect(&url, Some(&tools))
        .map_err(|err| from_download_error(err, Some(&tools)))?;
    tracing::info!(site = %info.site, title = %info.title, "importing a link");

    let id = uuid::Uuid::new_v4().to_string();
    let destination = state.paths.audio_file(&id);
    let started_at = time::OffsetDateTime::now_utc().unix_timestamp();

    let row = NewRecording {
        id: id.clone(),
        started_at,
        duration_ms: info.duration_ms.unwrap_or(0),
        source: format!("Imported from {}", info.site),
        template_id: template_of(app),
        title: info.title.clone(),
        audio_path: Some(destination.display().to_string()),
    };

    if let Err(err) = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .insert(&row)
    {
        tracing::error!(%err, "could not create the imported recording");
        return Err(CommandError::new(
            "storage",
            "the recording could not be created",
            "Check that ZScribe's data directory exists and has free space.",
        ));
    }

    let _ = app.emit(events::RECORDING_STARTED, &id);

    let temporary = destination.with_extension("download");

    let result = fetch_and_decode(app, &id, &url, &temporary, &destination);

    let _ = std::fs::remove_file(&temporary);

    match result {
        Ok(duration_ms) => {
            recording::process_existing(app.clone(), id.clone(), destination, duration_ms);
            Ok(id)
        }
        Err(err) => {
            let _ = state
                .recordings
                .lock()
                .expect("recordings lock poisoned")
                .delete(&id);
            let _ = std::fs::remove_file(&destination);
            let _ = app.emit(events::RECORDING_READY, &id);
            Err(err)
        }
    }
}

fn fetch_and_decode(
    app: &AppHandle,
    id: &str,
    url: &str,
    temporary: &Path,
    destination: &Path,
) -> Result<u32, CommandError> {
    const DOWNLOAD_SHARE: f32 = 0.9;

    let report = |percent: u8| {
        let _ = app.emit(
            events::TRANSCRIBE_PROGRESS,
            &StageProgress {
                recording_id: id.to_owned(),
                stage: IMPORTING.to_owned(),
                percent,
                step: None,
                steps: None,
                on_this_machine: true,
            },
        );
    };

    let tools = tools_dir(app);
    downloader::fetch(url, temporary, Some(&tools), |percent| {
        report((percent as f32 * DOWNLOAD_SHARE) as u8);
    })
    .map_err(|err| from_download_error(err, Some(&tools)))?;

    zscribe_audio::decode_to_wav(temporary, destination, |percent| {
        report((90.0 + percent as f32 * (1.0 - DOWNLOAD_SHARE)) as u8);
    })
    .map_err(to_command_error)
}

fn from_download_error(err: downloader::DownloadError, tools: Option<&Path>) -> CommandError {
    let code = match err {
        downloader::DownloadError::NotInstalled => "yt_dlp_missing",
        downloader::DownloadError::Rejected(_) => "link_rejected",
        downloader::DownloadError::Failed(_) => "download_failed",
        downloader::DownloadError::Unusable(_) => "yt_dlp_unusable",
    };

    let remedy = err.remedy(tools);
    CommandError::new(code, err.to_string(), remedy)
}

fn title_for(source: &Path) -> String {
    source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Imported recording")
        .to_owned()
}

fn source_label(source: &Path) -> String {
    match source.extension().and_then(|ext| ext.to_str()) {
        Some(extension) => format!("Imported {}", extension.to_lowercase()),
        None => "Imported file".to_owned(),
    }
}

pub fn tools_dir(app: &AppHandle) -> PathBuf {
    app.state::<AppState>().paths.tools_dir()
}

fn template_of(app: &AppHandle) -> String {
    app.state::<AppState>()
        .settings
        .lock()
        .expect("settings lock poisoned")
        .template_id
        .clone()
}

fn to_command_error(err: zscribe_audio::AudioError) -> CommandError {
    let code = match err {
        zscribe_audio::AudioError::NoAudioTrack(_) => "no_audio_track",
        zscribe_audio::AudioError::Read { .. } => "unreadable",
        _ => "undecodable",
    };

    CommandError::new(code, err.to_string(), err.remedy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_becomes_the_title() {
        assert_eq!(
            title_for(Path::new("/a/b/Team standup.mp4")),
            "Team standup"
        );
        assert_eq!(
            title_for(Path::new("interview.part2.m4a")),
            "interview.part2"
        );
    }

    #[test]
    fn a_file_with_no_usable_name_still_gets_one() {
        assert_eq!(title_for(Path::new("/")), "Imported recording");
        assert_eq!(title_for(Path::new("   .mp3")), "Imported recording");
    }

    #[test]
    fn the_source_says_it_was_imported_and_from_what() {
        assert_eq!(source_label(Path::new("talk.MP4")), "Imported mp4");
        assert_eq!(source_label(Path::new("recording")), "Imported file");
    }
}
