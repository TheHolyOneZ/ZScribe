#![forbid(unsafe_code)]

pub mod decode;
pub mod device;
pub mod level;
pub mod loopback;
pub mod resample;
pub mod tone;
pub mod voiceprint;
pub mod wav;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use decode::{decode_to_wav, inspect, MediaInfo, IMPORTABLE_EXTENSIONS};
pub use device::{input_devices, InputDevice};
pub use level::{levels_for, Level};
pub use loopback::{system_audio_sources, SystemAudioSource};
pub use tone::play_start_tone;
pub use voiceprint::{Frames, Voiceprint};
pub use wav::{peaks, read_mono, SAMPLE_RATE};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no microphone was found")]
    NoInputDevice,

    #[error("the microphone could not be opened: {0}")]
    Device(String),

    #[error("recording stopped: {0}")]
    Stream(String),

    #[error("this microphone offers a sample format ZScribe cannot read ({0})")]
    UnsupportedFormat(String),

    #[error("could not write {path}: {message}")]
    Write { path: String, message: String },

    #[error("could not read {path}: {message}")]
    Read { path: String, message: String },

    #[error("{0} has no audio in it")]
    NoAudioTrack(String),

    #[error("{path} could not be decoded: {message}")]
    Undecodable { path: String, message: String },
}

impl AudioError {
    pub fn remedy(&self) -> &'static str {
        match self {
            AudioError::NoInputDevice => {
                "Connect a microphone, then reopen the Audio sources panel to pick it."
            }
            AudioError::Device(_) => {
                "Another application may have exclusive use of the microphone. Close it, or \
                 choose a different device in Audio sources."
            }
            AudioError::Stream(_) => {
                "The microphone stopped responding — this usually means it was unplugged. \
                 Whatever was captured up to that point has been kept."
            }
            AudioError::UnsupportedFormat(_) => {
                "Choose a different microphone in Audio sources, or set this one to 16-bit or \
                 32-bit float in your system's sound settings."
            }
            AudioError::Write { .. } => {
                "Check that ZScribe's data directory exists and has free space. The path is \
                 shown in the Storage panel."
            }
            AudioError::Read { .. } => {
                "The audio file is missing or damaged. If the transcript was already saved it \
                 is unaffected."
            }
            AudioError::NoAudioTrack(_) => {
                "Only the sound is used, so a video is fine — but this file has no audio track \
                 at all. A silent screen recording is the usual cause."
            }
            AudioError::Undecodable { .. } => {
                "ZScribe reads WAV, MP3, M4A, MP4, FLAC, OGG, MKV and AIFF. A file that plays \
                 elsewhere but not here is usually in a format it does not know; converting it \
                 to WAV or MP3 will import."
            }
        }
    }
}

pub const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub struct RecordOptions {
    pub device_id: Option<String>,

    pub system_source: Option<String>,

    pub exact_device: bool,

    pub output: PathBuf,

    pub preroll: Vec<f32>,
}

#[derive(Debug)]
struct Shared {
    rms_bits: AtomicU32,
    peak_bits: AtomicU32,

    frames: AtomicU64,
    paused: AtomicBool,
    stopping: AtomicBool,

    hiccups: AtomicU32,

    failure: Mutex<Option<String>>,

    recent: Mutex<VecDeque<f32>>,

    recent_cap: usize,
}

impl Shared {
    fn holding(keep: Duration) -> Self {
        let samples = (keep.as_secs_f64() * f64::from(SAMPLE_RATE)) as usize;
        Self {
            recent_cap: samples.max(1),
            ..Self::default()
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            rms_bits: AtomicU32::default(),
            peak_bits: AtomicU32::default(),
            frames: AtomicU64::default(),
            paused: AtomicBool::default(),
            stopping: AtomicBool::default(),
            hiccups: AtomicU32::default(),
            failure: Mutex::default(),
            recent: Mutex::default(),
            recent_cap: LIVE_WINDOW_SAMPLES,
        }
    }
}

pub const LIVE_WINDOW_SECONDS: usize = 28;

