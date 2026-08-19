use std::path::{Path, PathBuf};

use crate::AudioError;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;

pub struct WavWriter {
    inner: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    path: PathBuf,
    frames: u64,
}

impl WavWriter {
    pub fn create(path: &Path) -> Result<Self, AudioError> {
        let spec = hound::WavSpec {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let inner = hound::WavWriter::create(path, spec).map_err(|source| AudioError::Write {
            path: path.display().to_string(),
            message: source.to_string(),
        })?;

        Ok(Self {
            inner: Some(inner),
            path: path.to_path_buf(),
            frames: 0,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        let Some(writer) = self.inner.as_mut() else {
            return Ok(());
        };

        for sample in samples {
            writer
                .write_sample(to_i16(*sample))
                .map_err(|source| AudioError::Write {
                    path: self.path.display().to_string(),
                    message: source.to_string(),
                })?;
        }
        self.frames += samples.len() as u64;
        Ok(())
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn duration_ms(&self) -> u32 {
        ((self.frames * 1000) / u64::from(SAMPLE_RATE)) as u32
    }

    pub fn finish(mut self) -> Result<u32, AudioError> {
        let duration_ms = self.duration_ms();

        if let Some(writer) = self.inner.take() {
            writer.finalize().map_err(|source| AudioError::Write {
                path: self.path.display().to_string(),
                message: source.to_string(),
            })?;
        }
        Ok(duration_ms)
    }
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

pub fn read_mono(path: &Path) -> Result<Vec<f32>, AudioError> {
    let mut reader = hound::WavReader::open(path).map_err(|source| AudioError::Read {
        path: path.display().to_string(),
        message: source.to_string(),
    })?;

    let spec = reader.spec();
    let read = |result: Result<i16, hound::Error>| {
        result
            .map(|sample| f32::from(sample) / f32::from(i16::MAX))
            .map_err(|source| AudioError::Read {
                path: path.display().to_string(),
                message: source.to_string(),
            })
    };

    let samples: Result<Vec<f32>, AudioError> = match spec.sample_format {
        hound::SampleFormat::Int => reader.samples::<i16>().map(read).collect(),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|result| {
                result.map_err(|source| AudioError::Read {
                    path: path.display().to_string(),
                    message: source.to_string(),
                })
            })
            .collect(),
    };
    let samples = samples?;

    if spec.channels > 1 {
        return Ok(samples
            .chunks(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect());
    }
    Ok(samples)
}

pub fn peaks(path: &Path, buckets: usize) -> Result<Vec<u8>, AudioError> {
    let mut reader = hound::WavReader::open(path).map_err(|source| AudioError::Read {
        path: path.display().to_string(),
        message: source.to_string(),
    })?;

    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let frames = reader.len() as usize / channels;

    if buckets == 0 || frames == 0 {
        return Ok(Vec::new());
    }

    let per_bucket = frames.div_ceil(buckets);

    let samples: Box<dyn Iterator<Item = Result<f32, hound::Error>>> = match spec.sample_format {
        hound::SampleFormat::Int => Box::new(
            reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| f32::from(value) / f32::from(i16::MAX))),
        ),
        hound::SampleFormat::Float => Box::new(reader.samples::<f32>()),
    };

    let mut envelope = Vec::with_capacity(buckets);
    let mut peak = 0.0f32;
    let mut counted = 0usize;
    let mut frame = 0.0f32;
    let mut channel = 0usize;

    for sample in samples {
        frame += sample.map_err(|source| AudioError::Read {
            path: path.display().to_string(),
            message: source.to_string(),
        })?;

        channel += 1;
        if channel < channels {
            continue;
        }

        peak = peak.max((frame / channels as f32).abs());
        frame = 0.0;
        channel = 0;

        counted += 1;
        if counted == per_bucket {
            envelope.push(to_level(peak));
            peak = 0.0;
            counted = 0;
        }
    }

    if counted > 0 {
        envelope.push(to_level(peak));
    }

    Ok(envelope)
}

fn to_level(peak: f32) -> u8 {
    (peak.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_wav() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("take.wav");
        (dir, path)
    }

    #[test]
    fn a_recording_round_trips_through_the_file() {
        let (_dir, path) = temp_wav();

        let original: Vec<f32> = (0..1_600).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&original).expect("write");
        writer.finish().expect("finish");

        let read_back = read_mono(&path).expect("read");
        assert_eq!(read_back.len(), original.len());

        for (index, (a, b)) in original.iter().zip(&read_back).enumerate() {
            assert!((a - b).abs() < 1e-3, "sample {index}: {a} vs {b}");
        }
    }

    #[test]
    fn the_file_has_the_format_whisper_expects() {
        let (_dir, path) = temp_wav();

        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&[0.0; 160]).expect("write");
        writer.finish().expect("finish");

        let spec = hound::WavReader::open(&path).expect("open").spec();
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
    }

    #[test]
    fn duration_is_reported_from_the_sample_count() {
        let (_dir, path) = temp_wav();

        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&vec![0.0; 16_000 * 3]).expect("write");

        assert_eq!(writer.frames(), 48_000);
        assert_eq!(writer.duration_ms(), 3_000);
        assert_eq!(writer.finish().expect("finish"), 3_000);
    }

