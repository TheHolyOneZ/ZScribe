use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::state::AppState;

const POLL: Duration = Duration::from_secs(4);

const DONE: &str = "Imported";

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut seen: HashMap<PathBuf, u64> = HashMap::new();

        loop {
            std::thread::sleep(POLL);

            let Some(folder) = app.state::<AppState>().settings().folders.watch else {
                seen.clear();
                continue;
            };
            let folder = PathBuf::from(folder);

            let ready = match settled(&folder, &mut seen) {
                Ok(ready) => ready,
                Err(err) => {
                    tracing::debug!(folder = %folder.display(), %err, "cannot read the watched folder");
                    continue;
                }
            };

            for file in ready {
                if crate::recording::busy_with(&app).is_some() {
                    break;
                }

                tracing::info!(file = %file.display(), "importing from the watched folder");

                match crate::import::file(&app, file.clone()) {
                    Ok(_) => {
                        seen.remove(&file);
                        if let Err(err) = put_aside(&folder, &file) {
                            tracing::warn!(file = %file.display(), %err, "imported, but could not move the file aside");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(file = %file.display(), code = %err.code, "could not import from the watched folder");
                        seen.remove(&file);
                    }
                }
            }
        }
    });
}

fn settled(folder: &Path, seen: &mut HashMap<PathBuf, u64>) -> std::io::Result<Vec<PathBuf>> {
    let mut ready = Vec::new();
    let mut present: Vec<PathBuf> = Vec::new();

    for entry in std::fs::read_dir(folder)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if !path.is_file() || !importable(&path) {
            continue;
        }

        let Ok(size) = entry.metadata().map(|meta| meta.len()) else {
            continue;
        };
        present.push(path.clone());

        match seen.get(&path) {
            Some(before) if *before == size && size > 0 => ready.push(path),
            _ => {
                seen.insert(path, size);
            }
        }
    }

    seen.retain(|path, _| present.contains(path));

    ready.sort();
    Ok(ready)
}

fn importable(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let extension = extension.to_lowercase();
    zscribe_audio::IMPORTABLE_EXTENSIONS
        .iter()
        .any(|known| *known == extension)
}

fn put_aside(folder: &Path, file: &Path) -> std::io::Result<()> {
    let done = folder.join(DONE);
    std::fs::create_dir_all(&done)?;

    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "recording".to_owned());

    let mut destination = done.join(&name);
    let mut attempt = 2;
    while destination.exists() {
        let stem = Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "recording".to_owned());
        let extension = Path::new(&name)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        destination = done.join(format!("{stem} ({attempt}){extension}"));
        attempt += 1;
    }

    match std::fs::rename(file, &destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(file, &destination)?;
            std::fs::remove_file(file)
        }
    }
}

pub fn write_note(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();

    let Some(folder) = state.settings().folders.notes else {
        return;
    };
    let folder = PathBuf::from(folder);

    let markdown = match crate::commands::markdown_for(&state, id) {
        Ok(markdown) => markdown,
        Err(err) => {
            tracing::warn!(recording = %id, message = %err.message, "no note written");
            return;
        }
    };

    let existing = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .note_path(id)
        .ok()
        .flatten();

    let path = match existing {
        Some(path) => path,
        None => {
            let Some(detail) = state
                .recordings
                .lock()
                .expect("recordings lock poisoned")
                .get(id)
                .ok()
                .flatten()
            else {
                return;
            };
            folder.join(note_name(
                &detail.recording.title,
                detail.recording.started_at,
            ))
        }
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(folder = %parent.display(), %err, "could not create the notes folder");
            return;
        }
    }

    if let Err(err) = std::fs::write(&path, markdown) {
        tracing::warn!(note = %path.display(), %err, "could not write the note");
        return;
    }

    tracing::info!(recording = %id, note = %path.display(), "wrote the note");

    let _ = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .set_note_path(id, &path);
}

fn note_name(title: &str, started_at: i64) -> String {
    let when = time::OffsetDateTime::from_unix_timestamp(started_at)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(crate::local_offset());

    let date = format!(
        "{:04}-{:02}-{:02}",
        when.year(),
        when.month() as u8,
        when.day()
    );

    let title = safe_name(title);
    if title.is_empty() {
        format!("{date} recording.md")
    } else {
        format!("{date} {title}.md")
    }
}

fn safe_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();

    let trimmed = cleaned.trim().trim_end_matches('.').trim();

    let mut out: String = trimmed.chars().take(80).collect();
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_named_by_date_and_title() {
        let name = note_name("Weekly sync with the design team", 1_787_054_400);
        assert!(name.starts_with("2026-08-1"), "got: {name}");
        assert!(name.ends_with(" Weekly sync with the design team.md"));
    }

    #[test]
    fn a_title_that_would_break_a_file_system_is_repaired() {
        assert_eq!(
            safe_name("Q3/Q4 planning: what's next?"),
            "Q3-Q4 planning- what's next-"
        );
        assert_eq!(safe_name("Trailing dots..."), "Trailing dots");
        assert_eq!(safe_name("  padded  "), "padded");
    }

    #[test]
    fn a_recording_with_no_usable_title_still_gets_a_name() {
        let name = note_name("///", 1_787_054_400);
        assert!(name.ends_with(" ---.md"), "got: {name}");

        let name = note_name("   ", 1_787_054_400);
        assert!(name.ends_with(" recording.md"), "got: {name}");
    }

    #[test]
    fn only_media_files_are_picked_up() {
        assert!(importable(Path::new("/inbox/interview.MP3")));
        assert!(importable(Path::new("/inbox/call.wav")));
        assert!(!importable(Path::new("/inbox/notes.txt")));
        assert!(!importable(Path::new("/inbox/no-extension")));
    }

    #[test]
    fn a_file_is_only_ready_once_its_size_holds_still() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("call.wav");
        std::fs::write(&file, b"partial").expect("write");

        let mut seen = HashMap::new();

        assert!(settled(dir.path(), &mut seen).expect("list").is_empty());

        std::fs::write(&file, b"partial and then some").expect("write");
        assert!(settled(dir.path(), &mut seen).expect("list").is_empty());

        assert_eq!(settled(dir.path(), &mut seen).expect("list"), vec![file]);
    }

    #[test]
    fn an_empty_file_is_never_ready() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("call.wav"), b"").expect("write");

        let mut seen = HashMap::new();
        assert!(settled(dir.path(), &mut seen).expect("list").is_empty());
        assert!(settled(dir.path(), &mut seen).expect("list").is_empty());
    }

    #[test]
    fn a_file_that_disappears_is_forgotten() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("call.wav");
        std::fs::write(&file, b"something").expect("write");

        let mut seen = HashMap::new();
        settled(dir.path(), &mut seen).expect("list");
        assert_eq!(seen.len(), 1);

        std::fs::remove_file(&file).expect("remove");
        settled(dir.path(), &mut seen).expect("list");
        assert!(seen.is_empty());
    }

    #[test]
    fn moving_a_file_aside_never_overwrites_one_already_there() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("call.wav");

        std::fs::write(&file, b"first").expect("write");
        put_aside(dir.path(), &file).expect("move");

        std::fs::write(&file, b"second").expect("write");
        put_aside(dir.path(), &file).expect("move");

        let done = dir.path().join(DONE);
        assert_eq!(
            std::fs::read(done.join("call.wav")).expect("first"),
            b"first"
        );
        assert_eq!(
            std::fs::read(done.join("call (2).wav")).expect("second"),
            b"second"
        );
        assert!(!file.exists(), "the original is moved, not copied");
    }
}
