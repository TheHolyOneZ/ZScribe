use crate::error::ProviderError;
use crate::history_messages;
use crate::http;
use crate::{Completion, CompletionRequest, ModelInfo, Provider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use zscribe_core::{ProviderId, TokenUsage};

const ID: ProviderId = ProviderId::Ollama;

pub struct Ollama {
    client: reqwest::Client,
    base_url: String,
}

impl Ollama {
    pub fn new(base_url: String) -> Result<Self, ProviderError> {
        Ok(Self {
            client: http::client()?,
            base_url,
        })
    }
}

#[async_trait]
impl Provider for Ollama {
    fn id(&self) -> ProviderId {
        ID
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        let body: TagsResponse = http::error_for_status(ID, response)
            .await?
            .json()
            .await
            .map_err(|err| ProviderError::Malformed(err.to_string()))?;

        let mut models: Vec<ModelInfo> = body
            .models
            .into_iter()
            .map(|model| ModelInfo::new(model.name))
            .collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(models)
    }

    async fn complete(
        &self,
        request: &CompletionRequest,
        cancel: &CancellationToken,
    ) -> Result<Completion, ProviderError> {
        let url = format!("{}/api/chat", self.base_url);

        let body = serde_json::json!({
            "model": request.model,
            "messages": messages(request),
            "stream": false,
            "options": {
                "temperature": request.params.temperature,
                "num_predict": request.params.max_output_tokens,


                "num_ctx": context_window(request),


                "repeat_penalty": 1.2,
            },
        });

        http::with_retry(cancel, |_attempt| {
            let request_future = self.client.post(&url).json(&body).send();

            async move {
                let response = tokio::select! {
                    _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                    result = request_future => result.map_err(|err| http::map_transport_error(ID, &err))?,
                };

                let payload: ChatResponse = http::error_for_status(ID, response)
                    .await?
                    .json()
                    .await
                    .map_err(|err| ProviderError::Malformed(err.to_string()))?;

                parse_completion(payload)
            }
        })
        .await
    }
}

fn context_window(request: &CompletionRequest) -> u32 {
    let input_chars = request.prompt.system.len()
        + request.prompt.user.len()
        + request
            .history
            .iter()
            .map(|turn| turn.content.len())
            .sum::<usize>();

    let input_tokens = (input_chars / 4 + 512) as u32;
    let needed = input_tokens.saturating_add(request.params.max_output_tokens);

    (needed.div_ceil(2048) * 2048).clamp(4096, 32768)
}

fn messages(request: &CompletionRequest) -> Vec<serde_json::Value> {
    let mut out = vec![serde_json::json!({
        "role": "system",
        "content": request.prompt.system,
    })];

    out.extend(history_messages(&request.history));
    out.push(serde_json::json!({
        "role": "user",
        "content": request.prompt.user,
    }));
    out
}

fn parse_completion(payload: ChatResponse) -> Result<Completion, ProviderError> {
    if payload.done_reason.as_deref() == Some("length") {
        return Err(ProviderError::Truncated);
    }

    let text = payload
        .message
        .map(|message| message.content)
        .unwrap_or_default();

    if text.is_empty() {
        return Err(ProviderError::Malformed(
            "response contained no text".to_owned(),
        ));
    }

    Ok(Completion {
        text,
        usage: TokenUsage {
            input: payload.prompt_eval_count,
            output: payload.eval_count,
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullProgress {
    #[ts(type = "number")]
    pub downloaded_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub percent: u8,

    pub stage: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Running {
    pub model: String,

    #[ts(type = "number")]
    pub size_bytes: u64,

    #[ts(type = "number")]
    pub vram_bytes: u64,
}

impl Running {
    pub fn on_gpu(&self) -> bool {
        self.size_bytes > 0 && self.vram_bytes * 100 >= self.size_bytes * 90
    }
}

impl Ollama {
    pub async fn embed(
        &self,
        model: &str,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let response = http::download_client()?
            .post(format!("{}/api/embed", self.base_url))
            .json(&serde_json::json!({ "model": model, "input": inputs }))
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        let body: EmbedResponse = http::error_for_status(ID, response)
            .await?
            .json()
            .await
            .map_err(|err| ProviderError::Malformed(err.to_string()))?;

        if body.embeddings.len() != inputs.len() {
            return Err(ProviderError::Malformed(format!(
                "asked for {} embeddings and got {}",
                inputs.len(),
                body.embeddings.len()
            )));
        }

        Ok(body.embeddings)
    }

    pub async fn pull(
        &self,
        model: &str,
        cancel: &CancellationToken,
        mut on_progress: impl FnMut(PullProgress),
    ) -> Result<(), ProviderError> {
        let mut response = http::download_client()?
            .post(format!("{}/api/pull", self.base_url))
            .json(&serde_json::json!({ "model": model, "stream": true }))
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        response = http::error_for_status(ID, response).await?;

        let mut last: Option<(u8, String)> = None;
        let mut buffer = String::new();

        loop {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }

            let chunk = tokio::select! {
                _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                result = response.chunk() => {
                    result.map_err(|err| http::map_transport_error(ID, &err))?
                }
            };

            let Some(chunk) = chunk else { break };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline) = buffer.find('\n') {
                let line: String = buffer.drain(..=newline).collect();

                if let Some(progress) = parse_pull_line(line.trim()) {
                    if worth_reporting(last.as_ref(), &progress) {
                        last = Some((progress.percent, progress.stage.clone()));
                        on_progress(progress);
                    }
                }
            }
        }

        tracing::info!(model, "model pulled");
        Ok(())
    }

    pub async fn running(&self) -> Result<Vec<Running>, ProviderError> {
        let response = self
            .client
            .get(format!("{}/api/ps", self.base_url))
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        let body: PsResponse = http::error_for_status(ID, response)
            .await?
            .json()
            .await
            .map_err(|err| ProviderError::Malformed(err.to_string()))?;

        Ok(body
            .models
            .into_iter()
            .map(|entry| Running {
                model: entry.name,
                size_bytes: entry.size,
                vram_bytes: entry.size_vram,
            })
            .collect())
    }
}

fn worth_reporting(last: Option<&(u8, String)>, progress: &PullProgress) -> bool {
    match last {
        None => true,
        Some((percent, stage)) => *percent != progress.percent || *stage != progress.stage,
    }
}

fn parse_pull_line(line: &str) -> Option<PullProgress> {
    if line.is_empty() {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(line).ok()?;

    if let Some(error) = value.get("error").and_then(|e| e.as_str()) {
        tracing::warn!(error, "ollama reported an error while pulling");
        return None;
    }

    let stage = value
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("downloading")
        .to_owned();

    let total = value
        .get("total")
        .and_then(|t| t.as_u64())
        .unwrap_or_default();

    if total == 0 {
        return Some(PullProgress {
            downloaded_bytes: 0,
            total_bytes: 0,
            percent: 0,
            stage,
        });
    }

    let completed = value.get("completed").and_then(|c| c.as_u64()).unwrap_or(0);

    Some(PullProgress {
        downloaded_bytes: completed,
        total_bytes: total,
        percent: (completed.min(total) * 100).checked_div(total).unwrap_or(0) as u8,
        stage,
    })
}

#[derive(Debug, Deserialize)]
struct PsResponse {
    #[serde(default)]
    models: Vec<PsEntry>,
}

#[derive(Debug, Deserialize)]
struct PsEntry {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    size_vram: u64,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    #[serde(default)]
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: serde_json::Value) -> Result<Completion, ProviderError> {
        parse_completion(serde_json::from_value(json).expect("fixture parses"))
    }

    fn request_with_history() -> CompletionRequest {
        CompletionRequest::new(
            "test-model",
            zscribe_core::Prompt {
                system: "rules".to_owned(),
                user: "and who was that?".to_owned(),
            },
        )
        .with_history(vec![
            zscribe_core::Turn::user("who is sending it?"),
            zscribe_core::Turn::assistant("Anna."),
        ])
    }

    #[test]
    fn the_conversation_is_sent_as_real_turns_not_folded_into_one_message() {
        let rendered = messages(&request_with_history());

        assert_eq!(rendered.len(), 4, "system, two history turns, the question");
        assert_eq!(rendered[0]["role"], "system");
        assert_eq!(rendered[1]["role"], "user");
        assert_eq!(rendered[2]["role"], "assistant");
        assert_eq!(rendered[3]["content"], "and who was that?");
    }

    #[test]
    fn summarising_sends_no_history() {
        let plain = CompletionRequest::new(
            "m",
            zscribe_core::Prompt {
                system: "s".to_owned(),
                user: "u".to_owned(),
            },
        );
        assert_eq!(messages(&plain).len(), 2);
    }

    #[test]
    fn extracts_the_text_and_the_token_counts() {
        let result = parse(serde_json::json!({
            "message": { "role": "assistant", "content": "## Decisions\n\n- Ship it" },
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 1840,
            "eval_count": 210
        }))
        .expect("parses");

        assert_eq!(result.text, "## Decisions\n\n- Ship it");
        assert_eq!(result.usage.input, 1840);
        assert_eq!(result.usage.output, 210);
    }

    #[test]
    fn hitting_the_output_limit_is_an_error_not_a_short_summary() {
        let result = parse(serde_json::json!({
            "message": { "content": "## Decisions\n\n- Shi" },
            "done_reason": "length"
        }));
        assert!(matches!(result, Err(ProviderError::Truncated)));
    }

    #[test]
    fn a_missing_message_is_malformed() {
        assert!(matches!(
            parse(serde_json::json!({ "done": true })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn an_empty_message_is_malformed_rather_than_an_empty_summary() {
        assert!(matches!(
            parse(serde_json::json!({ "message": { "content": "" } })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn a_pull_line_with_byte_counts_becomes_progress() {
        let progress = parse_pull_line(
            r#"{"status":"pulling c539","digest":"sha256:x","total":1000,"completed":250}"#,
        )
        .expect("has counts");

        assert_eq!(progress.percent, 25);
        assert_eq!(progress.downloaded_bytes, 250);
    }

    #[test]
    fn stage_lines_without_counts_are_reported_as_stages() {
        let manifest = parse_pull_line(r#"{"status":"pulling manifest"}"#).expect("a stage");
        assert_eq!(manifest.stage, "pulling manifest");
        assert_eq!(manifest.total_bytes, 0, "nothing to measure yet");

        let verifying =
            parse_pull_line(r#"{"status":"verifying sha256 digest"}"#).expect("a stage");
        assert_eq!(verifying.stage, "verifying sha256 digest");
        assert_eq!(verifying.total_bytes, 0);
    }

    #[test]
    fn the_very_first_update_is_always_reported() {
        let first = PullProgress {
            downloaded_bytes: 0,
            total_bytes: 5_000_000_000,
            percent: 0,
            stage: "pulling".to_owned(),
        };
        assert!(worth_reporting(None, &first));
    }

    #[test]
    fn an_unchanged_percent_in_the_same_stage_is_not_reported() {
        let last = (42u8, "pulling".to_owned());
        let same = PullProgress {
            downloaded_bytes: 42,
            total_bytes: 100,
            percent: 42,
            stage: "pulling".to_owned(),
        };
        assert!(!worth_reporting(Some(&last), &same));
    }

    #[test]
    fn a_new_stage_is_reported_even_at_the_same_percent() {
        let last = (100u8, "pulling".to_owned());
        let verifying = PullProgress {
            downloaded_bytes: 0,
            total_bytes: 0,
            percent: 100,
            stage: "verifying sha256 digest".to_owned(),
        };
        assert!(worth_reporting(Some(&last), &verifying));
    }

    #[test]
    fn nonsense_is_still_not_progress() {
        assert!(parse_pull_line("").is_none());
        assert!(parse_pull_line("not json").is_none());
    }

    #[test]
    fn a_zero_total_never_reports_a_percentage() {
        let closing =
            parse_pull_line(r#"{"status":"success","total":0,"completed":0}"#).expect("a stage");
        assert_eq!(closing.total_bytes, 0);
        assert_eq!(closing.stage, "success");
    }

    #[test]
    fn a_line_before_any_bytes_have_arrived_reads_as_zero() {
        let progress =
            parse_pull_line(r#"{"status":"pulling","total":1000}"#).expect("has a total");
        assert_eq!(progress.percent, 0);
    }

    #[test]
    fn an_error_reported_in_the_stream_is_not_treated_as_progress() {
        assert!(parse_pull_line(r#"{"error":"model not found"}"#).is_none());
    }

    #[test]
    fn a_model_wholly_in_graphics_memory_counts_as_accelerated() {
        let loaded = Running {
            model: "qwen2.5:7b".to_owned(),
            size_bytes: 4_748_056_984,
            vram_bytes: 4_748_056_984,
        };
        assert!(loaded.on_gpu());
    }

    #[test]
    fn a_model_that_only_partly_fits_is_not_called_accelerated() {
        let spilled = Running {
            model: "qwen2.5:32b".to_owned(),
            size_bytes: 20_000_000_000,
            vram_bytes: 8_000_000_000,
        };
        assert!(!spilled.on_gpu());

        let cpu = Running {
            model: "qwen2.5:7b".to_owned(),
            size_bytes: 4_000_000_000,
            vram_bytes: 0,
        };
        assert!(!cpu.on_gpu());
    }

    #[test]
    fn absent_token_counts_default_to_zero() {
        let result = parse(serde_json::json!({ "message": { "content": "ok" } })).expect("parses");
        assert_eq!(result.usage, TokenUsage::default());
    }
}