    #[test]
    fn writing_in_many_small_pieces_gives_the_same_file() {
        let (_dir, path) = temp_wav();

        let mut writer = WavWriter::create(&path).expect("create");
        for _ in 0..100 {
            writer.write(&[0.25; 16]).expect("write");
        }
        writer.finish().expect("finish");

        assert_eq!(read_mono(&path).expect("read").len(), 1_600);
    }

    #[test]
    fn a_sample_beyond_full_scale_clamps_instead_of_wrapping() {
        assert_eq!(to_i16(1.5), i16::MAX);
        assert_eq!(to_i16(-1.5), -i16::MAX);
        assert_eq!(to_i16(0.0), 0);
    }

    #[test]
    fn an_empty_recording_is_still_a_valid_file() {
        let (_dir, path) = temp_wav();

        let writer = WavWriter::create(&path).expect("create");
        assert_eq!(writer.finish().expect("finish"), 0);

        assert!(read_mono(&path).expect("read").is_empty());
    }

    #[test]
    fn a_stereo_file_dropped_in_is_downmixed_rather_than_refused() {
        let (dir, _) = temp_wav();
        let path = dir.path().join("stereo.wav");

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("create");
        for _ in 0..100 {
            writer.write_sample(i16::MAX).expect("left");
            writer.write_sample(0i16).expect("right");
        }
        writer.finalize().expect("finalize");

        let mono = read_mono(&path).expect("read");
        assert_eq!(mono.len(), 100, "two channels become one");
        assert!(
            (mono[0] - 0.5).abs() < 1e-3,
            "hard-left becomes centre at half"
        );
    }

    #[test]
    fn the_envelope_follows_the_loudness_it_was_drawn_from() {
        let (_dir, path) = temp_wav();

        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&vec![0.1; 8_000]).expect("quiet");
        writer.write(&vec![0.9; 8_000]).expect("loud");
        writer.finish().expect("finish");

        let envelope = peaks(&path, 4).expect("peaks");
        assert_eq!(envelope.len(), 4);

        assert!(envelope[0] < 40, "quiet reads quiet: {envelope:?}");
        assert!(envelope[1] < 40, "quiet reads quiet: {envelope:?}");
        assert!(envelope[2] > 200, "loud reads loud: {envelope:?}");
        assert!(envelope[3] > 200, "loud reads loud: {envelope:?}");
    }

    #[test]
    fn a_bucket_takes_the_peak_and_not_the_average() {
        let (_dir, path) = temp_wav();

        let mut samples = vec![0.0f32; 1_600];
        samples[800] = 1.0;

        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&samples).expect("write");
        writer.finish().expect("finish");

        assert_eq!(peaks(&path, 1).expect("peaks"), vec![255]);
    }

    #[test]
    fn the_tail_of_a_recording_is_not_dropped() {
        let (_dir, path) = temp_wav();

        let mut writer = WavWriter::create(&path).expect("create");
        writer.write(&vec![0.5; 1_000]).expect("write");
        writer.finish().expect("finish");

        assert_eq!(peaks(&path, 3).expect("peaks").len(), 3);
    }

    #[test]
    fn an_empty_or_unasked_for_envelope_is_empty_rather_than_an_error() {
        let (_dir, path) = temp_wav();
        WavWriter::create(&path)
            .expect("create")
            .finish()
            .expect("finish");

        assert!(peaks(&path, 600).expect("peaks").is_empty());

        let (_other, second) = temp_wav();
        let mut writer = WavWriter::create(&second).expect("create");
        writer.write(&vec![0.5; 100]).expect("write");
        writer.finish().expect("finish");
        assert!(peaks(&second, 0).expect("peaks").is_empty());
    }

    #[test]
    fn a_stereo_file_is_downmixed_for_the_envelope_too() {
        let (dir, _) = temp_wav();
        let path = dir.path().join("stereo.wav");

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("create");
        for _ in 0..100 {
            writer.write_sample(i16::MAX).expect("left");
            writer.write_sample(0i16).expect("right");
        }
        writer.finalize().expect("finalize");

        let envelope = peaks(&path, 1).expect("peaks");
        assert_eq!(envelope.len(), 1);
        assert!(
            (i16::from(envelope[0]) - 128).abs() <= 2,
            "got: {envelope:?}"
        );
    }

    #[test]
    fn reading_a_file_that_is_not_there_reports_the_path() {
        let error = read_mono(Path::new("/nonexistent/take.wav")).expect_err("should fail");
        assert!(error.to_string().contains("take.wav"), "got: {error}");
    }
}
