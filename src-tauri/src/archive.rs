use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;
use zscribe_core::archive::Retrieved;
use zscribe_providers::ollama::Ollama;
use zscribe_store::recordings::PassageHit;

use crate::commands::CommandError;
use crate::events;
use crate::state::AppState;

const RETRIEVE: usize = 12;

const FLOOR: f32 = 0.5;

const RELATIVE: f32 = 0.85;

const BATCH: usize = 16;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArchiveStatus {
    pub embedding_model: String,

    pub transcribed: u32,

    pub indexed: u32,

    pub passages: u32,

    pub ollama_ready: bool,

    pub model_ready: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IndexProgress {
    pub done: u32,
    pub total: u32,

    pub title: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArchiveAnswer {
    pub text: String,
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Citation {
    pub recording_id: String,
    pub title: String,

    #[ts(type = "number")]
    pub started_at: i64,

    pub start_ms: u32,
    pub text: String,
    pub score: f32,
}

fn embedder(app: &AppHandle) -> Result<(Ollama, String), CommandError> {
    let settings = app.state::<AppState>().settings();

    let base_url = settings
        .providers
        .iter()
        .find(|profile| profile.id == zscribe_core::ProviderId::Ollama)
        .and_then(|profile| profile.base_url.clone())
        .unwrap_or_else(|| {
            zscribe_core::ProviderId::Ollama
                .default_base_url()
                .to_owned()
        });

    let ollama = Ollama::new(base_url).map_err(|err| {
        CommandError::new(
            "ollama",
            err.to_string(),
            "Asking across recordings needs Ollama running on this machine.",
        )
    })?;

    Ok((ollama, settings.archive.embedding_model))
}

pub async fn status(app: AppHandle) -> Result<ArchiveStatus, CommandError> {
    let (ollama, model) = embedder(&app)?;

    let (transcribed, indexed, passages) = {
        let state = app.state::<AppState>();
        let recordings = state.recordings.lock().expect("recordings lock poisoned");

        let (indexed, passages) = recordings.index_size(&model).map_err(storage)?;
        let waiting = recordings.unindexed(&model).map_err(storage)?.len() as u32;

        (indexed + waiting, indexed, passages)
    };

    let models = zscribe_providers::Provider::list_models(&ollama).await;
    let ollama_ready = models.is_ok();
    let model_ready = models
        .map(|models| models.iter().any(|entry| tag_matches(&entry.id, &model)))
        .unwrap_or(false);

    Ok(ArchiveStatus {
        embedding_model: model.clone(),
        transcribed,
        indexed,
        passages,
        ollama_ready,
        model_ready,
    })
}

fn tag_matches(listed: &str, wanted: &str) -> bool {
    listed == wanted
        || listed.split(':').next() == Some(wanted)
        || wanted.split(':').next() == listed.split(':').next()
}

pub async fn index(app: AppHandle) -> Result<u32, CommandError> {
    let (ollama, model) = embedder(&app)?;

    let waiting = {
        let state = app.state::<AppState>();
        let recordings = state.recordings.lock().expect("recordings lock poisoned");
        recordings.unindexed(&model).map_err(storage)?
    };

    let total = waiting.len() as u32;
    let mut done = 0u32;

    for id in waiting {
        let prepared = {
            let state = app.state::<AppState>();
            let recordings = state.recordings.lock().expect("recordings lock poisoned");

            let transcript = recordings.transcript(&id).map_err(storage)?;
            let title = recordings
                .get(&id)
                .map_err(storage)?
                .map(|detail| detail.recording.title)
                .unwrap_or_default();

            transcript.map(|transcript| (title, zscribe_core::passages(&transcript.segments)))
        };

        let Some((title, passages)) = prepared else {
            continue;
        };

        let _ = app.emit(
            events::INDEX_PROGRESS,
            &IndexProgress {
                done,
                total,
                title: title.clone(),
            },
        );

        let mut embedded: Vec<(zscribe_core::Passage, Vec<f32>)> = Vec::new();

        for batch in passages.chunks(BATCH) {
            let inputs: Vec<String> = batch.iter().map(|passage| passage.text.clone()).collect();

            let vectors = ollama.embed(&model, &inputs).await.map_err(|err| {
                CommandError::new(
                    "embedding",
                    err.to_string(),
                    "Check that Ollama is running and that the embedding model is installed.",
                )
            })?;

            embedded.extend(batch.iter().cloned().zip(vectors));
        }

        {
            let state = app.state::<AppState>();
            let recordings = state.recordings.lock().expect("recordings lock poisoned");
            recordings
                .set_passages(&id, &model, &embedded)
                .map_err(storage)?;
        }

        done += 1;
        tracing::info!(recording = %id, passages = embedded.len(), "indexed for the archive");
    }

    let _ = app.emit(
        events::INDEX_PROGRESS,
        &IndexProgress {
            done,
            total,
            title: String::new(),
        },
    );

    Ok(done)
}

pub async fn ask(app: AppHandle, question: String) -> Result<ArchiveAnswer, CommandError> {
    let question = question.trim().to_owned();
    if question.is_empty() {
        return Err(CommandError::new(
            "empty",
            "there is no question to answer",
            "Type what you want to know.",
        ));
    }

    let (ollama, model) = embedder(&app)?;

    let vectors = ollama
        .embed(&model, std::slice::from_ref(&question))
        .await
        .map_err(|err| {
            CommandError::new(
                "embedding",
                err.to_string(),
                "Check that Ollama is running and that the embedding model is installed.",
            )
        })?;

    let Some(embedding) = vectors.first() else {
        return Err(CommandError::new(
            "embedding",
            "the embedding model returned nothing for that question",
            "Try again, or pick a different embedding model.",
        ));
    };

    let hits: Vec<PassageHit> = {
        let state = app.state::<AppState>();
        let recordings = state.recordings.lock().expect("recordings lock poisoned");
        recordings
            .nearest_passages(embedding, &model, RETRIEVE)
            .map_err(storage)?
    };

    let best = hits.first().map_or(0.0, |hit| hit.score);
    let bar = FLOOR.max(best * RELATIVE);

    let hits: Vec<PassageHit> = hits.into_iter().filter(|hit| hit.score >= bar).collect();

    if hits.is_empty() {
        return Ok(ArchiveAnswer {
            text: "None of your recordings seem to cover that. If you have just recorded \
                   something, index it and ask again."
                .to_owned(),
            citations: Vec::new(),
        });
    }

    let when: Vec<String> = hits.iter().map(|hit| day_of(hit.started_at)).collect();

    let redaction = {
        let state = app.state::<AppState>();
        let settings = state.settings();
        crate::privacy::wanted_for_text(&settings, settings.active_provider)
    };
    let sent: Vec<(String, String)> = hits
        .iter()
        .map(|hit| {
            (
                zscribe_core::redact(&hit.title, &redaction).0,
                zscribe_core::redact(&hit.text, &redaction).0,
            )
        })
        .collect();

    let retrieved: Vec<Retrieved<'_>> = hits
        .iter()
        .zip(&when)
        .zip(&sent)
        .map(|((hit, when), (title, text))| Retrieved {
            title,
            when,
            start_ms: hit.start_ms,
            text,
        })
        .collect();

    let prompt =
        zscribe_core::archive::prompt(&zscribe_core::redact(&question, &redaction).0, &retrieved);
    let text = crate::commands::complete_with_active_provider(&app, prompt).await?;

    Ok(ArchiveAnswer {
        text,
        citations: hits
            .into_iter()
            .map(|hit| Citation {
                recording_id: hit.recording_id,
                title: hit.title,
                started_at: hit.started_at,
                start_ms: hit.start_ms,
                text: hit.text,
                score: hit.score,
            })
            .collect(),
    })
}

fn day_of(started_at: i64) -> String {
    let when = time::OffsetDateTime::from_unix_timestamp(started_at)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(crate::local_offset());

    format!("{} {}", when.day(), when.month())
}

fn storage(err: zscribe_store::RecordingsError) -> CommandError {
    tracing::error!(%err, "the archive index could not be read");
    CommandError::new(
        "storage",
        "the archive index could not be read",
        "Check that ZScribe's data directory is readable and has free space.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_is_recognised_however_ollama_tags_it() {
        assert!(tag_matches("nomic-embed-text:latest", "nomic-embed-text"));
        assert!(tag_matches("nomic-embed-text", "nomic-embed-text:latest"));
        assert!(tag_matches("nomic-embed-text", "nomic-embed-text"));
        assert!(tag_matches("mxbai-embed-large:335m", "mxbai-embed-large"));
    }

    #[test]
    fn a_different_model_is_not_mistaken_for_the_wanted_one() {
        assert!(!tag_matches("mxbai-embed-large", "nomic-embed-text"));
        assert!(!tag_matches("qwen2.5:7b", "nomic-embed-text"));
    }
}