const LIVE_WINDOW_SAMPLES: usize = LIVE_WINDOW_SECONDS * SAMPLE_RATE as usize;

impl Shared {
    fn push_recent(&self, samples: &[f32]) {
        let mut recent = self.recent.lock().expect("recent-audio lock poisoned");
        recent.extend(samples.iter().copied());

        let excess = recent.len().saturating_sub(self.recent_cap);
        if excess > 0 {
            recent.drain(..excess);
        }
    }

    fn store_level(&self, level: Level) {
        self.rms_bits.store(level.rms.to_bits(), Ordering::Relaxed);
        self.peak_bits
            .store(level.peak.to_bits(), Ordering::Relaxed);
    }

    fn level(&self) -> Level {
        Level {
            rms: f32::from_bits(self.rms_bits.load(Ordering::Relaxed)),
            peak: f32::from_bits(self.peak_bits.load(Ordering::Relaxed)),
        }
    }
}

pub struct Session {
    shared: Arc<Shared>,
    capture: Option<std::thread::JoinHandle<()>>,
    writer: Option<std::thread::JoinHandle<Result<u32, AudioError>>>,
    output: PathBuf,

    listening: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finished {
    pub path: String,
    pub duration_ms: u32,
}

impl Session {
    pub fn start(options: RecordOptions) -> Result<Self, AudioError> {
        Self::open(options, None)
    }

    pub fn listen(options: RecordOptions, keep: Duration) -> Result<Self, AudioError> {
        Self::open(options, Some(keep))
    }

    fn open(mut options: RecordOptions, keep: Option<Duration>) -> Result<Self, AudioError> {
        let sink = match keep {
            None => Some(options.output.clone()),
            Some(_) => None,
        };

        let _source_guard = options.system_source.as_deref().map(loopback::with_source);

        let candidates = match options.system_source.as_deref() {
            Some(source) => loopback::candidates(source)?,
            None => device::candidates(options.device_id.as_deref(), options.exact_device)?,
        };

        let (rate_tx, rate_rx) = std::sync::mpsc::channel::<u32>();
        let shared = Arc::new(match keep {
            Some(keep) => Shared::holding(keep),
            None => Shared::default(),
        });

        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), AudioError>>();

        let capture = std::thread::Builder::new()
            .name("zscribe-capture".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                move || {
                    let (stream, rate) = match open_first_working(candidates, tx, &shared) {
                        Ok(opened) => opened,
                        Err(err) => {
                            let _ = ready_tx.send(Err(err));
                            return;
                        }
                    };

                    let _ = rate_tx.send(rate);
                    let _ = ready_tx.send(Ok(()));

                    while !shared.stopping.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    drop(stream);
                }
            })
            .map_err(|err| AudioError::Device(err.to_string()))?;

        let input_rate = rate_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or(SAMPLE_RATE);

