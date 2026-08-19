use std::path::Path;

use zscribe_core::voices::{self, Utterance, VoiceOptions};
use zscribe_core::Transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heard {
    pub speakers: usize,

    pub unattributed: usize,
}

pub fn label(audio: &Path, transcript: &mut Transcript, options: &VoiceOptions) -> Heard {
    if transcript
        .segments
        .iter()
        .any(|segment| segment.speaker.is_some())
    {
        return Heard {
            speakers: zscribe_core::speakers(transcript).len(),
            unattributed: 0,
        };
    }

    let samples = match zscribe_audio::read_mono(audio) {
        Ok(samples) => samples,
        Err(err) => {
            tracing::warn!(%err, "could not read the audio to tell voices apart");
            return Heard {
                speakers: 0,
                unattributed: transcript.segments.len(),
            };
        }
    };

    let frames = zscribe_audio::Frames::of(&samples);
    if frames.is_empty() {
        return Heard {
            speakers: 0,
            unattributed: transcript.segments.len(),
        };
    }

    let utterances: Vec<Utterance> = transcript
        .segments
        .iter()
        .map(|segment| Utterance {
            print: frames
                .print(segment.start_ms, segment.end_ms)
                .unwrap_or_default(),
            frames: frames.frames_in(segment.start_ms, segment.end_ms),
        })
        .collect();

    let labels = voices::cluster(&utterances, options);
    let found = labels
        .iter()
        .flatten()
        .collect::<std::collections::HashSet<_>>();

    if found.len() <= 1 {
        return Heard {
            speakers: found.len(),
            unattributed: labels.iter().filter(|label| label.is_none()).count(),
        };
    }

    for (segment, label) in transcript.segments.iter_mut().zip(&labels) {
        segment.speaker = label.map(voices::default_name);
    }

    Heard {
        speakers: found.len(),
        unattributed: labels.iter().filter(|label| label.is_none()).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zscribe_core::Segment;

    fn segment(start_ms: u32, end_ms: u32, speaker: Option<&str>) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: "words".to_owned(),
            speaker: speaker.map(str::to_owned),
        }
    }

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "test".to_owned(),
            segments,
        }
    }

    fn write_voices(path: &Path, turns: &[(f32, f32, f32)]) {
        let mut writer = zscribe_audio::wav::WavWriter::create(path).expect("create");

        for (seconds, pitch, formant) in turns {
            let count = (zscribe_audio::SAMPLE_RATE as f32 * seconds) as usize;
            let samples: Vec<f32> = (0..count)
                .map(|n| {
                    let t = n as f32 / zscribe_audio::SAMPLE_RATE as f32;
                    let buzz = (t * pitch * std::f32::consts::TAU).sin();
                    let shaped = (t * formant * std::f32::consts::TAU).sin() * 0.6;
                    let upper = (t * formant * 2.1 * std::f32::consts::TAU).sin() * 0.25;
                    (buzz + shaped + upper) * 0.3
                })
                .collect();
            writer.write(&samples).expect("write");
        }

        writer.finish().expect("finish");
    }

    #[test]
    fn two_voices_taking_turns_are_labelled_as_two_people() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("meeting.wav");

        write_voices(
            &path,
            &[
                (2.0, 110.0, 600.0),
                (2.0, 220.0, 1_500.0),
                (2.0, 110.0, 600.0),
                (2.0, 220.0, 1_500.0),
            ],
        );

        let mut transcript = transcript(vec![
            segment(100, 1_900, None),
            segment(2_100, 3_900, None),
            segment(4_100, 5_900, None),
            segment(6_100, 7_900, None),
        ]);

        let heard = label(&path, &mut transcript, &VoiceOptions::default());

        assert_eq!(heard.speakers, 2, "expected two voices");
        let who: Vec<Option<String>> = transcript
            .segments
            .iter()
            .map(|segment| segment.speaker.clone())
            .collect();

        assert_eq!(who[0], who[2], "the first voice returns: {who:?}");
        assert_eq!(who[1], who[3], "the second voice returns: {who:?}");
        assert_ne!(who[0], who[1], "the two were merged: {who:?}");
        assert_eq!(
            who[0].as_deref(),
            Some("Speaker 1"),
            "first heard, first named"
        );
    }

    #[test]
    fn one_voice_is_left_unlabelled_rather_than_called_speaker_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.wav");
        write_voices(&path, &[(6.0, 130.0, 800.0)]);

        let mut transcript = transcript(vec![
            segment(100, 1_900, None),
            segment(2_100, 3_900, None),
            segment(4_100, 5_900, None),
        ]);

        let heard = label(&path, &mut transcript, &VoiceOptions::default());

        assert_eq!(heard.speakers, 1);
        assert!(
            transcript.segments.iter().all(|s| s.speaker.is_none()),
            "a solo recording was labelled",
        );
    }

    #[test]
    fn named_microphones_are_never_overwritten_by_a_guess() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("meeting.wav");
        write_voices(&path, &[(2.0, 110.0, 600.0), (2.0, 220.0, 1_500.0)]);

        let mut transcript = transcript(vec![
            segment(100, 1_900, Some("Anna")),
            segment(2_100, 3_900, Some("Max")),
        ]);

        let heard = label(&path, &mut transcript, &VoiceOptions::default());

        assert_eq!(heard.speakers, 2);
        assert_eq!(transcript.segments[0].speaker.as_deref(), Some("Anna"));
        assert_eq!(transcript.segments[1].speaker.as_deref(), Some("Max"));
    }

    #[test]
    fn a_missing_audio_file_leaves_the_transcript_alone() {
        let mut transcript = transcript(vec![segment(0, 1_000, None)]);
        let heard = label(
            Path::new("/nonexistent/gone.wav"),
            &mut transcript,
            &VoiceOptions::default(),
        );

        assert_eq!(heard.speakers, 0);
        assert!(transcript.segments[0].speaker.is_none());
    }
}
