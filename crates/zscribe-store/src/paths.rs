use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("could not determine this platform's application directories")]
    NoHomeDirectory,
    #[error("could not create {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct Paths {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self, PathsError> {
        let dirs = ProjectDirs::from("dev", "TheHolyOneZ", "ZScribe")
            .ok_or(PathsError::NoHomeDirectory)?;

        let paths = Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
        };
        paths.ensure_dirs()?;
        Ok(paths)
    }

    pub fn rooted_at(root: impl AsRef<Path>) -> Result<Self, PathsError> {
        let root = root.as_ref();
        let paths = Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
        };
        paths.ensure_dirs()?;
        Ok(paths)
    }

    pub fn from_env() -> Result<Self, PathsError> {
        match std::env::var_os("ZSCRIBE_DATA_DIR") {
            Some(root) => Self::rooted_at(root),
            None => Self::resolve(),
        }
    }

    fn ensure_dirs(&self) -> Result<(), PathsError> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.logs_dir(),
            &self.recordings_dir(),
            &self.models_dir(),
            &self.tools_dir(),
        ] {
            std::fs::create_dir_all(dir).map_err(|source| PathsError::CreateDir {
                path: dir.clone(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.data_dir.join("recordings")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn tools_dir(&self) -> PathBuf {
        self.data_dir.join("bin")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn recordings_db(&self) -> PathBuf {
        self.data_dir.join("recordings.db")
    }

    pub fn fallback_key_file(&self) -> PathBuf {
        self.data_dir.join("secrets.key")
    }

    pub fn fallback_secrets_file(&self) -> PathBuf {
        self.data_dir.join("secrets.enc")
    }

    pub fn audio_file(&self, id: &str) -> PathBuf {
        self.recordings_dir().join(format!("{id}.wav"))
    }

    pub fn track_file(&self, id: &str, index: usize) -> PathBuf {
        self.recordings_dir().join(format!("{id}-{index}.wav"))
    }

    pub fn audio_files(&self, id: &str) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(self.recordings_dir()) else {
            return Vec::new();
        };

        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            })
            .filter(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| {
                        stem == id
                            || stem
                                .strip_prefix(id)
                                .and_then(|rest| rest.strip_prefix('-'))
                                .is_some_and(|index| {
                                    !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())
                                })
                    })
            })
            .collect()
    }

    pub fn model_file(&self, model_id: &str) -> PathBuf {
        self.models_dir().join(format!("ggml-{model_id}.bin"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_directory_is_created_and_distinct() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");

        assert!(paths.config_dir().is_dir());
        assert!(paths.data_dir().is_dir());
        assert!(paths.logs_dir().is_dir());
        assert!(paths.recordings_dir().is_dir());
        assert!(paths.models_dir().is_dir());

        assert!(paths.tools_dir().is_dir());
        assert_ne!(paths.config_dir(), paths.data_dir());
        assert_ne!(paths.recordings_dir(), paths.models_dir());
    }

    #[test]
    fn resolving_twice_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = Paths::rooted_at(temp.path()).expect("resolve");
        let second = Paths::rooted_at(temp.path()).expect("resolve again");
        assert_eq!(first.settings_file(), second.settings_file());
    }

    #[test]
    fn files_live_under_the_right_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");

        assert!(paths.settings_file().starts_with(paths.config_dir()));
        assert!(paths.recordings_db().starts_with(paths.data_dir()));
        assert!(paths.fallback_key_file().starts_with(paths.data_dir()));
        assert!(paths.audio_file("abc").starts_with(paths.recordings_dir()));
        assert!(paths.model_file("small").starts_with(paths.models_dir()));
    }

    #[test]
    fn every_track_of_a_recording_is_found_and_nobody_elses() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");

        let id = "5f2b";
        for path in [
            paths.audio_file(id),
            paths.track_file(id, 0),
            paths.track_file(id, 1),
            paths.track_file(id, 12),
            paths.audio_file("5f2bc9"),
            paths.track_file("5f2bc9", 0),
            paths.recordings_dir().join(format!("{id}-notes.txt")),
        ] {
            std::fs::write(&path, b"").expect("write");
        }

        let mut found: Vec<String> = paths
            .audio_files(id)
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();

        assert_eq!(
            found,
            ["5f2b-0.wav", "5f2b-1.wav", "5f2b-12.wav", "5f2b.wav"]
        );
    }

    #[test]
    fn asking_for_the_audio_of_a_recording_that_has_none_is_not_an_error() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");
        assert!(paths.audio_files("nothing-here").is_empty());
    }

    #[test]
    fn deleting_recordings_would_not_take_the_models_with_it() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");
        assert!(!paths.models_dir().starts_with(paths.recordings_dir()));
        assert!(!paths.recordings_dir().starts_with(paths.models_dir()));
    }

    #[test]
    fn each_microphone_writes_its_own_track_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");

        assert_ne!(paths.track_file("abc", 0), paths.track_file("abc", 1));
        assert!(paths
            .track_file("abc", 0)
            .starts_with(paths.recordings_dir()));
    }

    #[test]
    fn a_track_file_is_named_by_number_not_by_speaker() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");
        assert!(paths.track_file("abc", 1).ends_with("abc-1.wav"));
    }

    #[test]
    fn model_files_use_the_name_whisper_cpp_expects() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = Paths::rooted_at(temp.path()).expect("resolve");
        assert!(paths
            .model_file("large-v3-turbo")
            .ends_with("ggml-large-v3-turbo.bin"));
    }
}