        let writer = std::thread::Builder::new()
            .name("zscribe-writer".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                let output = sink.clone();
                let preroll = std::mem::take(&mut options.preroll);
                move || write_loop(output.as_deref(), preroll, rx, input_rate, &shared)
            })
            .map_err(|err| AudioError::Device(err.to_string()))?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                shared.stopping.store(true, Ordering::Relaxed);
                let _ = capture.join();
                let _ = writer.join();
                return Err(err);
            }
            Err(_) => {
                shared.stopping.store(true, Ordering::Relaxed);
                return Err(AudioError::Device(
                    "the microphone did not start within five seconds".to_owned(),
                ));
            }
        }

        Ok(Self {
            shared,
            capture: Some(capture),
            writer: Some(writer),
            output: options.output,
            listening: keep.is_some(),
        })
    }

    pub fn level(&self) -> Level {
        self.shared.level()
    }

    pub fn duration_ms(&self) -> u32 {
        ((self.shared.frames.load(Ordering::Relaxed) * 1000) / u64::from(SAMPLE_RATE)) as u32
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    pub fn recent_audio(&self) -> Vec<f32> {
        self.shared
            .recent
            .lock()
            .expect("recent-audio lock poisoned")
            .iter()
            .copied()
            .collect()
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
        self.shared.store_level(Level::default());
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
    }

    pub fn hiccups(&self) -> u32 {
        self.shared.hiccups.load(Ordering::Relaxed)
    }

    pub fn failure(&self) -> Option<String> {
        self.shared
            .failure
            .lock()
            .expect("failure lock poisoned")
            .clone()
    }

    pub fn buffered(&self) -> Vec<f32> {
        self.recent_audio()
    }

    pub fn buffered_ms(&self) -> u32 {
        let samples = self
            .shared
            .recent
            .lock()
            .expect("recent-audio lock poisoned")
            .len();
        ((samples as u64 * 1000) / u64::from(SAMPLE_RATE)) as u32
    }

    pub fn stop(mut self) -> Result<Finished, AudioError> {
        self.shared.stopping.store(true, Ordering::Relaxed);

        if let Some(handle) = self.capture.take() {
            let _ = handle.join();
        }

        let duration_ms = match self.writer.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| AudioError::Stream("the writer thread panicked".to_owned()))??,
            None => 0,
        };

        if let Some(message) = self.failure() {
            tracing::warn!(%message, duration_ms, "the recording ended early");
        }

        Ok(Finished {
            path: if self.listening {
                String::new()
            } else {
                self.output.display().to_string()
            },
            duration_ms,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Relaxed);
        if let Some(handle) = self.capture.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.writer.take() {
            let _ = handle.join();
        }
    }
}

fn is_fatal(err: &cpal::Error) -> bool {
    use cpal::ErrorKind;

    match err.kind() {
        ErrorKind::DeviceNotAvailable
        | ErrorKind::HostUnavailable
        | ErrorKind::PermissionDenied
        | ErrorKind::StreamInvalidated => true,

        ErrorKind::DeviceChanged
        | ErrorKind::DeviceBusy
        | ErrorKind::RealtimeDenied
        | ErrorKind::ResourceExhausted
        | ErrorKind::Xrun => false,

        _ => false,
    }
}

fn open_first_working(
    candidates: Vec<device::Candidate>,
    tx: SyncSender<Vec<f32>>,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, u32), AudioError> {
    let mut last_error = AudioError::NoInputDevice;

    for candidate in candidates {
        let rate = candidate.config.sample_rate();
        let channels = candidate.config.channels() as usize;
        let format = candidate.config.sample_format();

        let stream = match build_stream(
            &candidate.device,
            &candidate.config,
            format,
            channels,
            tx.clone(),
            shared,
        ) {
            Ok(stream) => stream,
            Err(err) => {
                tracing::debug!(device = %candidate.label, %err, "microphone would not open");
                last_error = err;
                continue;
            }
        };

        if let Err(err) = stream.play() {
            tracing::debug!(device = %candidate.label, %err, "microphone would not start");
            last_error = AudioError::Device(err.to_string());
            continue;
        }

        tracing::info!(
            device = %candidate.label,
            rate,
            channels,
            format = ?format,
            "recording from microphone"
        );
        return Ok((stream, rate));
    }

    Err(last_error)
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    format: cpal::SampleFormat,
    channels: usize,
    tx: SyncSender<Vec<f32>>,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, AudioError> {
    let stream_config: cpal::StreamConfig = (*config).into();

    let on_error = {
        let shared = Arc::clone(shared);
        move |err: cpal::Error| {
            if is_fatal(&err) {
                tracing::error!(%err, "the audio stream stopped");
                *shared.failure.lock().expect("failure lock poisoned") = Some(err.to_string());
            } else {
                let seen = shared.hiccups.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(%err, seen, "audio glitch; recording continues");
            }
        }
    };

    macro_rules! input_stream {
        ($sample:ty, $to_f32:expr) => {{
            let shared = Arc::clone(shared);
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                    if shared.paused.load(Ordering::Relaxed) {
                        return;
                    }
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|frame| {
                            frame.iter().map(|s| $to_f32(*s)).sum::<f32>() / frame.len() as f32
                        })
                        .collect();

                    if let Err(TrySendError::Full(_)) = tx.try_send(mono) {
                        tracing::warn!("audio buffer dropped: the writer could not keep up");
                    }
                },
                on_error,
                None,
            )
        }};
    }

    let stream = match format {
        cpal::SampleFormat::F32 => input_stream!(f32, |s: f32| s),
        cpal::SampleFormat::F64 => input_stream!(f64, |s: f64| s as f32),
        cpal::SampleFormat::I16 => {
            input_stream!(i16, |s: i16| f32::from(s) / f32::from(i16::MAX))
        }
        cpal::SampleFormat::I32 => input_stream!(i32, |s: i32| s as f32 / i32::MAX as f32),
        cpal::SampleFormat::I8 => input_stream!(i8, |s: i8| f32::from(s) / f32::from(i8::MAX)),
        cpal::SampleFormat::U16 => {
            input_stream!(u16, |s: u16| { (f32::from(s) - 32_768.0) / 32_768.0 })
        }
        cpal::SampleFormat::U8 => {
            input_stream!(u8, |s: u8| (f32::from(s) - 128.0) / 128.0)
        }
        other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
    };

    stream.map_err(|err| AudioError::Device(err.to_string()))
}

