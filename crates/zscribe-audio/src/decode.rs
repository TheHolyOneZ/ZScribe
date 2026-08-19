use std::fs::File;
use std::path::Path;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::resample::Resampler;
use crate::wav::{WavWriter, SAMPLE_RATE};
use crate::AudioError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub duration_ms: Option<u32>,

    pub codec: String,
}

pub fn decode_to_wav(
    source: &Path,
    destination: &Path,
    mut on_progress: impl FnMut(u8),
) -> Result<u32, AudioError> {
    let mut reader = open(source)?;

    let track = reader
        .first_track(TrackType::Audio)
        .ok_or_else(|| AudioError::NoAudioTrack(name_of(source)))?;

    let track_id = track.id;
    let time_base = track.time_base;

    let total_ms = length_ms(track).map(f64::from);

    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| AudioError::NoAudioTrack(name_of(source)))?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|err| AudioError::Undecodable {
            path: name_of(source),
            message: err.to_string(),
        })?;

    let mut writer = WavWriter::create(destination)?;
    let mut resampler: Option<Resampler> = None;
    let mut interleaved: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    let mut reported = 0u8;

    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,

            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(err) => {
                return Err(AudioError::Undecodable {
                    path: name_of(source),
                    message: err.to_string(),
                })
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,

            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(err) => {
                return Err(AudioError::Undecodable {
                    path: name_of(source),
                    message: err.to_string(),
                })
            }
        };

        if decoded.frames() == 0 {
            continue;
        }

        let rate = decoded.spec().rate();
        let channels = decoded.spec().channels().count().max(1);

        to_mono(&decoded, channels, &mut interleaved, &mut mono);

        let resampler = resampler.get_or_insert_with(|| Resampler::new(rate, SAMPLE_RATE));
        let ready = resampler.process(&mono);
        writer.write(&ready)?;

        if let Some(total) = total_ms {
            let done = time_base
                .and_then(|base| base.calc_time(packet.pts))
                .map(|time| time.as_secs_f64() * 1000.0)
                .unwrap_or(0.0);

            let percent = ((done / total.max(1.0)) * 100.0).clamp(0.0, 100.0) as u8;
            if percent > reported {
                reported = percent;
                on_progress(percent);
            }
        }
    }

    if let Some(mut resampler) = resampler {
        let tail = resampler.finish();
        if !tail.is_empty() {
            writer.write(&tail)?;
        }
    }

    on_progress(100);

    let duration_ms = writer.finish()?;
    if duration_ms == 0 {
        return Err(AudioError::NoAudioTrack(name_of(source)));
    }

    Ok(duration_ms)
}

pub fn inspect(source: &Path) -> Result<MediaInfo, AudioError> {
    let reader = open(source)?;

    let track = reader
        .first_track(TrackType::Audio)
        .ok_or_else(|| AudioError::NoAudioTrack(name_of(source)))?;

    let duration_ms = length_ms(track);

    let codec = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .and_then(|audio| symphonia::default::get_codecs().get_audio_decoder(audio.codec))
        .map(|registered| registered.codec.info.short_name.to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    Ok(MediaInfo { duration_ms, codec })
}

fn length_ms(track: &symphonia::core::formats::Track) -> Option<u32> {
    let base = track.time_base?;
    let time = base.calc_duration(track.duration?)?;
    u32::try_from((time.as_secs_f64() * 1000.0) as i64).ok()
}

fn open(source: &Path) -> Result<Box<dyn symphonia::core::formats::FormatReader>, AudioError> {
    let file = File::open(source).map_err(|err| AudioError::Read {
        path: source.display().to_string(),
        message: err.to_string(),
    })?;

    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = source.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(extension);
    }

    symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|err| AudioError::Undecodable {
            path: name_of(source),
            message: err.to_string(),
        })
}

fn to_mono(
    decoded: &GenericAudioBufferRef<'_>,
    channels: usize,
    interleaved: &mut Vec<f32>,
    mono: &mut Vec<f32>,
) {
    interleaved.clear();
    decoded.copy_to_vec_interleaved(interleaved);
    mix_frames(interleaved, channels, mono);
}

fn mix_frames(interleaved: &[f32], channels: usize, mono: &mut Vec<f32>) {
    mono.clear();

    if channels <= 1 {
        mono.extend_from_slice(interleaved);
        return;
    }

    mono.reserve(interleaved.len() / channels);
    for frame in interleaved.chunks_exact(channels) {
        mono.push(frame.iter().sum::<f32>() / channels as f32);
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("that file")
        .to_owned()
}

pub const IMPORTABLE_EXTENSIONS: [&str; 12] = [
    "wav", "mp3", "m4a", "mp4", "aac", "flac", "ogg", "oga", "opus", "mkv", "webm", "aiff",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_channel_reaches_the_mix() {
        let mut mono = Vec::new();

        mix_frames(&[1.0, 0.0, 0.5, 0.5, 0.0, 1.0], 2, &mut mono);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);

        mix_frames(&[0.0, 0.0, 1.0, 0.0, 0.0], 5, &mut mono);
        assert_eq!(mono, vec![0.2]);
    }

    #[test]
    fn mono_is_passed_through_untouched() {
        let mut mono = Vec::new();
        mix_frames(&[0.25, -0.5, 1.0], 1, &mut mono);
        assert_eq!(mono, vec![0.25, -0.5, 1.0]);
    }

    #[test]
    fn a_trailing_partial_frame_is_dropped_rather_than_mixed_wrongly() {
        let mut mono = Vec::new();
        mix_frames(&[1.0, 1.0, 1.0], 2, &mut mono);
        assert_eq!(mono, vec![1.0]);
    }

    #[test]
    fn every_offered_extension_is_lowercase_and_bare() {
        for extension in IMPORTABLE_EXTENSIONS {
            assert!(!extension.starts_with('.'), "{extension} has a leading dot");
            assert_eq!(extension.to_lowercase(), extension);
        }
    }

    #[test]
    fn a_missing_file_is_reported_as_unreadable_not_as_undecodable() {
        let err = inspect(Path::new("/nonexistent/nothing.mp3")).unwrap_err();
        assert!(matches!(err, AudioError::Read { .. }), "got {err:?}");
    }
}
