use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::prompt::Prompt;
use crate::summary::Summary;
use crate::transcript::{format_offset, Transcript};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Role {
    User,

    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Turn {
    pub role: Role,
    pub content: String,
}

impl Turn {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

pub const CONTEXT_CHARS: usize = 20_000;

const RULES: &str = "\
You answer questions about one recorded conversation, using its transcript.

The transcript is the only thing you know. If it does not contain the answer, \
say so plainly — \"the recording does not say\" — and stop there. Do not fill \
the gap from general knowledge, and do not reason about what was probably \
meant unless you say that is what you are doing. Being wrong here is worse \
than being unhelpful, because the person asking cannot tell the difference \
without listening to the whole recording again.

Quote the transcript when it settles the question. Timestamps are in the \
margin; cite them when they help someone find the moment.

Answer in the language of the question. Keep it short unless asked for detail, \
and use Markdown only where it earns its place.";

pub const TRUNCATION_NOTE: &str = "\n\n[The transcript continues beyond this point and was cut to \
fit. If the answer depends on what came later, say so.]";

#[derive(Debug, Clone)]
pub struct Context<'a> {
    pub transcript: &'a Transcript,

    pub summary: Option<&'a Summary>,

    pub title: &'a str,

    pub timestamps: bool,
}

pub fn prompt(context: &Context<'_>, history: &[Turn], question: &str) -> Prompt {
    let mut user = format!("Recording: {}\n", context.title.trim());

    if let Some(summary) = context.summary {
        user.push_str("\nA summary of it was written earlier:\n\n");
        user.push_str(summary.body_md.trim());
        user.push('\n');
    }

    user.push_str("\nTranscript:\n\n");
    user.push_str(&render(context));

    if !history.is_empty() {
        user.push_str("\n\nOur conversation so far:\n\n");
        for turn in history {
            let who = match turn.role {
                Role::User => "Question",
                Role::Assistant => "Answer",
            };
            user.push_str(&format!("{who}: {}\n", turn.content.trim()));
        }
    }

    user.push_str(&format!("\n\nQuestion: {}", question.trim()));

    Prompt {
        system: RULES.to_owned(),
        user,
    }
}

fn render(context: &Context<'_>) -> String {
    let mut out = String::new();
    let mut truncated = false;

    for segment in &context.transcript.segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }

        let line = match (&segment.speaker, context.timestamps) {
            (Some(speaker), true) if !speaker.is_empty() => {
                format!("[{}] {speaker}: {text}\n", format_offset(segment.start_ms))
            }
            (Some(speaker), false) if !speaker.is_empty() => format!("{speaker}: {text}\n"),
            (_, true) => format!("[{}] {text}\n", format_offset(segment.start_ms)),
            (_, false) => format!("{text}\n"),
        };

        if out.len() + line.len() > CONTEXT_CHARS {
            truncated = true;
            break;
        }
        out.push_str(&line);
    }

    if truncated {
        out.push_str(TRUNCATION_NOTE);
    }
    out.trim_end().to_owned()
}

