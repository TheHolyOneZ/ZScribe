use crate::prompt::Prompt;
use crate::transcript::format_offset;
use crate::Segment;

pub const PASSAGE_CHARS: usize = 900;

pub const OVERLAP_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

pub fn passages(segments: &[Segment]) -> Vec<Passage> {
    let mut out: Vec<Passage> = Vec::new();
    let mut current: Vec<&Segment> = Vec::new();
    let mut length = 0usize;

    for segment in segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }

        current.push(segment);
        length += text.len() + 1;

        if length < PASSAGE_CHARS {
            continue;
        }

        out.push(passage(&current));

        let mut carried: Vec<&Segment> = Vec::new();
        let mut carried_len = 0usize;
        for segment in current.iter().rev().skip(1) {
            let len = segment.text.trim().len() + 1;
            if carried_len + len > OVERLAP_CHARS {
                break;
            }
            carried_len += len;
            carried.push(segment);
        }
        carried.reverse();

        current = carried;
        length = carried_len;
    }

    if !current.is_empty() {
        let last = passage(&current);

        if out
            .last()
            .map(|previous| previous.text != last.text)
            .unwrap_or(true)
        {
            out.push(last);
        }
    }

    out
}

fn passage(segments: &[&Segment]) -> Passage {
    Passage {
        start_ms: segments.first().map_or(0, |segment| segment.start_ms),
        end_ms: segments.last().map_or(0, |segment| segment.end_ms),
        text: segments
            .iter()
            .map(|segment| segment.text.trim())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

const RULES: &str = "\
You answer questions from someone's own recordings, using passages retrieved \
from their transcripts.

The passages are the only thing you know. Each is labelled with the recording \
it came from and the time it was said. They come from *different* \
conversations, possibly months apart, and some of them will have nothing to do \
with the question — they were retrieved by similarity, not by understanding. \
Ignore those rather than fitting them in.

Never merge separate conversations into one account. If two recordings say \
different things, say that they do, and say which said what.

Attribute every claim to its recording by name — \"In *Weekly sync* (12:04) …\" \
— so the person can go and listen. If the passages do not contain the answer, \
say \"none of your recordings say\" and stop. Do not fill the gap from general \
knowledge, and do not guess at what was probably meant.

Answer in your own words, and quote only the phrase that settles it — the \
passages are shown to the person beside your answer, so repeating them back is \
wasted space.

Answer in the language of the question. Keep it short unless asked for detail.";

#[derive(Debug, Clone)]
pub struct Retrieved<'a> {
    pub title: &'a str,

    pub when: &'a str,

    pub start_ms: u32,
    pub text: &'a str,
}

pub fn prompt(question: &str, retrieved: &[Retrieved<'_>]) -> Prompt {
    let mut user = String::from("Passages from your recordings:\n");

    for (index, passage) in retrieved.iter().enumerate() {
        user.push_str(&format!(
            "\n[{}] {} — {} at {}\n{}\n",
            index + 1,
            passage.title.trim(),
            passage.when.trim(),
            format_offset(passage.start_ms),
            passage.text.trim(),
        ));
    }

    if retrieved.is_empty() {
        user.push_str("\n(nothing was retrieved for this question)\n");
    }

    user.push_str(&format!("\n\nQuestion: {}", question.trim()));

    Prompt {
        system: RULES.to_owned(),
        user,
    }
}

pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut left = 0.0f32;
    let mut right = 0.0f32;

    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        left += x * x;
        right += y * y;
    }

    let magnitude = (left.sqrt()) * (right.sqrt());
    if magnitude == 0.0 {
        return 0.0;
    }
    dot / magnitude
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start_ms: u32, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms + 3_000,
            text: text.to_owned(),
            speaker: None,
        }
    }

    fn long_transcript() -> Vec<Segment> {
        (0..40)
            .map(|i| {
                segment(
                    i * 4_000,
                    &format!("This is sentence number {i}, long enough to fill a passage quickly."),
                )
            })
            .collect()
    }

    #[test]
    fn a_short_transcript_is_one_passage() {
        let segments = vec![
            segment(0, "We agreed the budget."),
            segment(4_000, "That was all."),
        ];
        let passages = passages(&segments);

        assert_eq!(passages.len(), 1);
        assert_eq!(passages[0].text, "We agreed the budget. That was all.");
        assert_eq!(passages[0].start_ms, 0);
        assert_eq!(passages[0].end_ms, 7_000);
    }

    #[test]
    fn a_long_transcript_is_cut_into_several() {
        let passages = passages(&long_transcript());
        assert!(passages.len() > 2, "got {} passages", passages.len());

        for passage in &passages {
            assert!(
                passage.text.len() < PASSAGE_CHARS * 2,
                "a passage ran away: {} chars",
                passage.text.len()
            );
        }
    }

    #[test]
    fn passages_overlap_so_a_sentence_on_a_boundary_is_not_lost() {
        let passages = passages(&long_transcript());
        let first = &passages[0];
        let second = &passages[1];

        assert!(
            second.start_ms < first.end_ms,
            "no overlap: {} then {}",
            first.end_ms,
            second.start_ms
        );
    }

    #[test]
    fn passages_move_forward_however_long_the_segments_are() {
        let huge = "word ".repeat(500);
        let segments = vec![
            segment(0, &huge),
            segment(9_000, &huge),
            segment(18_000, &huge),
        ];

        let passages = passages(&segments);
        assert_eq!(passages.len(), 3, "one passage each, and no infinite loop");
        assert!(passages[0].start_ms < passages[1].start_ms);
        assert!(passages[1].start_ms < passages[2].start_ms);
    }

    #[test]
    fn timings_come_from_the_segments_a_passage_was_built_from() {
        let passages = passages(&long_transcript());
        let last = passages.last().expect("at least one");

        assert_eq!(passages[0].start_ms, 0);
        assert_eq!(last.end_ms, 39 * 4_000 + 3_000);
    }

    #[test]
    fn an_empty_or_silent_transcript_yields_nothing_to_index() {
        assert!(passages(&[]).is_empty());
        assert!(passages(&[segment(0, "   "), segment(1_000, "")]).is_empty());
    }

    #[test]
    fn the_prompt_labels_every_passage_with_where_it_came_from() {
        let prompt = prompt(
            "What did we decide about the budget?",
            &[
                Retrieved {
                    title: "Weekly sync",
                    when: "12 August",
                    start_ms: 724_000,
                    text: "We agreed the budget lands in March.",
                },
                Retrieved {
                    title: "Board call",
                    when: "3 July",
                    start_ms: 0,
                    text: "Nothing about money here.",
                },
            ],
        );

        assert!(prompt.user.contains("Weekly sync"), "{}", prompt.user);
        assert!(prompt.user.contains("12 August"), "{}", prompt.user);
        assert!(
            prompt.user.contains("12:04"),
            "the offset is human-readable"
        );
        assert!(prompt.user.contains("What did we decide about the budget?"));

        let sync = prompt.user.find("Weekly sync").expect("first");
        let board = prompt.user.find("Board call").expect("second");
        assert!(sync < board);

        assert!(prompt.system.contains("none of your recordings say"));
        assert!(prompt.system.contains("Never merge separate conversations"));
    }

    #[test]
    fn a_question_with_nothing_retrieved_still_asks_honestly() {
        let prompt = prompt("Anything about the budget?", &[]);
        assert!(
            prompt.user.contains("nothing was retrieved"),
            "{}",
            prompt.user
        );
        assert!(prompt.user.contains("Anything about the budget?"));
    }

    #[test]
    fn similarity_is_one_for_the_same_direction_and_zero_for_a_right_angle() {
        assert!((similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);

        assert!((similarity(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert!((similarity(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn mismatched_or_empty_vectors_score_zero_rather_than_panicking() {
        assert_eq!(similarity(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0);
        assert_eq!(similarity(&[], &[]), 0.0);
        assert_eq!(similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }
}
