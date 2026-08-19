use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Template {
    pub id: String,

    pub name: String,

    pub description: String,

    pub instructions: String,
}

pub const DEFAULT_TEMPLATE_ID: &str = "self-note";

pub const SHARED_RULES: &str = "\
Work only from the transcript. Never add a fact, name, number or date that is \
not in it. If a section has nothing to report, write \"Nothing recorded\" under \
that heading rather than inventing content or dropping the heading.

The transcript comes from automatic speech recognition and will contain \
mishearings. Where a word is clearly garbled, prefer the reading the \
surrounding sentence supports; where you cannot tell, quote it as heard.

Reply with Markdown only — no preamble, no closing offer of further help, and \
do not wrap the whole answer in a code fence.";

fn template(id: &str, name: &str, description: &str, instructions: &str) -> Template {
    Template {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        instructions: instructions.to_owned(),
    }
}

pub fn builtin_templates() -> Vec<Template> {
    vec![
        template(
            "overview",
            "Overview",
            "One summary that fits anything — condensed, not the transcript again.",
            "\
This can be anything: a meeting, a video, a phone call, or a note to yourself. \
Fit the summary to what it actually is, and drop any heading below that has \
nothing to go under it.

Condense. The point is to give the reader the recording in a fraction of the \
time, so write in your own words and never reproduce the transcript line by \
line or copy stretches of dialogue verbatim. If one moment is worth an exact \
quote, keep that single line and nothing around it.

Produce:

## Overview
Two to four sentences: what this recording is, who is in it as far as the \
transcript says, and what it was about.

## Key points
The substance, as bullets grouped by topic rather than in the order things were \
said, with repetition merged. One point per bullet, each a decision, claim, \
result or reaction stated in your own words — never a line of the transcript \
quoted back.

## Details
Specific names, numbers, dates, or a notable moment worth keeping that the key \
points did not already carry. Omit this heading entirely if there is nothing to \
add.

## Action items
Anything someone said they would do, one per line, with who and by when where \
the recording says so. Omit this heading entirely if there were none.",
        ),
        template(
            "self-note",
            "Self-note",
            "Thinking out loud, sorted into themes.",
            "\
This is someone thinking out loud on their own, not a meeting. Do not impose \
meeting structure on it.

Produce:

## Summary
Two or three sentences on what this recording is about.

## Thoughts
The ideas, grouped by theme rather than in the order they were spoken. \
Repetition and false starts are normal here — merge them.

## Open questions
Anything the speaker was undecided about or explicitly left open.

## Next steps
Concrete things the speaker said they would do. Omit this heading entirely if \
they named none.",
        ),
        template(
            "meeting",
            "Meeting",
            "Decisions, open points, and tasks with an owner.",
            "\
Produce:

## Summary
Three or four sentences on what the meeting was about and where it landed.

## Decisions
What was actually decided. Each entry is one decision, stated plainly. A topic \
that was discussed without resolution belongs under Open points, not here.

## Open points
Questions raised and left unanswered, and disagreements that were not settled.

## Action items
A Markdown table with the columns Task, Owner, Due. Use the name as spoken. \
Where an owner or a date was never stated, write \"unassigned\" or \"none\" \
rather than guessing.",
        ),
        template(
            "sales-call",
            "Sales call",
            "Needs, objections, and the next step you agreed.",
            "\
This is a conversation with a prospect or customer. Keep the customer's own \
wording where it matters — the phrasing of an objection carries information \
that a paraphrase loses.

Produce:

## Summary
Who this was with, as far as the transcript says, and where it stands.

## Needs
What the customer said they need, and any deadline or budget they mentioned.

## Objections
Every concern, hesitation or push-back, with the customer's own words in \
quotes where they were specific.

## Agreed next steps
What was actually agreed, with who does what and by when. If the call ended \
without an agreed next step, say so plainly — that is the most useful thing \
this section can tell you.",
        ),
        template(
            "interview",
            "Interview",
            "Key statements, quotable lines, and themes.",
            "\
Produce:

## Summary
What was covered, in three or four sentences.

## Themes
The subjects discussed, each with a short paragraph on what was said.

## Key statements
The substantive claims and positions the interviewee took, one per bullet.

## Quotes
Verbatim lines worth reproducing, each with its timestamp. Copy them exactly \
as transcribed — do not tidy the grammar of a quote.",
        ),
        template(
            "raw",
            "Plain summary",
            "No structure imposed — just a faithful summary.",
            "\
Produce a faithful prose summary of the recording, in as many paragraphs as \
the content needs. Do not impose headings, sections or bullet lists on \
material that does not have that shape. Follow the order of the conversation.

After the summary, if and only if the speakers committed to specific actions, \
add an \"## Action items\" heading and list them.",
        ),
    ]
}