fn write_loop(
    output: Option<&Path>,
    preroll: Vec<f32>,
    rx: Receiver<Vec<f32>>,
    input_rate: u32,
    shared: &Arc<Shared>,
) -> Result<u32, AudioError> {
    let mut writer = match output {
        Some(path) => Some(wav::WavWriter::create(path)?),
        None => None,
    };
    let mut resampler = resample::Resampler::new(input_rate, SAMPLE_RATE);
    let mut meter = Level::default();

    if !preroll.is_empty() {
        match writer.as_mut() {
            Some(writer) => {
                writer.write(&preroll)?;
                shared.frames.store(writer.frames(), Ordering::Relaxed);
            }
            None => shared.push_recent(&preroll),
        }
    }

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(buffer) => {
                let resampled = resampler.process(&buffer);
                if resampled.is_empty() {
                    continue;
                }

                meter = meter.smoothed(Level::of(&resampled));
                shared.store_level(meter);
                shared.push_recent(&resampled);

                match writer.as_mut() {
                    Some(writer) => {
                        writer.write(&resampled)?;
                        shared.frames.store(writer.frames(), Ordering::Relaxed);
                    }

                    None => {
                        shared
                            .frames
                            .fetch_add(resampled.len() as u64, Ordering::Relaxed);
                    }
                }
            }

            Err(RecvTimeoutError::Timeout) => {
                if shared.stopping.load(Ordering::Relaxed) {
                    break;
                }
            }

            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let tail = resampler.finish();
    if !tail.is_empty() {
        shared.push_recent(&tail);
        if let Some(writer) = writer.as_mut() {
            writer.write(&tail)?;
        }
    }

    shared.store_level(Level::default());

    let Some(writer) = writer else {
        return Ok(0);
    };

    let duration_ms = writer.finish()?;
    shared.frames.store(
        u64::from(duration_ms) * u64::from(SAMPLE_RATE) / 1000,
        Ordering::Relaxed,
    );

    Ok(duration_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preroll_lands_at_the_head_of_the_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("preroll.wav");

        let preroll: Vec<f32> = (0..SAMPLE_RATE)
            .map(|n| (n as f32 / 32_000.0) - 0.5)
            .collect();

        let shared = Arc::new(Shared::default());
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);

        tx.send(vec![0.75; 400]).expect("send");
        drop(tx);

        let duration_ms =
            write_loop(Some(&path), preroll.clone(), rx, SAMPLE_RATE, &shared).expect("write");

        let written = wav::read_mono(&path).expect("read back");
        assert_eq!(written.len(), preroll.len() + 400);
        assert_eq!(duration_ms, 1025, "one second of buffer plus 400 samples");

        for (index, (found, wanted)) in written.iter().zip(&preroll).enumerate() {
            assert!(
                (found - wanted).abs() < 0.001,
                "sample {index}: {found} is not {wanted}"
            );
        }
        assert!(
            (written[preroll.len()] - 0.75).abs() < 0.001,
            "then the live audio"
        );
    }

    #[test]
    fn a_listening_session_writes_no_file_at_all() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("never.wav");

        let shared = Arc::new(Shared::holding(Duration::from_secs(2)));
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);
        tx.send(vec![0.5; 800]).expect("send");
        drop(tx);

        let duration_ms = write_loop(None, Vec::new(), rx, SAMPLE_RATE, &shared).expect("listen");

        assert_eq!(duration_ms, 0, "listening produces no recording");
        assert!(!path.exists(), "and no file, not even an empty one");
        assert_eq!(
            shared.recent.lock().expect("lock").len(),
            800,
            "but it is holding what it heard"
        );
    }

    #[test]
    fn a_listening_session_can_be_reopened_holding_what_it_had() {
        let shared = Arc::new(Shared::holding(Duration::from_secs(2)));
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);
        drop(tx);

        let held = vec![0.25; 1_600];
        write_loop(None, held.clone(), rx, SAMPLE_RATE, &shared).expect("listen");

        let recent: Vec<f32> = shared
            .recent
            .lock()
            .expect("lock")
            .iter()
            .copied()
            .collect();
        assert_eq!(recent, held, "reopened still holding every sample");
    }

    #[test]
    fn the_rolling_buffer_never_grows_past_what_was_asked_for() {
        let shared = Shared::holding(Duration::from_millis(500));
        let cap = SAMPLE_RATE as usize / 2;

        for _ in 0..10 {
            shared.push_recent(&vec![0.1; cap]);
        }

        assert_eq!(shared.recent.lock().expect("lock").len(), cap);
    }

    #[test]
    fn every_error_explains_itself_and_says_what_to_do() {
        for error in [
            AudioError::NoInputDevice,
            AudioError::Device("busy".to_owned()),
            AudioError::Stream("disconnected".to_owned()),
            AudioError::UnsupportedFormat("F64".to_owned()),
            AudioError::Write {
                path: "/tmp/a.wav".to_owned(),
                message: "no space".to_owned(),
            },
            AudioError::Read {
                path: "/tmp/a.wav".to_owned(),
                message: "missing".to_owned(),
            },
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
            assert!(!error.remedy().is_empty(), "{error:?}");
        }
    }

    #[test]
    fn an_unplugged_microphone_promises_that_the_audio_so_far_was_kept() {
        let remedy = AudioError::Stream("device removed".to_owned()).remedy();
        assert!(remedy.contains("kept"), "got: {remedy}");
    }

    #[test]
    fn a_recoverable_glitch_does_not_end_the_recording() {
        for kind in [
            cpal::ErrorKind::DeviceChanged,
            cpal::ErrorKind::DeviceBusy,
            cpal::ErrorKind::RealtimeDenied,
            cpal::ErrorKind::ResourceExhausted,
        ] {
            let err = cpal::Error::new(kind);
            assert!(!is_fatal(&err), "{kind:?} must not end a recording");
        }
    }

    #[test]
    fn a_device_that_has_genuinely_gone_ends_the_recording() {
        for kind in [
            cpal::ErrorKind::DeviceNotAvailable,
            cpal::ErrorKind::HostUnavailable,
            cpal::ErrorKind::PermissionDenied,
            cpal::ErrorKind::StreamInvalidated,
        ] {
            let err = cpal::Error::new(kind);
            assert!(is_fatal(&err), "{kind:?} should end a recording");
        }
    }

    #[test]
    fn a_buffer_underrun_never_ends_a_recording() {
        assert!(!is_fatal(&cpal::Error::new(cpal::ErrorKind::Xrun)));
    }

    #[test]
    fn an_unclassified_backend_error_is_treated_as_recoverable() {
        let err = cpal::Error::new(cpal::ErrorKind::BackendError);
        assert!(!is_fatal(&err));
    }

    #[test]
    fn glitches_are_counted_so_a_rough_recording_can_be_reported() {
        let shared = Shared::default();
        assert_eq!(shared.hiccups.load(Ordering::Relaxed), 0);

        shared.hiccups.fetch_add(1, Ordering::Relaxed);
        shared.hiccups.fetch_add(1, Ordering::Relaxed);
        assert_eq!(shared.hiccups.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn the_level_survives_the_trip_through_shared_state() {
        let shared = Shared::default();
        let level = Level {
            rms: 0.25,
            peak: 0.8,
        };

        shared.store_level(level);
        assert_eq!(shared.level(), level);
    }

    #[test]
    fn the_meter_starts_at_silence() {
        assert_eq!(Shared::default().level(), Level::default());
    }

    #[test]
    fn the_writer_produces_a_playable_file_from_synthetic_audio() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("take.wav");

        let shared = Arc::new(Shared::default());
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);

        for chunk in 0..50 {
            let buffer: Vec<f32> = (0..480)
                .map(|i| {
                    let t = (chunk * 480 + i) as f32 / 48_000.0;
                    (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
                })
                .collect();
            tx.send(buffer).expect("send");
        }
        drop(tx);

        let duration_ms =
            write_loop(Some(&path), Vec::new(), rx, 48_000, &shared).expect("write loop");

        assert!(
            (480..=520).contains(&duration_ms),
            "half a second became {duration_ms} ms"
        );

        let samples = read_mono(&path).expect("read back");
        assert!(
            (7_800..=8_200).contains(&samples.len()),
            "got {} samples",
            samples.len()
        );

        let peak = samples.iter().fold(0.0f32, |max, s| max.max(s.abs()));
        assert!((0.45..=0.55).contains(&peak), "amplitude drifted to {peak}");
    }

    #[test]
    fn the_writer_leaves_a_valid_empty_file_when_no_audio_arrives() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("silence.wav");

        let shared = Arc::new(Shared::default());
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);
        drop(tx);

        assert_eq!(
            write_loop(Some(&path), Vec::new(), rx, 48_000, &shared).expect("write loop"),
            0
        );
        assert!(read_mono(&path).expect("read").is_empty());
    }

    #[test]
    fn recent_audio_is_kept_for_transcribing_while_still_recording() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("take.wav");

        let shared = Arc::new(Shared::default());
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);

        for _ in 0..20 {
            tx.send(vec![0.4; 1_600]).expect("send");
        }
        drop(tx);

        write_loop(Some(&path), Vec::new(), rx, SAMPLE_RATE, &shared).expect("write loop");

        let recent = shared.recent.lock().expect("lock").len();
        assert_eq!(recent, 32_000, "two seconds should all still be there");
    }

    #[test]
    fn the_recent_buffer_is_bounded_so_a_long_recording_cannot_grow_it() {
        let shared = Shared::default();

        for _ in 0..600 {
            shared.push_recent(&vec![0.1; SAMPLE_RATE as usize]);
        }

        let held = shared.recent.lock().expect("lock").len();
        assert_eq!(held, LIVE_WINDOW_SAMPLES, "must hold exactly the window");
    }

    #[test]
    fn the_recent_buffer_keeps_the_newest_audio_not_the_oldest() {
        let shared = Shared::default();

        shared.push_recent(&vec![0.1; LIVE_WINDOW_SAMPLES]);
        shared.push_recent(&vec![0.9; 1_000]);

        let recent = shared.recent.lock().expect("lock");
        assert_eq!(
            recent.back().copied(),
            Some(0.9),
            "the tail is what is live"
        );
        assert_eq!(recent.len(), LIVE_WINDOW_SAMPLES);
    }

    #[test]
    fn the_meter_returns_to_silence_once_writing_ends() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("take.wav");

        let shared = Arc::new(Shared::default());
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(8);
        tx.send(vec![0.9; 4_800]).expect("send");
        drop(tx);

        write_loop(Some(&path), Vec::new(), rx, 48_000, &shared).expect("write loop");
        assert_eq!(
            shared.level(),
            Level::default(),
            "a stopped meter must read zero"
        );
    }
}
