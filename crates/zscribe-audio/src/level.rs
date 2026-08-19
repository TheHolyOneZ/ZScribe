use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
}

impl Level {
    pub fn of(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sum_squares = 0.0f64;
        let mut peak = 0.0f32;

        for sample in samples {
            sum_squares += f64::from(*sample) * f64::from(*sample);
            peak = peak.max(sample.abs());
        }

        Self {
            rms: (sum_squares / samples.len() as f64).sqrt() as f32,
            peak: peak.min(1.0),
        }
    }

    pub const CLIP_THRESHOLD: f32 = 0.99;

    pub fn is_clipping(self) -> bool {
        self.peak >= Self::CLIP_THRESHOLD
    }

    pub const SILENCE_THRESHOLD: f32 = 0.001;

    pub fn is_silent(self) -> bool {
        self.peak < Self::SILENCE_THRESHOLD
    }

    pub fn smoothed(self, next: Self) -> Self {
        const FALL: f32 = 0.6;

        Self {
            rms: if next.rms > self.rms {
                next.rms
            } else {
                self.rms * FALL + next.rms * (1.0 - FALL)
            },
            peak: if next.peak > self.peak {
                next.peak
            } else {
                self.peak * FALL + next.peak * (1.0 - FALL)
            },
        }
    }
}

pub fn levels_for(samples: &[f32], windows: &[(u32, u32)], sample_rate: u32) -> Vec<f32> {
    windows
        .iter()
        .map(|(start_ms, end_ms)| {
            let index = |ms: u32| {
                ((ms as u64 * sample_rate as u64) / 1000).min(samples.len() as u64) as usize
            };

            let (from, to) = (index(*start_ms), index(*end_ms));
            if from >= to {
                return 0.0;
            }
            Level::of(&samples[from..to]).rms
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| amplitude * (i as f32 * 0.1).sin())
            .collect()
    }

    #[test]
    fn segment_levels_follow_the_audio_under_each_window() {
        let mut samples = vec![0.02f32; 16_000];
        samples.extend(sine(0.8, 16_000));

        let levels = levels_for(&samples, &[(0, 1_000), (1_000, 2_000)], 16_000);

        assert_eq!(levels.len(), 2);
        assert!(levels[0] < 0.05, "the quiet window read {}", levels[0]);
        assert!(levels[1] > 0.3, "the loud window read {}", levels[1]);
        assert!(levels[1] > levels[0] * 5.0, "the two must be clearly apart");
    }

    #[test]
    fn a_window_past_the_end_of_the_audio_reads_zero_rather_than_panicking() {
        let samples = vec![0.5f32; 16_000];
        let levels = levels_for(&samples, &[(0, 1_000), (5_000, 6_000)], 16_000);

        assert!(levels[0] > 0.0);
        assert_eq!(levels[1], 0.0);
    }

    #[test]
    fn a_window_that_ends_before_it_starts_reads_zero() {
        assert_eq!(levels_for(&[0.5; 1_000], &[(900, 100)], 16_000), vec![0.0]);
    }

    #[test]
    fn a_window_running_past_the_end_measures_what_is_there() {
        let samples = vec![0.5f32; 16_000];
        assert!(levels_for(&samples, &[(500, 2_000)], 16_000)[0] > 0.0);
    }

    #[test]
    fn silence_reads_as_zero_on_both_meters() {
        let level = Level::of(&[0.0; 512]);
        assert_eq!(level.rms, 0.0);
        assert_eq!(level.peak, 0.0);
        assert!(level.is_silent());
    }

    #[test]
    fn an_empty_buffer_does_not_divide_by_zero() {
        assert_eq!(Level::of(&[]), Level::default());
    }

    #[test]
    fn a_full_scale_sine_has_the_expected_rms() {
        let level = Level::of(&sine(1.0, 100_000));
        assert!(
            (level.rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01,
            "got {}",
            level.rms
        );
        assert!(level.peak > 0.99);
    }

    #[test]
    fn peak_follows_the_loudest_sample_not_the_average() {
        let mut samples = vec![0.0; 1_000];
        samples[500] = 0.9;

        let level = Level::of(&samples);
        assert!((level.peak - 0.9).abs() < 1e-6);
        assert!(level.rms < 0.05, "one spike must not raise the average");
    }

    #[test]
    fn a_negative_peak_counts_as_loud_as_a_positive_one() {
        assert_eq!(Level::of(&[-0.8, 0.1]).peak, 0.8);
    }

    #[test]
    fn clipping_is_detected() {
        assert!(Level::of(&[1.0, 0.2]).is_clipping());
        assert!(!Level::of(&[0.8, 0.2]).is_clipping());
    }

    #[test]
    fn a_sample_beyond_full_scale_is_clamped_rather_than_overflowing_the_meter() {
        let level = Level::of(&[1.4, -1.9]);
        assert_eq!(level.peak, 1.0);
        assert!(level.is_clipping());
    }

    #[test]
    fn a_quiet_room_is_not_mistaken_for_a_muted_microphone() {
        let level = Level::of(&sine(0.02, 1_000));
        assert!(!level.is_silent(), "0.02 is quiet speech, not silence");
    }

    #[test]
    fn the_meter_rises_instantly_and_falls_gradually() {
        let quiet = Level {
            rms: 0.1,
            peak: 0.1,
        };
        let loud = Level {
            rms: 0.9,
            peak: 0.9,
        };

        assert_eq!(quiet.smoothed(loud), loud);

        let falling = loud.smoothed(quiet);
        assert!(
            falling.rms > quiet.rms && falling.rms < loud.rms,
            "got {}",
            falling.rms
        );
    }

    #[test]
    fn repeated_smoothing_settles_at_the_new_level() {
        let mut level = Level {
            rms: 0.9,
            peak: 0.9,
        };
        let quiet = Level {
            rms: 0.0,
            peak: 0.0,
        };

        for _ in 0..100 {
            level = level.smoothed(quiet);
        }
        assert!(
            level.rms < 0.001,
            "the meter must reach zero, got {}",
            level.rms
        );
    }
}
