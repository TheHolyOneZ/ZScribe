const TAPS: usize = 16;

pub struct Resampler {
    step: f64,

    cutoff: f64,

    buffer: Vec<f32>,

    position: f64,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        let step = f64::from(input_rate) / f64::from(output_rate);

        Self {
            step,

            cutoff: (1.0 / step).min(1.0),

            buffer: vec![0.0; TAPS],
            position: TAPS as f64,
        }
    }

    pub fn is_identity(&self) -> bool {
        (self.step - 1.0).abs() < f64::EPSILON
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.is_identity() {
            return input.to_vec();
        }

        self.buffer.extend_from_slice(input);

        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 1);

        let limit = self.buffer.len().saturating_sub(TAPS) as f64;
        while self.position < limit {
            out.push(self.sample_at(self.position));
            self.position += self.step;
        }

        let keep_from = (self.position as usize).saturating_sub(TAPS);
        if keep_from > 0 {
            self.buffer.drain(..keep_from);
            self.position -= keep_from as f64;
        }

        out
    }

    pub fn finish(&mut self) -> Vec<f32> {
        if self.is_identity() {
            return Vec::new();
        }

        self.buffer.extend(std::iter::repeat_n(0.0, TAPS));
        let limit = self.buffer.len().saturating_sub(TAPS) as f64;

        let mut out = Vec::new();
        while self.position < limit {
            out.push(self.sample_at(self.position));
            self.position += self.step;
        }
        out
    }

    fn sample_at(&self, position: f64) -> f32 {
        let centre = position.floor() as usize;
        let fraction = position - position.floor();

        let mut sum = 0.0;
        let mut weight = 0.0;

        for tap in 0..=(2 * TAPS) {
            let index = centre + tap - TAPS;
            let Some(sample) = self.buffer.get(index) else {
                continue;
            };

            let offset = (tap as f64 - TAPS as f64) - fraction;
            let coefficient = kernel(offset, self.cutoff);

            sum += f64::from(*sample) * coefficient;
            weight += coefficient;
        }

        if weight.abs() < 1e-9 {
            0.0
        } else {
            (sum / weight) as f32
        }
    }
}