pub fn derive_custom(from: &Template, name: &str) -> Template {
    Template {
        id: format!("custom-{}", uuid_like(name)),
        name: name.to_owned(),
        description: String::new(),
        instructions: from.instructions.clone(),
    }
}

fn uuid_like(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let slug = slug.trim_matches('-').replace("--", "-");
    let slug = if slug.is_empty() {
        "template".to_owned()
    } else {
        slug
    };

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    format!("{}-{stamp:x}", slug.chars().take(40).collect::<String>())
}

pub fn builtin_template(id: &str) -> Option<Template> {
    builtin_templates().into_iter().find(|t| t.id == id)
}

pub fn resolve(id: &str, custom: &[Template]) -> Template {
    if let Some(found) = custom.iter().find(|t| t.id == id) {
        return found.clone();
    }
    builtin_template(id)
        .or_else(|| builtin_template(DEFAULT_TEMPLATE_ID))
        .expect("the default template is a built-in")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_template_exists() {
        assert!(builtin_template(DEFAULT_TEMPLATE_ID).is_some());
    }

    #[test]
    fn the_default_is_the_self_note_case_the_user_will_reach_for_most() {
        assert_eq!(DEFAULT_TEMPLATE_ID, "self-note");
    }

    #[test]
    fn builtin_ids_are_unique() {
        let mut ids: Vec<String> = builtin_templates().into_iter().map(|t| t.id).collect();
        ids.sort();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn every_builtin_is_completely_filled_in() {
        for t in builtin_templates() {
            assert!(!t.name.is_empty(), "{}", t.id);
            assert!(!t.description.is_empty(), "{}", t.id);
            assert!(!t.instructions.is_empty(), "{}", t.id);
        }
    }

    #[test]
    fn every_builtin_asks_for_markdown_headings() {
        for t in builtin_templates() {
            assert!(t.instructions.contains("##"), "{} has no headings", t.id);
        }
    }

    #[test]
    fn shared_rules_forbid_inventing_content() {
        assert!(SHARED_RULES.contains("Never add a fact"));
    }

    #[test]
    fn a_derived_template_starts_from_a_working_example() {
        let meeting = builtin_template("meeting").expect("built in");
        let mine = derive_custom(&meeting, "Board meeting");

        assert_eq!(mine.name, "Board meeting");
        assert_eq!(
            mine.instructions, meeting.instructions,
            "copy the shape to learn from"
        );
    }

    #[test]
    fn a_derived_template_can_never_shadow_a_builtin() {
        let meeting = builtin_template("meeting").expect("built in");

        for name in ["Meeting", "meeting", "self note", "Raw"] {
            let mine = derive_custom(&meeting, name);
            assert!(
                builtin_template(&mine.id).is_none(),
                "{name:?} produced the built-in id {}",
                mine.id
            );
        }
    }

    #[test]
    fn two_templates_with_the_same_name_get_different_ids() {
        let meeting = builtin_template("meeting").expect("built in");
        let first = derive_custom(&meeting, "Board meeting");
        let second = derive_custom(&meeting, "Board meeting");

        assert_ne!(first.id, second.id);
    }

    #[test]
    fn an_awkward_name_still_produces_a_usable_id() {
        let meeting = builtin_template("meeting").expect("built in");

        for name in ["", "   ", "///", "Kundengespräch — Q3 / 2026"] {
            let mine = derive_custom(&meeting, name);
            assert!(mine.id.starts_with("custom-"), "{name:?} gave {}", mine.id);
            assert!(!mine.id.contains('/'), "{name:?} gave {}", mine.id);
        }
    }

    #[test]
    fn a_custom_template_wins_over_a_builtin_with_the_same_id() {
        let custom = vec![template("meeting", "Mine", "d", "i")];
        assert_eq!(resolve("meeting", &custom).name, "Mine");
    }

    #[test]
    fn an_unknown_id_falls_back_to_the_default_rather_than_failing() {
        assert_eq!(resolve("deleted-long-ago", &[]).id, DEFAULT_TEMPLATE_ID);
    }
}
