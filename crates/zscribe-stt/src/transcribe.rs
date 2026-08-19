use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use thiserror::Error;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use zscribe_core::{Segment, Transcript};

use crate::models::find;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("no transcription model is installed")]
    NoModel,

    #[error("the model file {path} is missing")]
    ModelMissing { path: String },

    #[error("the model file {path} could not be loaded: {message}")]
    ModelUnreadable { path: String, message: String },

    #[error("transcription failed: {0}")]
    Failed(String),

    #[error("the recording contains no audio")]
    EmptyAudio,

    #[error("cancelled")]
    Cancelled,
}

impl SttError {
    pub fn code(&self) -> &'static str {
        match self {
            SttError::NoModel => "no_model",
            SttError::ModelMissing { .. } => "model_missing",
            SttError::ModelUnreadable { .. } => "model_unreadable",
            SttError::Failed(_) => "stt_failed",
            SttError::EmptyAudio => "empty_audio",
            SttError::Cancelled => "cancelled",
        }
    }

    pub fn remedy(&self) -> String {
        match self {
            SttError::NoModel => {
                "Open Speech to text and install a model. ZScribe scans your machine first and \
                 recommends one that will actually run well on it."
                    .to_owned()
            }
            SttError::ModelMissing { .. } => {
                "The model file has been deleted or moved. Download it again from \
                 Speech to text."
                    .to_owned()
            }
            SttError::ModelUnreadable { .. } => {
                "The download is probably damaged. Remove the model in Speech to text and \
                 download it again."
                    .to_owned()
            }
            SttError::Failed(_) => {
                "If this repeats, try turning off GPU acceleration in Speech to text — a driver \
                 problem shows up here first. Your recording is saved either way."
                    .to_owned()
            }
            SttError::EmptyAudio => {
                "Nothing was captured. Check the microphone in Audio sources, and watch the \
                 waveform on the recording bar next time — it moves when sound is arriving."
                    .to_owned()
            }
            SttError::Cancelled => "No action needed.".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub model_path: PathBuf,

    pub model_id: String,

    pub language: Option<String>,

    pub threads: usize,

    pub use_gpu: bool,
}

pub const fn gpu_support_compiled_in() -> bool {
    cfg!(feature = "vulkan") || cfg!(feature = "metal")
}

const MIN_SAMPLES: usize = 16_000 / 4;

pub fn transcribe(
    audio: &[f32],
    options: &Options,
    cancel: Arc<AtomicBool>,
    on_progress: impl FnMut(u8) + 'static,
) -> Result<Transcript, SttError> {
    if audio.len() < MIN_SAMPLES {
        return Err(SttError::EmptyAudio);
    }

    if !options.model_path.is_file() {
        return Err(SttError::ModelMissing {
            path: options.model_path.display().to_string(),
        });
    }

    let context = load(options)?;
    let mut state = context
        .create_state()
        .map_err(|err| SttError::Failed(err.to_string()))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    configure(&mut params, options, Arc::clone(&cancel), on_progress);

    state.full(params, audio).map_err(|err| {
        if cancel.load(Ordering::Relaxed) {
            SttError::Cancelled
        } else {
            SttError::Failed(err.to_string())
        }
    })?;

    if cancel.load(Ordering::Relaxed) {
        return Err(SttError::Cancelled);
    }

    let segments = collect_segments(&state);
    let language = detect_language(&state, options);

    tracing::info!(
        model = options.model_id,
        language,
        segments = segments.len(),
        "transcription finished"
    );

    Ok(Transcript {
        language,
        model: options.model_id.clone(),
        segments,
    })
}

fn load(options: &Options) -> Result<WhisperContext, SttError> {
    let mut params = WhisperContextParameters::default();
    params.use_gpu(options.use_gpu && gpu_support_compiled_in());

    let path = options.model_path.display().to_string();

    WhisperContext::new_with_params(&path, params).map_err(|err| SttError::ModelUnreadable {
        path,
        message: err.to_string(),
    })
}

fn configure<'a>(
    params: &mut FullParams<'a, '_>,
    options: &'a Options,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(u8) + 'static,
) {
    params.set_n_threads(options.threads.clamp(1, 16) as i32);

    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    params.set_translate(false);

    params.set_language(Some(whisper_language(options)));

    params.set_suppress_blank(true);
    params.set_suppress_nst(true);

    params.set_progress_callback_safe(move |percent: i32| {
        on_progress(percent.clamp(0, 100) as u8);
    });

    let abort: Box<dyn FnMut() -> bool> = Box::new(move || cancel.load(Ordering::Relaxed));
    params.set_abort_callback_safe(abort);
}

fn collect_segments(state: &whisper_rs::WhisperState) -> Vec<Segment> {
    state
        .as_iter()
        .filter_map(|segment| {
            let text = segment.to_str_lossy().ok()?.trim().to_owned();
            if text.is_empty() {
                return None;
            }

            Some(Segment::new(
                centiseconds_to_ms(segment.start_timestamp()),
                centiseconds_to_ms(segment.end_timestamp()),
                text,
            ))
        })
        .collect()
}

fn whisper_language(options: &Options) -> &str {
    match options.language.as_deref() {
        Some(tag) if !tag.is_empty() && tag != "auto" => tag,
        _ => "auto",
    }
}

fn centiseconds_to_ms(value: i64) -> u32 {
    value
        .max(0)
        .saturating_mul(10)
        .try_into()
        .unwrap_or(u32::MAX)
}

fn detect_language(state: &whisper_rs::WhisperState, options: &Options) -> String {
    if let Some(tag) = options.language.as_deref() {
        if !tag.is_empty() && tag != "auto" {
            return tag.to_owned();
        }
    }

    whisper_rs::get_lang_str(state.full_lang_id_from_state())
        .unwrap_or("unknown")
        .to_owned()
}

