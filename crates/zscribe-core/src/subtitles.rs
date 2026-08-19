use crate::transcript::Transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subtitles {
    Srt,

    Vtt,
}

impl Subtitles {
    pub const fn extension(self) -> &'static str {
        match self {
            Subtitles::Srt => "srt",
            Subtitles::Vtt => "vtt",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Subtitles::Srt => "SubRip",
            Subtitles::Vtt => "WebVTT",
        }
    }
}

pub fn write(transcript: &Transcript, format: Subtitles) -> String {
    let mut out = String::new();

    if format == Subtitles::Vtt {
        out.push_str("WEBVTT\n\n");
    }

    let mut number = 1;

    for segment in &transcript.segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }

        let start = segment.start_ms;
        let end = segment.end_ms.max(start + MINIMUM_CUE_MS);

        if format == Subtitles::Srt {
            out.push_str(&format!("{number}\n"));
        }

        out.push_str(&format!(
            "{} --> {}\n",
            stamp(start, format),
            stamp(end, format)
        ));

        match (&segment.speaker, format) {
            (Some(speaker), Subtitles::Vtt) => {
                out.push_str(&format!("<v {}>{}\n", escape(speaker), escape(text)));
            }
            (Some(speaker), Subtitles::Srt) => {
                out.push_str(&format!("{speaker}: {text}\n"));
            }
            (None, Subtitles::Vtt) => out.push_str(&format!("{}\n", escape(text))),
            (None, Subtitles::Srt) => out.push_str(&format!("{text}\n")),
        }

        out.push('\n');
        number += 1;
    }

    out
}

const MINIMUM_CUE_MS: u32 = 500;

fn stamp(ms: u32, format: Subtitles) -> String {
    let (hours, minutes, seconds, millis) = (
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1_000,
        ms % 1_000,
    );

    let separator = if format == Subtitles::Srt { ',' } else { '.' };
    format!("{hours:02}:{minutes:02}:{seconds:02}{separator}{millis:03}")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Segment;

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments,
        }
    }

    fn segment(start_ms: u32, end_ms: u32, text: &str, speaker: Option<&str>) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: text.to_owned(),
            speaker: speaker.map(str::to_owned),
        }
    }

    #[test]
    fn srt_is_numbered_cues_with_comma_timestamps() {
        let file = write(
            &transcript(vec![
                segment(0, 2_500, "Right, let's start.", None),
                segment(3_000, 5_750, "The installer is done.", None),
            ]),
            Subtitles::Srt,
        );

        assert_eq!(
            file,
            "1\n00:00:00,000 --> 00:00:02,500\nRight, let's start.\n\n\
             2\n00:00:03,000 --> 00:00:05,750\nThe installer is done.\n\n"
        );
    }

    #[test]
    fn vtt_has_a_header_and_full_stops() {
        let file = write(
            &transcript(vec![segment(61_000, 63_200, "One minute in.", None)]),
            Subtitles::Vtt,
        );

        assert!(file.starts_with("WEBVTT\n\n"), "{file}");
        assert!(file.contains("00:01:01.000 --> 00:01:03.200"), "{file}");

        assert!(!file.contains("\n1\n"), "{file}");
    }

    #[test]
    fn hours_are_not_lost_on_a_long_recording() {
        let file = write(
            &transcript(vec![segment(7_384_500, 7_386_000, "Still going.", None)]),
            Subtitles::Srt,
        );
        assert!(file.contains("02:03:04,500 --> 02:03:06,000"), "{file}");
    }

    #[test]
    fn a_speaker_is_named_the_way_each_format_can_carry_it() {
        let segments = vec![segment(
            0,
            2_000,
            "The page is not done.",
            Some("Speaker 2"),
        )];

        let srt = write(&transcript(segments.clone()), Subtitles::Srt);
        assert!(srt.contains("Speaker 2: The page is not done."), "{srt}");

        let vtt = write(&transcript(segments), Subtitles::Vtt);
        assert!(vtt.contains("<v Speaker 2>The page is not done."), "{vtt}");
    }

    #[test]
    fn markup_in_speech_cannot_break_a_vtt_cue() {
        let file = write(
            &transcript(vec![segment(0, 1_000, "a < b & c", None)]),
            Subtitles::Vtt,
        );
        assert!(file.contains("a &lt; b &amp; c"), "{file}");

        let srt = write(
            &transcript(vec![segment(0, 1_000, "a < b & c", None)]),
            Subtitles::Srt,
        );
        assert!(srt.contains("a < b & c"), "{srt}");
    }

    #[test]
    fn a_cue_that_ends_when_it_starts_is_given_a_length() {
        let file = write(
            &transcript(vec![segment(4_000, 4_000, "Yes.", None)]),
            Subtitles::Srt,
        );
        assert!(file.contains("00:00:04,000 --> 00:00:04,500"), "{file}");
    }

    #[test]
    fn blank_lines_are_dropped_and_the_numbering_stays_contiguous() {
        let file = write(
            &transcript(vec![
                segment(0, 1_000, "First.", None),
                segment(1_000, 2_000, "   ", None),
                segment(2_000, 3_000, "Second.", None),
            ]),
            Subtitles::Srt,
        );

        assert!(file.contains("1\n00:00:00,000"), "{file}");
        assert!(file.contains("2\n00:00:02,000"), "{file}");
        assert!(!file.contains("3\n"), "a blank cue was written: {file}");
    }

    #[test]
    fn an_empty_transcript_is_an_empty_file_rather_than_a_broken_one() {
        assert_eq!(write(&transcript(vec![]), Subtitles::Srt), "");

        assert_eq!(write(&transcript(vec![]), Subtitles::Vtt), "WEBVTT\n\n");
    }
}
