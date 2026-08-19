use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Redaction {
    pub contacts: bool,

    pub names: Vec<String>,
}

impl Redaction {
    pub fn is_empty(&self) -> bool {
        !self.contacts && self.names.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Removed {
    pub emails: usize,
    pub numbers: usize,
    pub names: usize,
}

impl Removed {
    pub fn total(&self) -> usize {
        self.emails + self.numbers + self.names
    }
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("a valid pattern")
});

static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        \(?\+?\d[\d\ \-\.\(\)]{6,}\d   # a phone number, however it is spaced
      | \b[A-Z]{2}\d{2}[A-Z0-9]{8,28}\b # an IBAN
      | \b\d{7,}\b                      # any long run of digits
    ",
    )
    .expect("a valid pattern")
});

pub fn placeholders(what: &Redaction) -> Vec<(String, String)> {
    let mut names: Vec<&str> = what
        .names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));

    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| (name.to_owned(), format!("[name {}]", index + 1)))
        .collect()
}

pub fn redact(text: &str, what: &Redaction) -> (String, Removed) {
    redact_with(text, what, &placeholders(what))
}

fn redact_with(text: &str, what: &Redaction, names: &[(String, String)]) -> (String, Removed) {
    let mut removed = Removed::default();

    if what.is_empty() {
        return (text.to_owned(), removed);
    }

    let mut out = text.to_owned();

    if what.contacts {
        removed.emails = EMAIL.find_iter(&out).count();
        out = EMAIL.replace_all(&out, "[email]").into_owned();

        removed.numbers = NUMBER.find_iter(&out).count();
        out = NUMBER.replace_all(&out, "[number]").into_owned();
    }

    for (name, placeholder) in names {
        let (replaced, count) = replace_words(&out, name, placeholder);
        out = replaced;
        removed.names += count;
    }

    (out, removed)
}

pub fn redact_transcript(
    transcript: &crate::Transcript,
    what: &Redaction,
) -> (crate::Transcript, Removed) {
    let mut removed = Removed::default();

    if what.is_empty() {
        return (transcript.clone(), removed);
    }

    let names = placeholders(what);
    let mut out = transcript.clone();

    for segment in &mut out.segments {
        let (text, went) = redact_with(&segment.text, what, &names);
        segment.text = text;
        removed.emails += went.emails;
        removed.numbers += went.numbers;
        removed.names += went.names;

        if let Some(speaker) = &segment.speaker {
            if let Some((_, placeholder)) = names
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(speaker.trim()))
            {
                segment.speaker = Some(placeholder.clone());
            }
        }
    }

    (out, removed)
}