pub fn transcribe_file(
    audio_path: &Path,
    options: &Options,
    cancel: Arc<AtomicBool>,
    on_progress: impl FnMut(u8) + 'static,
) -> Result<Transcript, SttError> {
    let audio =
        zscribe_audio::read_mono(audio_path).map_err(|err| SttError::Failed(err.to_string()))?;
    transcribe(&audio, options, cancel, on_progress)
}

pub fn options_for(
    model_id: &str,
    models_dir: &Path,
    language: &str,
    threads: usize,
    use_gpu: bool,
) -> Result<Options, SttError> {
    if model_id.trim().is_empty() {
        return Err(SttError::NoModel);
    }

    let spec = find(model_id).ok_or(SttError::NoModel)?;
    let model_path = models_dir.join(spec.file_name());

    if !model_path.is_file() {
        return Err(SttError::ModelMissing {
            path: model_path.display().to_string(),
        });
    }

    Ok(Options {
        model_path,
        model_id: model_id.to_owned(),
        language: (language != "auto").then(|| language.to_owned()),
        threads,
        use_gpu,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        Options {
            model_path: PathBuf::from("/nonexistent/ggml-tiny.bin"),
            model_id: "tiny".to_owned(),
            language: None,
            threads: 4,
            use_gpu: false,
        }
    }

    #[test]
    fn every_error_explains_itself_and_says_what_to_do() {
        for error in [
            SttError::NoModel,
            SttError::ModelMissing {
                path: "/a".to_owned(),
            },
            SttError::ModelUnreadable {
                path: "/a".to_owned(),
                message: "bad magic".to_owned(),
            },
            SttError::Failed("gpu fell over".to_owned()),
            SttError::EmptyAudio,
            SttError::Cancelled,
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
            assert!(!error.remedy().is_empty(), "{error:?}");
            assert!(!error.code().is_empty(), "{error:?}");
        }
    }

    #[test]
    fn a_transcription_failure_reassures_the_user_the_recording_survived() {
        assert!(SttError::Failed("boom".to_owned())
            .remedy()
            .contains("saved"));
    }

    #[test]
    fn error_codes_are_unique() {
        let codes = [
            SttError::NoModel.code(),
            SttError::ModelMissing {
                path: String::new(),
            }
            .code(),
            SttError::ModelUnreadable {
                path: String::new(),
                message: String::new(),
            }
            .code(),
            SttError::Failed(String::new()).code(),
            SttError::EmptyAudio.code(),
            SttError::Cancelled.code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        let total = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), total);
    }

    #[test]
    fn a_recording_of_almost_nothing_is_refused_rather_than_hallucinated() {
        let result = transcribe(
            &[0.0; 100],
            &options(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        );
        assert!(matches!(result, Err(SttError::EmptyAudio)));
    }

    #[test]
    fn a_missing_model_file_is_reported_before_any_work_starts() {
        let result = transcribe(
            &vec![0.1; 16_000],
            &options(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        );
        assert!(matches!(result, Err(SttError::ModelMissing { .. })));
    }

    #[test]
    fn no_model_configured_is_distinct_from_a_model_file_that_vanished() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert!(matches!(
            options_for("", dir.path(), "auto", 4, true),
            Err(SttError::NoModel)
        ));
        assert!(matches!(
            options_for("tiny", dir.path(), "auto", 4, true),
            Err(SttError::ModelMissing { .. })
        ));
    }

    #[test]
    fn an_unknown_model_id_is_not_silently_accepted() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(matches!(
            options_for("whisper-nine", dir.path(), "auto", 4, true),
            Err(SttError::NoModel)
        ));
    }

    #[test]
    fn options_resolve_to_the_file_name_whisper_cpp_expects() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("ggml-tiny.bin"), b"pretend").expect("write");

        let options = options_for("tiny", dir.path(), "auto", 6, true).expect("resolves");

        assert!(options.model_path.ends_with("ggml-tiny.bin"));
        assert_eq!(options.threads, 6);
        assert!(options.use_gpu);
        assert_eq!(options.language, None, "auto means let Whisper decide");
    }

    #[test]
    fn a_pinned_language_is_carried_through_rather_than_detected() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("ggml-tiny.bin"), b"pretend").expect("write");

        let options = options_for("tiny", dir.path(), "de", 4, false).expect("resolves");
        assert_eq!(options.language.as_deref(), Some("de"));
    }

    #[test]
    fn an_unpinned_language_is_passed_to_whisper_as_auto() {
        assert_eq!(whisper_language(&options()), "auto");

        assert_eq!(
            whisper_language(&Options {
                language: Some("auto".to_owned()),
                ..options()
            }),
            "auto"
        );
        assert_eq!(
            whisper_language(&Options {
                language: Some(String::new()),
                ..options()
            }),
            "auto"
        );
    }

    #[test]
    fn a_pinned_language_is_passed_through_verbatim() {
        assert_eq!(
            whisper_language(&Options {
                language: Some("de".to_owned()),
                ..options()
            }),
            "de"
        );
    }

    #[test]
    fn whisper_centiseconds_become_milliseconds() {
        assert_eq!(centiseconds_to_ms(0), 0);
        assert_eq!(centiseconds_to_ms(100), 1_000);
        assert_eq!(centiseconds_to_ms(7_250), 72_500);
    }

    #[test]
    fn a_negative_timestamp_does_not_wrap_around() {
        assert_eq!(centiseconds_to_ms(-5), 0);
    }

    #[test]
    fn this_build_reports_its_gpu_support_honestly() {
        assert_eq!(
            gpu_support_compiled_in(),
            cfg!(feature = "vulkan") || cfg!(feature = "metal")
        );
    }
}
