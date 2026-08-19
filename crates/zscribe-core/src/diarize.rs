use crate::transcript::{Segment, Transcript};

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub speaker: String,

    pub transcript: Transcript,

    pub levels: Vec<f32>,
}

impl Track {
    fn level_of(&self, index: usize) -> f32 {
        self.levels.get(index).copied().unwrap_or(0.0)
    }
}

const LOUDER_BY: f32 = 1.25;

const SAME_WORDING: f32 = 0.6;

fn same_utterance(a: &Segment, b: &Segment) -> bool {
    a.overlaps(b) && wording_overlap(&a.text, &b.text) >= SAME_WORDING
}

fn wording_overlap(a: &str, b: &str) -> f32 {
    let words = |text: &str| -> Vec<String> {
        text.split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect()
    };

    let (left, right) = (words(a), words(b));
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let (shorter, longer) = if left.len() <= right.len() {
        (&left, &right)
    } else {
        (&right, &left)
    };

    let shared = shorter.iter().filter(|word| longer.contains(word)).count();

    shared as f32 / shorter.len() as f32
}

pub fn merge(tracks: &[Track]) -> Transcript {
    let language = tracks
        .iter()
        .map(|track| track.transcript.language.clone())
        .next()
        .unwrap_or_else(|| "unknown".to_owned());

    let model = tracks
        .iter()
        .map(|track| track.transcript.model.clone())
        .next()
        .unwrap_or_default();

    if tracks.len() == 1 {
        let track = &tracks[0];
        let speaker = track.speaker.trim();

        let segments = track
            .transcript
            .segments
            .iter()
            .map(|segment| {
                let mut segment = segment.clone();
                segment.speaker = (!speaker.is_empty()).then(|| speaker.to_owned());
                segment
            })
            .collect();

        return Transcript {
            language,
            model,
            segments,
        };
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for (track_index, track) in tracks.iter().enumerate() {
        for (segment_index, segment) in track.transcript.segments.iter().enumerate() {
            if segment.text.trim().is_empty() {
                continue;
            }
            candidates.push(Candidate {
                track_index,
                level: track.level_of(segment_index),
                segment: segment.clone(),
            });
        }
    }

    candidates.sort_by_key(|candidate| candidate.segment.start_ms);

    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in candidates {
        let rival = kept
            .iter_mut()
            .rev()
            .take_while(|k| k.segment.end_ms + 2_000 >= candidate.segment.start_ms)
            .find(|k| {
                k.track_index != candidate.track_index
                    && same_utterance(&k.segment, &candidate.segment)
            });

        match rival {
            Some(existing) => {
                if candidate.level > existing.level * LOUDER_BY {
                    *existing = candidate;
                }
            }
            None => kept.push(candidate),
        }
    }

    kept.sort_by_key(|candidate| candidate.segment.start_ms);

    let segments = kept
        .into_iter()
        .map(|candidate| {
            let speaker = tracks[candidate.track_index].speaker.trim();
            let mut segment = candidate.segment;
            segment.speaker = (!speaker.is_empty()).then(|| speaker.to_owned());
            segment
        })
        .collect();

    Transcript {
        language,
        model,
        segments,
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    track_index: usize,
    level: f32,
    segment: Segment,
}

pub fn speakers(transcript: &Transcript) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for segment in &transcript.segments {
        if let Some(speaker) = &segment.speaker {
            if !out.iter().any(|seen| seen == speaker) {
                out.push(speaker.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments,
        }
    }

    fn track(speaker: &str, segments: Vec<(u32, u32, &str)>, levels: Vec<f32>) -> Track {
        Track {
            speaker: speaker.to_owned(),
            transcript: transcript(
                segments
                    .into_iter()
                    .map(|(start, end, text)| Segment::new(start, end, text))
                    .collect(),
            ),
            levels,
        }
    }

    #[test]
    fn a_single_named_microphone_attributes_everything_to_that_person() {
        let merged = merge(&[track(
            "Max Kruger",
            vec![(0, 2_000, "Morning."), (2_000, 4_000, "Shall we start?")],
            vec![0.3, 0.3],
        )]);

        assert_eq!(merged.segments.len(), 2);
        assert!(merged
            .segments
            .iter()
            .all(|s| s.speaker.as_deref() == Some("Max Kruger")));
    }

    #[test]
    fn an_unnamed_microphone_claims_nobody() {
        let merged = merge(&[track("", vec![(0, 2_000, "Morning.")], vec![0.3])]);
        assert_eq!(merged.segments[0].speaker, None);
    }

    #[test]
    fn two_people_on_their_own_microphones_are_attributed_separately() {
        let merged = merge(&[
            track("Max Kruger", vec![(0, 2_000, "Morning.")], vec![0.40]),
            track(
                "Anna Weiss",
                vec![(3_000, 5_000, "Morning, Max.")],
                vec![0.38],
            ),
        ]);

        assert_eq!(merged.segments.len(), 2);
        assert_eq!(merged.segments[0].speaker.as_deref(), Some("Max Kruger"));
        assert_eq!(merged.segments[1].speaker.as_deref(), Some("Anna Weiss"));
    }

    #[test]
    fn crosstalk_is_dropped_rather_than_duplicating_every_sentence() {
        let merged = merge(&[
            track(
                "Max Kruger",
                vec![(1_000, 3_000, "We ship on Friday.")],
                vec![0.06],
            ),
            track(
                "Anna Weiss",
                vec![(1_050, 3_100, "We ship on Friday.")],
                vec![0.42],
            ),
        ]);

        assert_eq!(merged.segments.len(), 1, "the utterance must appear once");
        assert_eq!(
            merged.segments[0].speaker.as_deref(),
            Some("Anna Weiss"),
            "the microphone that heard it loudest is the speaker's own"
        );
    }

    #[test]
    fn crosstalk_is_dropped_whichever_track_it_arrives_on_first() {
        let loud_first = merge(&[
            track(
                "Anna Weiss",
                vec![(1_000, 3_000, "We ship on Friday.")],
                vec![0.42],
            ),
            track(
                "Max Kruger",
                vec![(1_050, 3_100, "We ship on Friday.")],
                vec![0.06],
            ),
        ]);
        assert_eq!(loud_first.segments.len(), 1);
        assert_eq!(
            loud_first.segments[0].speaker.as_deref(),
            Some("Anna Weiss")
        );

        let quiet_first = merge(&[
            track(
                "Max Kruger",
                vec![(1_000, 3_000, "We ship on Friday.")],
                vec![0.06],
            ),
            track(
                "Anna Weiss",
                vec![(1_050, 3_100, "We ship on Friday.")],
                vec![0.42],
            ),
        ]);
        assert_eq!(quiet_first.segments.len(), 1);
        assert_eq!(
            quiet_first.segments[0].speaker.as_deref(),
            Some("Anna Weiss")
        );
    }

    #[test]
    fn a_whole_conversation_of_crosstalk_collapses_to_one_line_each() {
        let merged = merge(&[
            track(
                "Max Kruger",
                vec![(0, 2_000, "Morning."), (3_000, 5_000, "Morning, Max.")],
                vec![0.45, 0.05],
            ),
            track(
                "Anna Weiss",
                vec![(50, 2_050, "Morning."), (3_020, 5_100, "Morning, Max.")],
                vec![0.04, 0.44],
            ),
        ]);

        assert_eq!(merged.segments.len(), 2);
        assert_eq!(merged.segments[0].speaker.as_deref(), Some("Max Kruger"));
        assert_eq!(merged.segments[1].speaker.as_deref(), Some("Anna Weiss"));
    }

    #[test]
    fn two_people_genuinely_talking_at_once_both_survive() {
        let merged = merge(&[
            track(
                "Max Kruger",
                vec![(1_000, 3_000, "I think we should wait.")],
                vec![0.40],
            ),
            track(
                "Anna Weiss",
                vec![(1_100, 3_100, "No, ship it now.")],
                vec![0.41],
            ),
        ]);

        assert_eq!(
            merged.segments.len(),
            2,
            "an interruption is not a duplicate"
        );
    }

    #[test]
    fn the_same_sentence_heard_differently_still_counts_as_one_utterance() {
        assert!(wording_overlap("We ship on Friday", "We ship Friday") >= SAME_WORDING);
        assert!(wording_overlap("Morning, Max.", "morning max") >= SAME_WORDING);
    }

    #[test]
    fn two_different_sentences_are_not_mistaken_for_one() {
        assert!(wording_overlap("I think we should wait", "No, ship it now") < SAME_WORDING);
    }

    #[test]
    fn comparing_against_nothing_is_not_a_match() {
        assert_eq!(wording_overlap("", "hello"), 0.0);
        assert_eq!(wording_overlap("...", "hello"), 0.0);
    }

    #[test]
    fn segments_come_out_in_chronological_order() {
        let merged = merge(&[
            track(
                "Max Kruger",
                vec![(6_000, 7_000, "Third."), (0, 1_000, "First.")],
                vec![0.4, 0.4],
            ),
            track("Anna Weiss", vec![(3_000, 4_000, "Second.")], vec![0.4]),
        ]);

        let starts: Vec<u32> = merged.segments.iter().map(|s| s.start_ms).collect();
        assert_eq!(starts, vec![0, 3_000, 6_000]);
    }

    #[test]
    fn silence_on_one_track_does_not_produce_empty_lines() {
        let merged = merge(&[
            track(
                "Max Kruger",
                vec![(0, 2_000, "Morning."), (5_000, 6_000, "   ")],
                vec![0.4, 0.0],
            ),
            track("Anna Weiss", vec![(3_000, 4_000, "Hello.")], vec![0.4]),
        ]);

        assert_eq!(merged.segments.len(), 2);
        assert!(merged.segments.iter().all(|s| !s.text.trim().is_empty()));
    }

    #[test]
    fn a_track_with_no_level_readings_is_still_merged() {
        let merged = merge(&[
            track("Max Kruger", vec![(0, 2_000, "Morning.")], vec![]),
            track("Anna Weiss", vec![(5_000, 6_000, "Hello.")], vec![]),
        ]);
        assert_eq!(merged.segments.len(), 2);
    }

    #[test]
    fn merging_nothing_produces_an_empty_transcript_rather_than_panicking() {
        let merged = merge(&[]);
        assert!(merged.segments.is_empty());
        assert_eq!(merged.language, "unknown");
    }

    #[test]
    fn the_speaker_list_is_in_the_order_people_first_spoke() {
        let merged = merge(&[
            track("Anna Weiss", vec![(3_000, 4_000, "Second.")], vec![0.4]),
            track("Max Kruger", vec![(0, 1_000, "First.")], vec![0.4]),
        ]);

        assert_eq!(speakers(&merged), vec!["Max Kruger", "Anna Weiss"]);
    }

    #[test]
    fn a_transcript_with_no_attribution_lists_no_speakers() {
        assert!(speakers(&transcript(vec![Segment::new(0, 1_000, "Hello.")])).is_empty());
    }

    #[test]
    fn a_long_recording_does_not_compare_every_segment_against_every_other() {
        let long = |speaker: &str, offset: u32| Track {
            speaker: speaker.to_owned(),
            transcript: transcript(
                (0..4_000)
                    .map(|i| Segment::new(i * 3_000 + offset, i * 3_000 + 2_500 + offset, "line"))
                    .collect(),
            ),
            levels: vec![0.4; 4_000],
        };

        let started = std::time::Instant::now();
        let merged = merge(&[long("Max Kruger", 0), long("Anna Weiss", 60_000)]);

        assert!(!merged.segments.is_empty());
        assert!(
            started.elapsed().as_secs() < 5,
            "merging took {:?}, which suggests a quadratic scan",
            started.elapsed()
        );
    }
}