pub fn fits(transcript: &Transcript, timestamps: bool) -> bool {
    let context = Context {
        transcript,
        summary: None,
        title: "",
        timestamps,
    };
    !render(&context).contains(TRUNCATION_NOTE.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::TokenUsage;
    use crate::transcript::Segment;

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments,
        }
    }

    fn context<'a>(transcript: &'a Transcript, summary: Option<&'a Summary>) -> Context<'a> {
        Context {
            transcript,
            summary,
            title: "Planning call",
            timestamps: true,
        }
    }

    fn summary_of(body: &str) -> Summary {
        Summary {
            provider: "ollama".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            template_id: "meeting".to_owned(),
            body_md: body.to_owned(),
            action_items: Vec::new(),
            usage: TokenUsage::default(),
            elapsed_ms: 0,
            redacted: 0,
        }
    }

    #[test]
    fn the_transcript_and_the_question_both_reach_the_model() {
        let t = transcript(vec![Segment::new(0, 2_000, "We ship on Friday.")]);
        let built = prompt(&context(&t, None), &[], "When do we ship?");

        assert!(built.user.contains("We ship on Friday."));
        assert!(built.user.contains("Question: When do we ship?"));
        assert!(built.user.contains("Planning call"));
    }

    #[test]
    fn the_model_is_told_the_transcript_is_all_it_knows() {
        let t = transcript(vec![Segment::new(0, 2_000, "Hello.")]);
        let built = prompt(&context(&t, None), &[], "What is the capital of France?");

        assert!(built.system.contains("only thing you know"));
        assert!(built.system.contains("the recording does not say"));
    }

    #[test]
    fn an_existing_summary_is_offered_as_context() {
        let t = transcript(vec![Segment::new(0, 2_000, "We ship on Friday.")]);
        let summary = summary_of("## Decisions\n\n- Ship on Friday");
        let built = prompt(&context(&t, Some(&summary)), &[], "When?");

        assert!(built.user.contains("## Decisions"));
    }

    #[test]
    fn a_recording_with_no_summary_still_produces_a_valid_question() {
        let t = transcript(vec![Segment::new(0, 2_000, "We ship on Friday.")]);
        let built = prompt(&context(&t, None), &[], "When?");

        assert!(!built.user.contains("summary of it was written"));
        assert!(built.user.contains("Question: When?"));
    }

    #[test]
    fn the_conversation_so_far_is_carried_into_the_next_question() {
        let t = transcript(vec![Segment::new(0, 2_000, "Anna will send it.")]);
        let history = [
            Turn::user("Who is sending the contract?"),
            Turn::assistant("Anna."),
        ];
        let built = prompt(&context(&t, None), &history, "When did she say that?");

        let asked = built
            .user
            .find("Who is sending")
            .expect("the earlier question");
        let answered = built
            .user
            .find("Answer: Anna.")
            .expect("the earlier answer");
        let now = built
            .user
            .find("Question: When did she")
            .expect("the new question");

        assert!(
            asked < answered && answered < now,
            "history must stay in order"
        );
    }

    #[test]
    fn speaker_names_are_carried_into_the_question() {
        let t = transcript(vec![
            Segment::new(0, 2_000, "We ship on Friday.").by("Anna Weiss")
        ]);
        let built = prompt(&context(&t, None), &[], "Who decided?");

        assert!(
            built.user.contains("Anna Weiss: We ship on Friday."),
            "{}",
            built.user
        );
    }

    #[test]
    fn timestamps_can_be_left_out() {
        let t = transcript(vec![Segment::new(72_000, 74_000, "We ship on Friday.")]);
        let mut ctx = context(&t, None);
        ctx.timestamps = false;

        let built = prompt(&ctx, &[], "When?");
        assert!(!built.user.contains("1:12"));
        assert!(built.user.contains("We ship on Friday."));
    }

    #[test]
    fn a_transcript_too_long_to_send_is_cut_and_says_so() {
        let t = transcript(
            (0..4_000)
                .map(|i| {
                    Segment::new(
                        i * 3_000,
                        i * 3_000 + 2_500,
                        "A sentence of some length here.",
                    )
                })
                .collect(),
        );
        let built = prompt(&context(&t, None), &[], "What was decided?");

        assert!(
            built.user.contains("was cut to fit"),
            "the model must be told"
        );
        assert!(built.user.len() < CONTEXT_CHARS + 2_000);
        assert!(!fits(&t, true));
    }

    #[test]
    fn an_ordinary_recording_is_sent_whole() {
        let t = transcript(vec![Segment::new(0, 2_000, "We ship on Friday.")]);

        assert!(fits(&t, true));
        assert!(!prompt(&context(&t, None), &[], "When?")
            .user
            .contains("was cut"));
    }

    #[test]
    fn silent_segments_do_not_reach_the_model() {
        let t = transcript(vec![
            Segment::new(0, 1_000, "Hello."),
            Segment::new(1_000, 4_000, "   "),
            Segment::new(4_000, 5_000, "Bye."),
        ]);
        let built = prompt(&context(&t, None), &[], "What happened?");

        assert!(built.user.contains("Hello."));
        assert!(built.user.contains("Bye."));
    }

    #[test]
    fn a_question_is_trimmed_rather_than_arriving_with_stray_whitespace() {
        let t = transcript(vec![Segment::new(0, 1_000, "Hello.")]);
        let built = prompt(&context(&t, None), &[], "  When?  \n");

        assert!(built.user.ends_with("Question: When?"));
    }
}