fn replace_words(text: &str, needle: &str, placeholder: &str) -> (String, usize) {
    if needle.is_empty() {
        return (text.to_owned(), 0);
    }

    let haystack = text.to_lowercase();
    let wanted = needle.to_lowercase();

    let mut out = String::with_capacity(text.len());
    let mut count = 0usize;
    let mut at = 0usize;

    while let Some(found) = haystack[at..].find(&wanted) {
        let start = at + found;
        let end = start + wanted.len();

        let before_is_word = text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric());
        let after_is_word = text[end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric());

        out.push_str(&text[at..start]);

        if before_is_word || after_is_word {
            out.push_str(&text[start..end]);
        } else {
            out.push_str(placeholder);
            count += 1;
        }

        at = end;
    }

    out.push_str(&text[at..]);
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contacts() -> Redaction {
        Redaction {
            contacts: true,
            names: Vec::new(),
        }
    }

    #[test]
    fn an_email_address_never_leaves() {
        let (text, removed) = redact(
            "Send it to anna.mueller+work@example.co.uk today",
            &contacts(),
        );
        assert_eq!(text, "Send it to [email] today");
        assert_eq!(removed.emails, 1);
    }

    #[test]
    fn phone_numbers_go_however_they_were_written() {
        for spoken in [
            "call me on +49 170 1234567",
            "call me on (030) 123 456 78",
            "call me on 030-1234-5678",
        ] {
            let (text, removed) = redact(spoken, &contacts());
            assert!(text.starts_with("call me on [number]"), "got: {text}");
            assert_eq!(removed.numbers, 1, "for {spoken:?}");
        }
    }

    #[test]
    fn an_iban_and_a_card_number_go_too() {
        let (text, _) = redact("pay to DE89370400440532013000 by Friday", &contacts());
        assert_eq!(text, "pay to [number] by Friday");

        let (text, _) = redact("the card is 4111111111111111", &contacts());
        assert_eq!(text, "the card is [number]");
    }

    #[test]
    fn ordinary_numbers_in_speech_are_left_alone() {
        let (text, removed) = redact("we agreed the 15th, about 40 people, £250", &contacts());
        assert_eq!(text, "we agreed the 15th, about 40 people, £250");
        assert_eq!(removed.total(), 0);
    }

    #[test]
    fn names_are_replaced_only_when_they_are_known() {
        let what = Redaction {
            contacts: false,
            names: vec!["Anna".to_owned(), "Max Kruger".to_owned()],
        };

        let (text, removed) = redact("Anna asked Max Kruger to write it. anna agreed.", &what);

        assert_eq!(
            text,
            "[name 2] asked [name 1] to write it. [name 2] agreed."
        );
        assert_eq!(removed.names, 3);
    }

    #[test]
    fn a_name_inside_another_word_is_not_that_person() {
        let what = Redaction {
            contacts: false,
            names: vec!["Ann".to_owned()],
        };

        let (text, removed) = redact("the announcement mentioned Ann", &what);
        assert_eq!(text, "the announcement mentioned [name 1]");
        assert_eq!(removed.names, 1);
    }

    #[test]
    fn a_transcript_keeps_its_shape_and_loses_its_names() {
        let transcript = crate::Transcript {
            language: "en".to_owned(),
            model: "small".to_owned(),
            segments: vec![
                crate::Segment {
                    start_ms: 0,
                    end_ms: 2_000,
                    text: "Anna, mail it to me at max@example.com".to_owned(),
                    speaker: Some("Max".to_owned()),
                },
                crate::Segment {
                    start_ms: 2_000,
                    end_ms: 4_000,
                    text: "Will do, Max.".to_owned(),
                    speaker: Some("Anna".to_owned()),
                },
            ],
        };

        let what = Redaction {
            contacts: true,
            names: vec!["Anna".to_owned(), "Max".to_owned()],
        };
        let (out, removed) = redact_transcript(&transcript, &what);

        let anna = out.segments[1].speaker.clone().unwrap();
        let max = out.segments[0].speaker.clone().unwrap();
        assert_ne!(anna, max);
        assert!(out.segments[0].text.starts_with(&anna));
        assert!(out.segments[1].text.contains(&max));
        assert!(out.segments[0].text.ends_with("[email]"));

        assert_eq!(out.segments[1].start_ms, 2_000);
        assert_eq!(out.segments[1].end_ms, 4_000);
        assert_eq!(out.language, "en");

        assert_eq!(removed.emails, 1);
        assert_eq!(removed.names, 2);
    }

    #[test]
    fn nothing_asked_for_means_nothing_changed() {
        let original = "Call anna@example.com on 0301234567";
        let (text, removed) = redact(original, &Redaction::default());
        assert_eq!(text, original);
        assert_eq!(removed.total(), 0);
    }

    #[test]
    fn the_report_is_honest_about_how_much_went() {
        let what = Redaction {
            contacts: true,
            names: vec!["Anna".to_owned()],
        };

        let (_, removed) = redact("Anna: anna@example.com, +49 170 1234567", &what);
        assert_eq!(removed.emails, 1);
        assert_eq!(removed.numbers, 1);
        assert_eq!(removed.names, 1);
        assert_eq!(removed.total(), 3);
    }
}
