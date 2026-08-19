use zscribe_core::{ProviderId, Redaction, Transcript};
use zscribe_store::settings::AppSettings;

pub fn wanted(settings: &AppSettings, provider: ProviderId, transcript: &Transcript) -> Redaction {
    if provider == ProviderId::Ollama {
        return Redaction::default();
    }

    let mut names: Vec<String> = settings.privacy.redact_terms.clone();

    if settings.privacy.redact_speakers {
        for segment in &transcript.segments {
            let Some(speaker) = segment.speaker.as_ref().map(|s| s.trim()) else {
                continue;
            };

            if speaker.is_empty()
                || speaker.starts_with("Speaker ")
                || names.iter().any(|name| name.eq_ignore_ascii_case(speaker))
            {
                continue;
            }
            names.push(speaker.to_owned());
        }
    }

    Redaction {
        contacts: settings.privacy.redact_contacts,
        names,
    }
}

pub fn wanted_for_text(settings: &AppSettings, provider: ProviderId) -> Redaction {
    if provider == ProviderId::Ollama {
        return Redaction::default();
    }

    Redaction {
        contacts: settings.privacy.redact_contacts,
        names: settings.privacy.redact_terms.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zscribe_core::Segment;

    fn transcript(speakers: &[&str]) -> Transcript {
        Transcript {
            language: "en".to_owned(),
            model: "small".to_owned(),
            segments: speakers
                .iter()
                .enumerate()
                .map(|(index, speaker)| Segment {
                    start_ms: index as u32 * 1000,
                    end_ms: index as u32 * 1000 + 1000,
                    text: "hello".to_owned(),
                    speaker: Some((*speaker).to_owned()),
                })
                .collect(),
        }
    }

    #[test]
    fn a_local_model_is_sent_everything() {
        let mut settings = AppSettings::default();
        settings.privacy.redact_speakers = true;
        settings.privacy.redact_terms = vec!["Acme".to_owned()];

        let what = wanted(&settings, ProviderId::Ollama, &transcript(&["Anna"]));
        assert!(what.is_empty());
    }

    #[test]
    fn contacts_go_to_the_cloud_by_default() {
        let settings = AppSettings::default();
        let what = wanted(&settings, ProviderId::Gemini, &transcript(&["Anna"]));

        assert!(what.contacts, "contacts are redacted unless turned off");
        assert!(
            what.names.is_empty(),
            "names are not, until the user asks for it"
        );
    }

    #[test]
    fn speakers_come_from_the_recording_once_each() {
        let mut settings = AppSettings::default();
        settings.privacy.redact_speakers = true;

        let what = wanted(
            &settings,
            ProviderId::Gemini,
            &transcript(&["Anna", "Max", "Anna", "Speaker 2"]),
        );

        assert_eq!(what.names, vec!["Anna".to_owned(), "Max".to_owned()]);
    }

    #[test]
    fn a_term_the_user_listed_is_never_repeated_by_a_speaker_label() {
        let mut settings = AppSettings::default();
        settings.privacy.redact_speakers = true;
        settings.privacy.redact_terms = vec!["anna".to_owned()];

        let what = wanted(&settings, ProviderId::Gemini, &transcript(&["Anna", "Max"]));

        assert_eq!(what.names, vec!["anna".to_owned(), "Max".to_owned()]);
    }
}
