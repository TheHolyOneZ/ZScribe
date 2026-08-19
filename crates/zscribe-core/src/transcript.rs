use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Segment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,

    #[serde(default)]
    pub speaker: Option<String>,
}

impl Segment {
    pub fn new(start_ms: u32, end_ms: u32, text: impl Into<String>) -> Self {
        Self {
            start_ms,
            end_ms,
            text: text.into(),
            speaker: None,
        }
    }

    pub fn by(mut self, speaker: impl Into<String>) -> Self {
        self.speaker = Some(speaker.into());
        self
    }

    pub fn duration_ms(&self) -> u32 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    pub fn overlaps(&self, other: &Segment) -> bool {
        let start = self.start_ms.max(other.start_ms);
        let end = self.end_ms.min(other.end_ms);
        if start >= end {
            return false;
        }

        let shared = end - start;
        let shorter = self.duration_ms().min(other.duration_ms()).max(1);

        shared * 2 >= shorter
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Transcript {
    pub language: String,

    pub model: String,

    pub segments: Vec<Segment>,
}

impl Transcript {
    pub fn text(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            let line = segment.text.trim();
            if line.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
        out
    }

    pub fn duration_ms(&self) -> u32 {
        self.segments.last().map_or(0, |segment| segment.end_ms)
    }

    pub fn is_empty(&self) -> bool {
        self.segments
            .iter()
            .all(|segment| segment.text.trim().is_empty())
    }

    pub fn has_speech(&self) -> bool {
        self.letters() >= MIN_SPEECH_LETTERS
    }

    fn letters(&self) -> usize {
        self.segments
            .iter()
            .flat_map(|segment| segment.text.chars())
            .filter(|c| c.is_alphanumeric())
            .count()
    }

    pub fn approx_tokens(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| segment.text.len().div_ceil(4))
            .sum()
    }
}

const MIN_SPEECH_LETTERS: usize = 20;

pub fn format_offset(ms: u32) -> String {
    let total = ms / 1000;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start_ms: u32, end_ms: u32, text: &str) -> Segment {
        Segment::new(start_ms, end_ms, text)
    }

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments,
        }
    }

    #[test]
    fn text_joins_segments_one_per_line() {
        let t = transcript(vec![
            segment(0, 1000, "Hello."),
            segment(1000, 2000, "How are you?"),
        ]);
        assert_eq!(t.text(), "Hello.\nHow are you?");
    }

    #[test]
    fn whisper_pads_segments_with_spaces_and_we_trim_them() {
        let t = transcript(vec![segment(0, 1000, "  Hello.  ")]);
        assert_eq!(t.text(), "Hello.");
    }

    #[test]
    fn silent_segments_do_not_become_blank_lines() {
        let t = transcript(vec![
            segment(0, 1000, "Hello."),
            segment(1000, 4000, "   "),
            segment(4000, 5000, "Still there?"),
        ]);
        assert_eq!(t.text(), "Hello.\nStill there?");
    }

    #[test]
    fn a_recording_of_pure_silence_reports_itself_empty() {
        assert!(transcript(vec![segment(0, 9000, "  ")]).is_empty());
        assert!(transcript(Vec::new()).is_empty());
    }

    #[test]
    fn a_transcript_of_pure_punctuation_is_not_treated_as_speech() {
        assert!(!transcript(vec![segment(0, 10_000, ".")]).has_speech());
        assert!(!transcript(vec![segment(0, 10_000, "...")]).has_speech());
        assert!(!transcript(vec![segment(0, 10_000, ". . . !")]).has_speech());
    }

    #[test]
    fn whispers_usual_hallucinations_for_silence_are_not_treated_as_speech() {
        for noise in ["you", "Thank you.", "Thanks for watching!", "[BLANK_AUDIO]"] {
            assert!(
                !transcript(vec![segment(0, 10_000, noise)]).has_speech(),
                "{noise:?} is silence, not a conversation"
            );
        }
    }

    #[test]
    fn a_short_but_real_sentence_does_count_as_speech() {
        assert!(transcript(vec![segment(0, 4_000, "Remember to call Anna back.")]).has_speech());
    }

    #[test]
    fn speech_spread_across_several_quiet_segments_still_counts() {
        let t = transcript(vec![
            segment(0, 1_000, "Yes."),
            segment(1_000, 2_000, "Agreed."),
            segment(2_000, 3_000, "Friday works."),
        ]);
        assert!(t.has_speech());
    }

    #[test]
    fn an_empty_transcript_has_no_speech() {
        assert!(!transcript(Vec::new()).has_speech());
        assert!(!transcript(vec![segment(0, 9_000, "   ")]).has_speech());
    }

    #[test]
    fn duration_is_the_end_of_the_last_segment() {
        let t = transcript(vec![segment(0, 1000, "a"), segment(1000, 7250, "b")]);
        assert_eq!(t.duration_ms(), 7250);
        assert_eq!(transcript(Vec::new()).duration_ms(), 0);
    }

    #[test]
    fn offsets_drop_the_hour_field_only_when_there_is_no_hour() {
        assert_eq!(format_offset(0), "0:00");
        assert_eq!(format_offset(9_000), "0:09");
        assert_eq!(format_offset(72_000), "1:12");
        assert_eq!(format_offset(3_600_000), "1:00:00");
        assert_eq!(format_offset(3_671_000), "1:01:11");
    }

    #[test]
    fn token_estimate_rounds_up_so_a_chunk_budget_is_never_overshot() {
        let t = transcript(vec![segment(0, 1000, "abcde")]);
        assert_eq!(t.approx_tokens(), 2);
    }
}
