use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const NOTES: [(f32, u64); 2] = [(880.0, 120), (1174.7, 160)];

const AMPLITUDE: f32 = 0.22;

pub fn play_start_tone() {
    if let Err(err) = play() {
        tracing::debug!(%err, "could not play the recording tone");
    }
}

fn play() -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no output device".to_owned())?;

    let config = device
        .default_output_config()
        .map_err(|err| err.to_string())?;

    let rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    let samples = Arc::new(render(rate));
    let total = samples.len();
    let cursor = Arc::new(AtomicUsize::new(0));

    let stream_config: cpal::StreamConfig = config.into();
    let playback = Arc::clone(&samples);
    let position = Arc::clone(&cursor);

    let stream = device
        .build_output_stream(
            stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    let index = position.fetch_add(1, Ordering::Relaxed);
                    let value = playback.get(index).copied().unwrap_or(0.0);

                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                }
            },
            |err: cpal::Error| tracing::debug!(%err, "tone playback failed"),
            None,
        )
        .map_err(|err| err.to_string())?;

    stream.play().map_err(|err| err.to_string())?;

    let millis = (total as f32 / rate * 1000.0) as u64;
    std::thread::sleep(Duration::from_millis(millis + 120));

    Ok(())
}

fn render(rate: f32) -> Vec<f32> {
    let mut out = Vec::new();

    for (frequency, millis) in NOTES {
        let count = (rate * millis as f32 / 1000.0) as usize;

        for index in 0..count {
            let t = index as f32 / rate;

            let progress = index as f32 / count as f32;
            let envelope = (progress * std::f32::consts::PI).sin();

            out.push((2.0 * std::f32::consts::PI * frequency * t).sin() * envelope * AMPLITUDE);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tone_is_short_enough_not_to_delay_the_recording() {
        let total: u64 = NOTES.iter().map(|(_, millis)| millis).sum();
        assert!(
            total <= 400,
            "{total} ms is too long to wait before recording"
        );
    }

    #[test]
    fn the_tone_rises_so_it_reads_as_starting_rather_than_stopping() {
        assert!(NOTES[1].0 > NOTES[0].0);
    }

    #[test]
    fn rendering_produces_the_expected_number_of_samples() {
        let rate = 48_000.0;
        let samples = render(rate);

        let expected: usize = NOTES
            .iter()
            .map(|(_, millis)| (rate * *millis as f32 / 1000.0) as usize)
            .sum();
        assert_eq!(samples.len(), expected);
    }

    #[test]
    fn the_tone_is_audible_but_not_startling() {
        let peak = render(48_000.0)
            .iter()
            .fold(0.0f32, |max, s| max.max(s.abs()));
        assert!(peak > 0.1, "too quiet to be heard across a table: {peak}");
        assert!(peak <= AMPLITUDE + 0.01, "louder than intended: {peak}");
    }

    #[test]
    fn each_note_fades_in_and_out_rather_than_clicking() {
        let samples = render(48_000.0);

        assert!(samples[0].abs() < 0.01, "starts with a click");
        assert!(samples[samples.len() - 1].abs() < 0.01, "ends with a click");
    }

    #[test]
    fn rendering_works_at_any_sample_rate() {
        for rate in [8_000.0, 44_100.0, 48_000.0, 96_000.0] {
            assert!(!render(rate).is_empty(), "{rate} Hz produced nothing");
        }
    }
}
