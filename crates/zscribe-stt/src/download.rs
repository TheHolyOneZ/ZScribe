use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::models::{find, ModelSpec};

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("unknown model '{0}'")]
    UnknownModel(String),

    #[error("could not reach the model server: {0}")]
    Network(String),

    #[error("the model server answered with HTTP {0}")]
    Server(u16),

    #[error("could not write {path}: {message}")]
    Write { path: String, message: String },

    #[error("the downloaded file does not match the published checksum, so it has been discarded")]
    ChecksumMismatch,

    #[error("the download was {got} bytes but should be {expected}")]
    SizeMismatch { got: u64, expected: u64 },

    #[error("not enough space: {needed_mb} MB is required and {free_mb} MB is free")]
    NotEnoughSpace { needed_mb: u64, free_mb: u64 },

    #[error("cancelled")]
    Cancelled,
}

impl DownloadError {
    pub fn remedy(&self) -> String {
        match self {
            DownloadError::UnknownModel(_) => "Pick a model from the list.".to_owned(),
            DownloadError::Network(_) => {
                "Check your internet connection and try again. Nothing else in ZScribe needs \
                 the network — this is a one-off download."
                    .to_owned()
            }
            DownloadError::Server(_) => {
                "The model host is having trouble. Try again in a few minutes.".to_owned()
            }
            DownloadError::Write { .. } => {
                "Check that ZScribe's data directory is writable and has free space. The path \
                 is in the Storage panel."
                    .to_owned()
            }
            DownloadError::ChecksumMismatch => {
                "This usually means the download was interrupted or altered in transit. Try \
                 again; if it keeps failing, check whether a proxy is rewriting your downloads."
                    .to_owned()
            }
            DownloadError::SizeMismatch { .. } => "The download ended early. Try again.".to_owned(),
            DownloadError::NotEnoughSpace { .. } => {
                "Free some disk space, or choose a smaller model.".to_owned()
            }
            DownloadError::Cancelled => "No action needed.".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    #[ts(type = "number")]
    pub downloaded_bytes: u64,

    #[ts(type = "number")]
    pub total_bytes: u64,

    pub percent: u8,

    pub verifying: bool,
}

impl Progress {
    fn downloading(downloaded_bytes: u64, total_bytes: u64) -> Self {
        Self {
            downloaded_bytes,
            total_bytes,

            percent: (downloaded_bytes.min(total_bytes) * 100)
                .checked_div(total_bytes)
                .unwrap_or(0) as u8,
            verifying: false,
        }
    }

    fn verifying(total_bytes: u64) -> Self {
        Self {
            downloaded_bytes: total_bytes,
            total_bytes,
            percent: 100,
            verifying: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledModel {
    pub id: String,
    pub installed: bool,

    pub partial: bool,
}

pub fn installed(models_dir: &Path) -> Vec<InstalledModel> {
    crate::models::MODELS
        .iter()
        .map(|spec| InstalledModel {
            id: spec.id.to_owned(),
            installed: spec.looks_installed(&models_dir.join(spec.file_name())),
            partial: part_path(models_dir, spec).exists(),
        })
        .collect()
}

pub fn remove(model_id: &str, models_dir: &Path) -> Result<bool, DownloadError> {
    let spec = find(model_id).ok_or_else(|| DownloadError::UnknownModel(model_id.to_owned()))?;

    let mut removed = false;
    for path in [
        models_dir.join(spec.file_name()),
        part_path(models_dir, spec),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => removed = true,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(DownloadError::Write {
                    path: path.display().to_string(),
                    message: err.to_string(),
                })
            }
        }
    }
    Ok(removed)
}

fn part_path(models_dir: &Path, spec: &ModelSpec) -> PathBuf {
    models_dir.join(format!("{}.part", spec.file_name()))
}

pub fn fetch(
    model_id: &str,
    models_dir: &Path,
    free_disk_mb: u64,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(Progress),
) -> Result<PathBuf, DownloadError> {
    let spec = find(model_id).ok_or_else(|| DownloadError::UnknownModel(model_id.to_owned()))?;
    let final_path = models_dir.join(spec.file_name());

    if spec.looks_installed(&final_path) {
        on_progress(Progress::downloading(spec.bytes, spec.bytes));
        return Ok(final_path);
    }

    if free_disk_mb > 0 && free_disk_mb < spec.megabytes() + 256 {
        return Err(DownloadError::NotEnoughSpace {
            needed_mb: spec.megabytes() + 256,
            free_mb: free_disk_mb,
        });
    }

    std::fs::create_dir_all(models_dir).map_err(|err| DownloadError::Write {
        path: models_dir.display().to_string(),
        message: err.to_string(),
    })?;

    let part = part_path(models_dir, spec);
    let downloaded = stream_to_file(spec, &part, &cancel, &mut on_progress)?;

    if downloaded != spec.bytes {
        let _ = std::fs::remove_file(&part);
        return Err(DownloadError::SizeMismatch {
            got: downloaded,
            expected: spec.bytes,
        });
    }

    on_progress(Progress::verifying(spec.bytes));

    let digest = sha256_file(&part).map_err(|err| DownloadError::Write {
        path: part.display().to_string(),
        message: err.to_string(),
    })?;

    if digest != spec.sha256 {
        tracing::error!(
            model = spec.id,
            expected = spec.sha256,
            got = %digest,
            "model checksum did not match; discarding the download"
        );
        let _ = std::fs::remove_file(&part);
        return Err(DownloadError::ChecksumMismatch);
    }

    std::fs::rename(&part, &final_path).map_err(|err| DownloadError::Write {
        path: final_path.display().to_string(),
        message: err.to_string(),
    })?;

    tracing::info!(model = spec.id, path = %final_path.display(), "model installed");
    Ok(final_path)
}

fn stream_to_file(
    spec: &ModelSpec,
    part: &Path,
    cancel: &Arc<AtomicBool>,
    on_progress: &mut impl FnMut(Progress),
) -> Result<u64, DownloadError> {
    let response = ureq::get(&spec.url()).call().map_err(|err| match err {
        ureq::Error::Status(code, _) => DownloadError::Server(code),
        other => DownloadError::Network(other.to_string()),
    })?;

    let total = response
        .header("content-length")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(spec.bytes);

    let file = std::fs::File::create(part).map_err(|err| DownloadError::Write {
        path: part.display().to_string(),
        message: err.to_string(),
    })?;
    let mut writer = std::io::BufWriter::new(file);
    let mut reader = response.into_reader();

    let mut buffer = vec![0u8; 256 * 1024];
    let mut downloaded = 0u64;
    let mut last_reported = 0u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(writer);
            let _ = std::fs::remove_file(part);
            return Err(DownloadError::Cancelled);
        }

        let read = reader
            .read(&mut buffer)
            .map_err(|err| DownloadError::Network(err.to_string()))?;
        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .map_err(|err| DownloadError::Write {
                path: part.display().to_string(),
                message: err.to_string(),
            })?;

        downloaded += read as u64;

        if downloaded - last_reported >= 1024 * 1024 {
            last_reported = downloaded;
            on_progress(Progress::downloading(downloaded, total));
        }
    }

    writer.flush().map_err(|err| DownloadError::Write {
        path: part.display().to_string(),
        message: err.to_string(),
    })?;

    Ok(downloaded)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn every_error_explains_itself_and_says_what_to_do() {
        for error in [
            DownloadError::UnknownModel("x".to_owned()),
            DownloadError::Network("dns".to_owned()),
            DownloadError::Server(503),
            DownloadError::Write {
                path: "/a".to_owned(),
                message: "full".to_owned(),
            },
            DownloadError::ChecksumMismatch,
            DownloadError::SizeMismatch {
                got: 1,
                expected: 2,
            },
            DownloadError::NotEnoughSpace {
                needed_mb: 1_800,
                free_mb: 200,
            },
            DownloadError::Cancelled,
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
            assert!(!error.remedy().is_empty(), "{error:?}");
        }
    }

    #[test]
    fn a_checksum_failure_says_the_file_was_discarded() {
        assert!(DownloadError::ChecksumMismatch
            .to_string()
            .contains("discarded"));
    }

    #[test]
    fn a_full_disk_is_reported_before_the_download_starts() {
        let dir = models_dir();
        let result = fetch(
            "large-v3-turbo",
            dir.path(),
            100,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        );

        assert!(matches!(result, Err(DownloadError::NotEnoughSpace { .. })));
        assert!(
            std::fs::read_dir(dir.path())
                .expect("read")
                .next()
                .is_none(),
            "nothing should have been written"
        );
    }

    #[test]
    fn a_disk_probe_that_failed_does_not_block_the_download() {
        let dir = models_dir();
        let cancelled = Arc::new(AtomicBool::new(true));

        let result = fetch("tiny", dir.path(), 0, cancelled, |_| {});
        assert!(
            !matches!(result, Err(DownloadError::NotEnoughSpace { .. })),
            "a failed probe must not be read as a full disk"
        );
    }

    #[test]
    fn an_unknown_model_is_refused_by_name() {
        let dir = models_dir();
        let result = fetch(
            "whisper-nine",
            dir.path(),
            100_000,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        );
        assert!(matches!(result, Err(DownloadError::UnknownModel(_))));
    }

    #[test]
    fn an_already_installed_model_is_not_downloaded_again() {
        let dir = models_dir();
        let spec = find("tiny").expect("present");

        let path = dir.path().join(spec.file_name());
        std::fs::write(&path, vec![0u8; spec.bytes as usize]).expect("write");

        let mut progress = None;
        let result = fetch(
            "tiny",
            dir.path(),
            100_000,
            Arc::new(AtomicBool::new(false)),
            |p| progress = Some(p),
        )
        .expect("already installed");

        assert_eq!(result, path);
        assert_eq!(progress.expect("reported").percent, 100);
    }

    #[test]
    fn cancelling_before_the_first_byte_leaves_nothing_behind() {
        let dir = models_dir();
        let result = fetch(
            "tiny",
            dir.path(),
            100_000,
            Arc::new(AtomicBool::new(true)),
            |_| {},
        );

        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert!(!dir.path().join("ggml-tiny.bin").exists());
        assert!(!dir.path().join("ggml-tiny.bin.part").exists());
    }

    #[test]
    fn a_fresh_directory_reports_every_model_as_missing() {
        let dir = models_dir();
        let listed = installed(dir.path());

        assert_eq!(listed.len(), crate::models::MODELS.len());
        assert!(listed.iter().all(|m| !m.installed && !m.partial));
    }

    #[test]
    fn an_interrupted_download_is_reported_as_partial_not_installed() {
        let dir = models_dir();
        std::fs::write(dir.path().join("ggml-tiny.bin.part"), b"half").expect("write");

        let tiny = installed(dir.path())
            .into_iter()
            .find(|m| m.id == "tiny")
            .expect("listed");

        assert!(!tiny.installed);
        assert!(tiny.partial);
    }

    #[test]
    fn removing_a_model_takes_its_partial_file_too() {
        let dir = models_dir();
        std::fs::write(dir.path().join("ggml-tiny.bin"), b"whole").expect("write");
        std::fs::write(dir.path().join("ggml-tiny.bin.part"), b"half").expect("write");

        assert!(remove("tiny", dir.path()).expect("remove"));
        assert!(!dir.path().join("ggml-tiny.bin").exists());
        assert!(!dir.path().join("ggml-tiny.bin.part").exists());
    }

    #[test]
    fn removing_a_model_that_is_not_there_is_not_an_error() {
        let dir = models_dir();
        assert!(!remove("tiny", dir.path()).expect("remove"));
    }

    #[test]
    fn progress_percentages_are_sane_at_the_edges() {
        assert_eq!(Progress::downloading(0, 100).percent, 0);
        assert_eq!(Progress::downloading(50, 100).percent, 50);
        assert_eq!(Progress::downloading(100, 100).percent, 100);

        assert_eq!(Progress::downloading(103, 100).percent, 100);

        assert_eq!(Progress::downloading(50, 0).percent, 0);
    }

    #[test]
    fn verifying_is_distinguishable_from_finished() {
        let verifying = Progress::verifying(1_000);
        assert!(verifying.verifying);
        assert_eq!(verifying.percent, 100);
        assert!(!Progress::downloading(1_000, 1_000).verifying);
    }

    #[test]
    fn the_checksum_of_a_known_file_is_correct() {
        let dir = models_dir();
        let path = dir.path().join("empty");
        std::fs::write(&path, b"").expect("write");

        assert_eq!(
            sha256_file(&path).expect("hash"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn the_checksum_reflects_the_content() {
        let dir = models_dir();
        let path = dir.path().join("abc");
        std::fs::write(&path, b"abc").expect("write");

        assert_eq!(
            sha256_file(&path).expect("hash"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
