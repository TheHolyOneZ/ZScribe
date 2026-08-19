use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

use crate::wav::SAMPLE_RATE;

const FRAME: usize = (SAMPLE_RATE as usize * 25) / 1000;

const HOP: usize = (SAMPLE_RATE as usize * 10) / 1000;

const FFT: usize = 512;

const FILTERS: usize = 26;

pub const COEFFICIENTS: usize = 13;

const LOW_HZ: f32 = 80.0;
const HIGH_HZ: f32 = 7_600.0;

pub type Voiceprint = Vec<f32>;

pub struct Frames {
    coefficients: Vec<[f32; COEFFICIENTS]>,
}

impl Frames {
    pub fn of(samples: &[f32]) -> Self {
        let mut coefficients = mfcc(samples);
        normalise(&mut coefficients);
        Self { coefficients }
    }

    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }

    pub fn frames_in(&self, start_ms: u32, end_ms: u32) -> usize {
        let (from, to) = self.range(start_ms, end_ms);
        to.saturating_sub(from)
    }

    pub fn print(&self, start_ms: u32, end_ms: u32) -> Option<Voiceprint> {
        let (from, to) = self.range(start_ms, end_ms);
        let window = self.coefficients.get(from..to)?;

        if window.is_empty() {
            return None;
        }

        let mut mean = [0.0f32; COEFFICIENTS];
        for frame in window {
            for (slot, value) in mean.iter_mut().zip(frame) {
                *slot += value;
            }
        }
        for slot in &mut mean {
            *slot /= window.len() as f32;
        }

        let mut spread = [0.0f32; COEFFICIENTS];
        for frame in window {
            for (slot, (value, average)) in spread.iter_mut().zip(frame.iter().zip(&mean)) {
                *slot += (value - average).powi(2);
            }
        }
        for slot in &mut spread {
            *slot = (*slot / window.len() as f32).sqrt();
        }

        Some(mean.into_iter().chain(spread).collect())
    }

    fn range(&self, start_ms: u32, end_ms: u32) -> (usize, usize) {
        let per_ms = SAMPLE_RATE as f32 / 1000.0;

        let from =
            (((start_ms as f32 * per_ms) / HOP as f32) as usize).min(self.coefficients.len());
        let to =
            (((end_ms as f32 * per_ms) / HOP as f32) as usize).clamp(from, self.coefficients.len());

        (from, to)
    }
}

fn normalise(frames: &mut [[f32; COEFFICIENTS]]) {
    if frames.is_empty() {
        return;
    }

    for index in 0..COEFFICIENTS {
        let mean: f32 = frames.iter().map(|frame| frame[index]).sum::<f32>() / frames.len() as f32;

        let variance: f32 = frames
            .iter()
            .map(|frame| (frame[index] - mean).powi(2))
            .sum::<f32>()
            / frames.len() as f32;

        let deviation = variance.sqrt().max(1e-6);

        for frame in frames.iter_mut() {
            frame[index] = (frame[index] - mean) / deviation;
        }
    }
}

fn mfcc(samples: &[f32]) -> Vec<[f32; COEFFICIENTS]> {
    if samples.len() < FRAME {
        return Vec::new();
    }

    let window = hamming();
    let filters = mel_filters();
    let cosines = dct_table();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT);

    let mut out = Vec::with_capacity((samples.len() - FRAME) / HOP + 1);
    let mut buffer = vec![Complex32::new(0.0, 0.0); FFT];
    let mut energies = [0.0f32; FILTERS];

    for start in (0..=samples.len() - FRAME).step_by(HOP) {
        let frame = &samples[start..start + FRAME];

        for (slot, (sample, weight)) in buffer.iter_mut().zip(frame.iter().zip(&window)) {
            *slot = Complex32::new(sample * weight, 0.0);
        }
        for slot in buffer.iter_mut().skip(FRAME) {
            *slot = Complex32::new(0.0, 0.0);
        }

        fft.process(&mut buffer);

        for (energy, filter) in energies.iter_mut().zip(&filters) {
            let total: f32 = filter
                .iter()
                .enumerate()
                .map(|(bin, weight)| weight * buffer[bin].norm_sqr())
                .sum();

            *energy = (total + 1e-10).ln();
        }

        let mut coefficients = [0.0f32; COEFFICIENTS];
        for (index, slot) in coefficients.iter_mut().enumerate() {
            *slot = energies
                .iter()
                .zip(&cosines[index + 1])
                .map(|(energy, cosine)| energy * cosine)
                .sum();
        }

        out.push(coefficients);
    }

    out
}

