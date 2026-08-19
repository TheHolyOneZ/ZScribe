use crate::template::{Template, SHARED_RULES};
use crate::transcript::{format_offset, Segment, Transcript};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    pub user: String,
}

pub const DEFAULT_CHUNK_CHARS: usize = 24_000;

const MIN_CHUNK_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Single(Prompt),

    MapReduce {
        parts: Vec<Prompt>,

        reduce: ReduceSpec,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReduceSpec {
    system: String,
    instructions: String,
}

impl ReduceSpec {
    pub fn build(&self, partials: &[String]) -> Prompt {
        let mut user = String::from(&self.instructions);
        user.push_str("\n\n");
        for (index, partial) in partials.iter().enumerate() {
            user.push_str(&format!(
                "--- Part {} of {} ---\n",
                index + 1,
                partials.len()
            ));
            user.push_str(partial.trim());
            user.push_str("\n\n");
        }
        Prompt {
            system: self.system.clone(),
            user: user.trim_end().to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub timestamps: bool,

    pub chunk_chars: usize,

    pub language: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            timestamps: true,
            chunk_chars: DEFAULT_CHUNK_CHARS,
            language: None,
        }
    }
}

fn language_rule(language: &Option<String>) -> String {
    match language {
        None => "Answer in the language the transcript is in.".to_owned(),
        Some(name) => format!(
            "Write the entire summary in {name}, regardless of the language spoken in the \
             recording. Keep people's names as they were said; translate everything else."
        ),
    }
}

pub fn plan(template: &Template, transcript: &Transcript, options: Options) -> Plan {
    let budget = options.chunk_chars.max(MIN_CHUNK_CHARS);

    let attributed = transcript
        .segments
        .iter()
        .any(|segment| segment.speaker.as_ref().is_some_and(|s| !s.is_empty()));

    let mut system = system_prompt(template, &options.language);
    if attributed {
        system.push_str(ATTRIBUTION_NOTE);
    }
    let chunks = chunk(&transcript.segments, budget, options.timestamps);

    if chunks.len() <= 1 {
        let body = render(&transcript.segments, options.timestamps);
        return Plan::Single(Prompt {
            system,
            user: format!("Here is the transcript.\n\n{body}"),
        });
    }

    let total = chunks.len();
    let parts = chunks
        .iter()
        .enumerate()
        .map(|(index, segments)| Prompt {
            system: if attributed {
                format!("{}{ATTRIBUTION_NOTE}", map_system_prompt(&options.language))
            } else {
                map_system_prompt(&options.language)
            },
            user: format!(
                "This is part {} of {} of a longer recording. Summarise only \
                 this part, in plain Markdown, keeping every decision, task, \
                 name, number and date it contains. Do not add an introduction \
                 or a conclusion — another pass will combine the parts.\n\n{}",
                index + 1,
                total,
                render(segments, options.timestamps)
            ),
        })
        .collect();

    Plan::MapReduce {
        parts,
        reduce: ReduceSpec {
            system,
            instructions: format!(
                "A recording was too long to process at once, so it was \
                 summarised in {total} parts. Below are those partial \
                 summaries, in order. Produce the final summary of the whole \
                 recording from them, following your instructions exactly.\n\n\
                 Merge duplicates rather than repeating them, and keep the \
                 recording's own chronology. Everything in the parts came from \
                 the transcript, so you may rely on it — but still add nothing \
                 that is not there."
            ),
        },
    }
}

fn system_prompt(template: &Template, language: &Option<String>) -> String {
    format!(
        "You summarise recorded conversations and spoken notes.\n\n{}\n\n{}\n\n{}",
        template.instructions.trim(),
        SHARED_RULES,
        language_rule(language),
    )
}

const ATTRIBUTION_NOTE: &str = "\n\nEach line is prefixed with the name of the person who said \
it. Those names come from each speaker having their own microphone, so they are reliable — use \
them when saying who decided, asked or agreed to something. Do not invent names for anyone not \
listed, and do not relabel anyone.";

fn map_system_prompt(language: &Option<String>) -> String {
    format!(
        "You are condensing one part of a long recorded conversation so it can \
         be summarised as a whole afterwards. Preserve detail: every decision, \
         task, owner, name, number, date, objection and open question. Drop \
         only filler, repetition and small talk.\n\n{SHARED_RULES}\n\n{}",
        language_rule(language)
    )
}

fn chunk(segments: &[Segment], budget: usize, timestamps: bool) -> Vec<&[Segment]> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut used = 0;

    for (index, segment) in segments.iter().enumerate() {
        let cost = line_len(segment, timestamps);

        if used > 0 && used + cost > budget {
            chunks.push(&segments[start..index]);
            start = index;
            used = 0;
        }
        used += cost;
    }
    chunks.push(&segments[start..]);
    chunks
}