fn kernel(offset: f64, cutoff: f64) -> f64 {
    let width = TAPS as f64;
    if offset.abs() > width {
        return 0.0;
    }

    let window = {
        let x = (offset + width) / (2.0 * width);
        0.42 - 0.5 * (2.0 * std::f64::consts::PI * x).cos()
            + 0.08 * (4.0 * std::f64::consts::PI * x).cos()
    };

    sinc(cutoff * offset) * window
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        return 1.0;
    }
    let pi_x = std::f64::consts::PI * x;
    pi_x.sin() / pi_x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency: f64, rate: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let t = i as f64 / f64::from(rate);
                (2.0 * std::f64::consts::PI * frequency * t).sin() as f32
            })
            .collect()
    }

    fn peak(samples: &[f32]) -> f32 {
        let margin = TAPS * 4;
        if samples.len() <= margin * 2 {
            return 0.0;
        }
        samples[margin..samples.len() - margin]
            .iter()
            .fold(0.0f32, |max, s| max.max(s.abs()))
    }

    fn dominant_frequency(samples: &[f32], rate: u32) -> f64 {
        let margin = TAPS * 4;
        let body = &samples[margin..samples.len() - margin];

        let crossings = body
            .windows(2)
            .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
            .count();

        let seconds = body.len() as f64 / f64::from(rate);
        crossings as f64 / 2.0 / seconds
    }

    #[test]
    fn a_matching_rate_passes_through_untouched() {
        let mut resampler = Resampler::new(16_000, 16_000);
        assert!(resampler.is_identity());

        let input = sine(440.0, 16_000, 1_000);
        assert_eq!(resampler.process(&input), input);
    }

    #[test]
    fn forty_eight_kilohertz_speech_becomes_sixteen() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(440.0, 48_000, 48_000));

        assert!(
            (15_900..=16_100).contains(&out.len()),
            "expected about 16000 samples, got {}",
            out.len()
        );
    }

    #[test]
    fn the_tone_survives_the_conversion_at_the_right_pitch() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(440.0, 48_000, 48_000));

        let frequency = dominant_frequency(&out, 16_000);
        assert!(
            (430.0..=450.0).contains(&frequency),
            "440 Hz became {frequency:.1} Hz"
        );
    }

    #[test]
    fn the_conversion_neither_boosts_nor_attenuates_speech() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(300.0, 48_000, 48_000));

        let amplitude = peak(&out);
        assert!(
            (0.95..=1.05).contains(&amplitude),
            "unit-amplitude input became {amplitude:.3}"
        );
    }

    #[test]
    fn a_tone_above_the_new_nyquist_is_filtered_out_rather_than_aliased() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(10_000.0, 48_000, 48_000));

        let amplitude = peak(&out);
        assert!(
            amplitude < 0.1,
            "a 10 kHz tone should be suppressed, got amplitude {amplitude:.3}"
        );
    }

    #[test]
    fn a_tone_just_under_the_new_nyquist_still_passes() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(3_000.0, 48_000, 48_000));
        assert!(peak(&out) > 0.8, "3 kHz is squarely inside speech");
    }

    #[test]
    fn the_awkward_forty_four_one_ratio_works_too() {
        let mut resampler = Resampler::new(44_100, 16_000);
        let out = resampler.process(&sine(440.0, 44_100, 44_100));

        assert!(
            (15_900..=16_100).contains(&out.len()),
            "expected about 16000 samples, got {}",
            out.len()
        );
        let frequency = dominant_frequency(&out, 16_000);
        assert!(
            (430.0..=450.0).contains(&frequency),
            "got {frequency:.1} Hz"
        );
    }

    #[test]
    fn upsampling_from_eight_kilohertz_works_without_aliasing() {
        let mut resampler = Resampler::new(8_000, 16_000);
        let out = resampler.process(&sine(440.0, 8_000, 8_000));

        assert!(
            (15_900..=16_100).contains(&out.len()),
            "expected about 16000 samples, got {}",
            out.len()
        );
        assert!(peak(&out) > 0.9);
    }

    #[test]
    fn feeding_the_same_audio_in_small_chunks_gives_the_same_result() {
        let input = sine(440.0, 48_000, 24_000);

        let whole = Resampler::new(48_000, 16_000).process(&input);

        let mut chunked = Resampler::new(48_000, 16_000);
        let mut pieces = Vec::new();
        for chunk in input.chunks(511) {
            pieces.extend(chunked.process(chunk));
        }

        assert_eq!(whole.len(), pieces.len());
        for (index, (a, b)) in whole.iter().zip(&pieces).enumerate() {
            assert!((a - b).abs() < 1e-6, "sample {index} differs: {a} vs {b}");
        }
    }

    #[test]
    fn the_tail_is_flushed_rather_than_dropped() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let mut out = resampler.process(&sine(440.0, 48_000, 4_800));
        let before = out.len();

        out.extend(resampler.finish());
        assert!(out.len() > before, "the last few milliseconds must survive");
    }

    #[test]
    fn silence_in_is_silence_out() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&vec![0.0; 48_000]);
        assert!(out.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn an_empty_buffer_produces_nothing_and_does_not_panic() {
        let mut resampler = Resampler::new(48_000, 16_000);
        assert!(resampler.process(&[]).is_empty());
    }

    #[test]
    fn memory_does_not_grow_across_a_long_recording() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let chunk = sine(440.0, 48_000, 4_800);

        for _ in 0..100 {
            resampler.process(&chunk);
        }
        assert!(
            resampler.buffer.len() < 10_000,
            "buffer grew to {}",
            resampler.buffer.len()
        );
    }
}