fn hamming() -> Vec<f32> {
    (0..FRAME)
        .map(|n| 0.54 - 0.46 * (2.0 * std::f32::consts::PI * n as f32 / (FRAME as f32 - 1.0)).cos())
        .collect()
}

fn mel_filters() -> Vec<Vec<f32>> {
    let to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let to_hz = |mel: f32| 700.0 * (10f32.powf(mel / 2595.0) - 1.0);

    let low = to_mel(LOW_HZ);
    let high = to_mel(HIGH_HZ);
    let bins = FFT / 2 + 1;

    let points: Vec<f32> = (0..FILTERS + 2)
        .map(|index| {
            let mel = low + (high - low) * index as f32 / (FILTERS + 1) as f32;
            to_hz(mel) * FFT as f32 / SAMPLE_RATE as f32
        })
        .collect();

    (0..FILTERS)
        .map(|filter| {
            let (left, centre, right) = (points[filter], points[filter + 1], points[filter + 2]);

            (0..bins)
                .map(|bin| {
                    let position = bin as f32;
                    if position < left || position > right {
                        0.0
                    } else if position <= centre {
                        (position - left) / (centre - left).max(1e-6)
                    } else {
                        (right - position) / (right - centre).max(1e-6)
                    }
                })
                .collect()
        })
        .collect()
}

fn dct_table() -> Vec<Vec<f32>> {
    (0..=COEFFICIENTS)
        .map(|k| {
            (0..FILTERS)
                .map(|n| {
                    (std::f32::consts::PI * k as f32 * (n as f32 + 0.5) / FILTERS as f32).cos()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(seconds: f32, pitch: f32, formants: (f32, f32)) -> Vec<f32> {
        let count = (SAMPLE_RATE as f32 * seconds) as usize;
        (0..count)
            .map(|n| {
                let t = n as f32 / SAMPLE_RATE as f32;
                let buzz = (t * pitch * std::f32::consts::TAU).sin();
                let first = (t * formants.0 * std::f32::consts::TAU).sin() * 0.6;
                let second = (t * formants.1 * std::f32::consts::TAU).sin() * 0.3;
                (buzz + first + second) * 0.3
            })
            .collect()
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let left: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let right: f32 = b.iter().map(|y| y * y).sum::<f32>().sqrt();
        dot / (left * right).max(1e-9)
    }

    #[test]
    fn one_voice_looks_the_same_at_the_start_and_at_the_end() {
        let samples = voice(4.0, 120.0, (700.0, 1_200.0));
        let frames = Frames::of(&samples);

        let first = frames.print(0, 1_500).expect("a print");
        let second = frames.print(2_500, 4_000).expect("a print");

        assert!(
            cosine(&first, &second) > 0.8,
            "the same voice drifted apart: {}",
            cosine(&first, &second)
        );
    }

    #[test]
    fn two_voices_look_different_from_each_other() {
        let mut samples = voice(2.0, 110.0, (600.0, 1_000.0));
        samples.extend(voice(2.0, 210.0, (1_100.0, 2_400.0)));

        let frames = Frames::of(&samples);
        let deep = frames.print(200, 1_800).expect("a print");
        let bright = frames.print(2_200, 3_800).expect("a print");

        let same_voice = {
            let a = frames.print(200, 900).expect("a print");
            let b = frames.print(1_000, 1_800).expect("a print");
            cosine(&a, &b)
        };

        assert!(
            cosine(&deep, &bright) < same_voice,
            "two voices ({}) should be further apart than one voice with itself ({})",
            cosine(&deep, &bright),
            same_voice
        );
    }

    #[test]
    fn a_stretch_shorter_than_a_frame_has_no_print_rather_than_a_wrong_one() {
        let samples = voice(2.0, 120.0, (700.0, 1_200.0));
        let frames = Frames::of(&samples);

        assert!(frames.print(500, 500).is_none());

        assert!(frames.print(9_000, 10_000).is_none());
    }

    #[test]
    fn silence_and_a_file_too_short_to_measure_are_both_empty() {
        assert!(Frames::of(&[]).is_empty());
        assert!(Frames::of(&[0.0; 100]).is_empty());

        let quiet = Frames::of(&vec![0.0; SAMPLE_RATE as usize]);
        if let Some(print) = quiet.print(0, 900) {
            assert!(print.iter().all(|value| value.is_finite()), "{print:?}");
        }
    }

    #[test]
    fn the_length_of_a_stretch_is_measured_in_frames() {
        let samples = voice(3.0, 120.0, (700.0, 1_200.0));
        let frames = Frames::of(&samples);

        let count = frames.frames_in(0, 1_000);
        assert!((95..=105).contains(&count), "got {count}");
    }
}