fn line_len(segment: &Segment, timestamps: bool) -> usize {
    let stamp = if timestamps { 11 } else { 0 };
    segment.text.trim().len() + stamp + 1
}

fn render(segments: &[Segment], timestamps: bool) -> String {
    let mut out = String::new();
    for segment in segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }
        if timestamps {
            out.push_str(&format!("[{}] ", format_offset(segment.start_ms)));
        }

        if let Some(speaker) = &segment.speaker {
            if !speaker.is_empty() {
                out.push_str(speaker);
                out.push_str(": ");
            }
        }
        out.push_str(text);
        out.push('\n');
    }
    out.trim_end().to_owned()
}

pub fn request_count(plan: &Plan) -> usize {
    match plan {
        Plan::Single(_) => 1,
        Plan::MapReduce { parts, .. } => parts.len() + 1,
    }
}

pub fn will_chunk(transcript: &Transcript, options: Options) -> bool {
    chunk(
        &transcript.segments,
        options.chunk_chars.max(MIN_CHUNK_CHARS),
        options.timestamps,
    )
    .len()
        > 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::builtin_template;

    fn seg(start_ms: u32, text: &str) -> Segment {
        Segment::new(start_ms, start_ms + 1000, text)
    }

    fn transcript_of(segments: Vec<Segment>) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments,
        }
    }

    fn long_transcript() -> Transcript {
        let segments = (0..3600)
            .map(|i| {
                seg(
                    i * 3000,
                    "This is a sentence of about fifty characters long.",
                )
            })
            .collect();
        transcript_of(segments)
    }

    fn meeting() -> Template {
        builtin_template("meeting").expect("built in")
    }

    #[test]
    fn a_summary_defaults_to_the_language_that_was_spoken() {
        let Plan::Single(prompt) = plan(
            &meeting(),
            &transcript_of(vec![seg(0, "hola")]),
            Options::default(),
        ) else {
            panic!("expected a single-request plan");
        };
        assert!(prompt
            .system
            .contains("Answer in the language the transcript is in"));
        assert!(!prompt.system.contains("Write the entire summary in"));
    }

    #[test]
    fn a_chosen_summary_language_is_pinned_in_every_pass() {
        let french = Options {
            language: Some("French".to_owned()),
            ..Options::default()
        };

        let Plan::Single(single) = plan(
            &meeting(),
            &transcript_of(vec![seg(0, "hola")]),
            french.clone(),
        ) else {
            panic!("expected a single-request plan");
        };
        assert!(single.system.contains("Write the entire summary in French"));
        assert!(!single
            .system
            .contains("Answer in the language the transcript is in"));

        let Plan::MapReduce { parts, reduce } = plan(&meeting(), &long_transcript(), french) else {
            panic!("expected a map-reduce plan");
        };
        assert!(parts
            .iter()
            .all(|part| part.system.contains("Write the entire summary in French")));
        assert!(reduce
            .build(&["a partial".to_owned()])
            .system
            .contains("Write the entire summary in French"));
    }

    #[test]
    fn a_short_transcript_is_one_request() {
        let plan = plan(
            &meeting(),
            &transcript_of(vec![seg(0, "Hello"), seg(1000, "Goodbye")]),
            Options::default(),
        );
        assert!(matches!(plan, Plan::Single(_)));
        assert_eq!(request_count(&plan), 1);
    }

    #[test]
    fn the_single_prompt_carries_the_template_and_the_shared_rules() {
        let Plan::Single(prompt) = plan(
            &meeting(),
            &transcript_of(vec![seg(0, "Hi")]),
            Options::default(),
        ) else {
            panic!("expected a single request");
        };
        assert!(prompt.system.contains("## Action items"));
        assert!(prompt.system.contains("Never add a fact"));
        assert!(prompt.user.contains("Hi"));
    }

    #[test]
    fn three_hours_of_speech_is_split_rather_than_silently_truncated() {
        let plan = plan(&meeting(), &long_transcript(), Options::default());
        let Plan::MapReduce { parts, .. } = &plan else {
            panic!("expected a map-reduce plan");
        };
        assert!(parts.len() > 5, "got {} parts", parts.len());
        assert_eq!(request_count(&plan), parts.len() + 1);
    }

    #[test]
    fn no_part_exceeds_the_budget() {
        let budget = 4_000;
        let options = Options {
            chunk_chars: budget,
            ..Options::default()
        };
        let Plan::MapReduce { parts, .. } = plan(&meeting(), &long_transcript(), options) else {
            panic!("expected a map-reduce plan");
        };
        for part in &parts {
            assert!(part.user.len() < budget + 500, "{}", part.user.len());
        }
    }

    #[test]
    fn splitting_loses_no_segments() {
        let segments: Vec<Segment> = (0..500)
            .map(|i| seg(i * 1000, &format!("line {i}")))
            .collect();
        let chunks = chunk(&segments, 2_000, true);
        assert!(chunks.len() > 1);
        assert_eq!(
            chunks.iter().map(|c| c.len()).sum::<usize>(),
            segments.len()
        );
    }

    #[test]
    fn splits_fall_on_segment_boundaries() {
        let segments: Vec<Segment> = (0..200)
            .map(|i| seg(i * 1000, &format!("line {i}")))
            .collect();
        let chunks = chunk(&segments, 2_000, false);

        let mut expected = 0;
        for c in &chunks {
            assert_eq!(c.as_ptr(), segments[expected..].as_ptr());
            expected += c.len();
        }
        assert_eq!(expected, segments.len());
    }

    #[test]
    fn one_oversized_segment_is_kept_whole_rather_than_cut_mid_sentence() {
        let segments = [seg(0, &"x".repeat(50_000))];
        let chunks = chunk(&segments, 4_000, false);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    #[test]
    fn the_map_pass_does_not_impose_the_template_structure_on_every_part() {
        let Plan::MapReduce { parts, .. } =
            plan(&meeting(), &long_transcript(), Options::default())
        else {
            panic!("expected a map-reduce plan");
        };
        assert!(!parts[0].system.contains("## Action items"));
        assert!(parts[0].system.contains("Preserve detail"));
    }

    #[test]
    fn the_reduce_pass_applies_the_template_and_receives_the_partials_in_order() {
        let Plan::MapReduce { reduce, .. } =
            plan(&meeting(), &long_transcript(), Options::default())
        else {
            panic!("expected a map-reduce plan");
        };
        let prompt = reduce.build(&["first bit".to_owned(), "second bit".to_owned()]);

        assert!(prompt.system.contains("## Action items"));
        let first = prompt.user.find("first bit").expect("first partial");
        let second = prompt.user.find("second bit").expect("second partial");
        assert!(first < second, "partials must stay in recording order");
    }

    #[test]
    fn a_named_speaker_reaches_the_model_rather_than_stopping_at_the_ui() {
        let t = transcript_of(vec![
            seg(0, "We ship on Friday.").by("Anna Weiss"),
            seg(3_000, "Agreed.").by("Max Kruger"),
        ]);

        let Plan::Single(prompt) = plan(&meeting(), &t, Options::default()) else {
            panic!("expected a single request");
        };

        assert!(
            prompt.user.contains("Anna Weiss: We ship on Friday."),
            "{}",
            prompt.user
        );
        assert!(
            prompt.user.contains("Max Kruger: Agreed."),
            "{}",
            prompt.user
        );
    }

    #[test]
    fn an_attributed_transcript_tells_the_model_the_names_can_be_trusted() {
        let attributed = transcript_of(vec![seg(0, "Hello.").by("Anna Weiss")]);
        let Plan::Single(prompt) = plan(&meeting(), &attributed, Options::default()) else {
            panic!()
        };
        assert!(
            prompt.system.contains("own microphone"),
            "{}",
            prompt.system
        );
    }

    #[test]
    fn an_unattributed_transcript_says_nothing_about_speakers() {
        let plain = transcript_of(vec![seg(0, "Hello.")]);
        let Plan::Single(prompt) = plan(&meeting(), &plain, Options::default()) else {
            panic!()
        };
        assert!(!prompt.system.contains("own microphone"));
        assert!(!prompt.user.contains(": Hello"), "{}", prompt.user);
    }

    #[test]
    fn the_map_pass_keeps_attribution_too() {
        let long = transcript_of(
            (0..3600)
                .map(|i| {
                    seg(
                        i * 3000,
                        "This is a sentence of about fifty characters long.",
                    )
                    .by(if i % 2 == 0 {
                        "Anna Weiss"
                    } else {
                        "Max Kruger"
                    })
                })
                .collect(),
        );

        let Plan::MapReduce { parts, .. } = plan(&meeting(), &long, Options::default()) else {
            panic!("expected a map-reduce plan");
        };
        assert!(parts[0].system.contains("own microphone"));
        assert!(
            parts[0].user.contains("Anna Weiss: "),
            "names must survive chunking"
        );
    }

    #[test]
    fn timestamps_can_be_turned_off_to_save_tokens() {
        let t = transcript_of(vec![seg(72_000, "Hello")]);

        let Plan::Single(with) = plan(&meeting(), &t, Options::default()) else {
            panic!()
        };
        assert!(with.user.contains("[1:12] Hello"));

        let Plan::Single(without) = plan(
            &meeting(),
            &t,
            Options {
                timestamps: false,
                ..Options::default()
            },
        ) else {
            panic!()
        };
        assert!(without.user.contains("Hello"));
        assert!(!without.user.contains("1:12"));
    }

    #[test]
    fn silent_segments_do_not_reach_the_model() {
        let t = transcript_of(vec![seg(0, "Hello"), seg(1000, "   "), seg(2000, "Bye")]);
        let Plan::Single(prompt) = plan(&meeting(), &t, Options::default()) else {
            panic!()
        };
        assert_eq!(prompt.user.matches('\n').count(), 3);
    }

    #[test]
    fn an_absurdly_small_budget_is_clamped_rather_than_producing_a_request_per_word() {
        let t = long_transcript();
        let plan = plan(
            &meeting(),
            &t,
            Options {
                chunk_chars: 1,
                ..Options::default()
            },
        );
        assert!(request_count(&plan) < t.segments.len());
    }

    #[test]
    fn an_empty_transcript_still_produces_a_valid_single_request() {
        let plan = plan(&meeting(), &transcript_of(Vec::new()), Options::default());
        assert!(matches!(plan, Plan::Single(_)));
    }

    #[test]
    fn will_chunk_agrees_with_the_plan_it_predicts() {
        let short = transcript_of(vec![seg(0, "Hi")]);
        let long = long_transcript();

        assert!(!will_chunk(&short, Options::default()));
        assert!(will_chunk(&long, Options::default()));

        assert_eq!(
            request_count(&plan(&meeting(), &short, Options::default())),
            1
        );
        assert!(request_count(&plan(&meeting(), &long, Options::default())) > 1);
    }
}
