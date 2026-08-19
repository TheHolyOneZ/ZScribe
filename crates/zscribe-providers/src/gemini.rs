use crate::error::ProviderError;
use crate::http;
use crate::{Completion, CompletionRequest, ModelInfo, Provider};
use async_trait::async_trait;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use zscribe_core::{ProviderId, TokenUsage};

const ID: ProviderId = ProviderId::Gemini;

pub struct Gemini {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl Gemini {
    pub fn new(base_url: String, api_key: String) -> Result<Self, ProviderError> {
        Ok(Self {
            client: http::client()?,
            base_url,
            api_key,
        })
    }
}

#[async_trait]
impl Provider for Gemini {
    fn id(&self) -> ProviderId {
        ID
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let url = format!("{}/models", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        let body: ModelListResponse = http::error_for_status(ID, response)
            .await?
            .json()
            .await
            .map_err(|err| ProviderError::Malformed(err.to_string()))?;

        Ok(body
            .models
            .into_iter()
            .filter(|model| {
                model
                    .supported_generation_methods
                    .iter()
                    .any(|method| method == "generateContent")
            })
            .map(|model| ModelInfo {
                id: model.short_name().to_owned(),
                label: model
                    .display_name
                    .clone()
                    .unwrap_or_else(|| model.short_name().to_owned()),
            })
            .collect())
    }

    async fn complete(
        &self,
        request: &CompletionRequest,
        cancel: &CancellationToken,
    ) -> Result<Completion, ProviderError> {
        let url = format!("{}/models/{}:generateContent", self.base_url, request.model);

        let body = serde_json::json!({
            "systemInstruction": { "parts": [{ "text": request.prompt.system }] },
            "contents": contents(request),
            "generationConfig": {
                "temperature": request.params.temperature,
                "maxOutputTokens": request.params.max_output_tokens,
            },
        });

        http::with_retry(cancel, |_attempt| {
            let request_future = self
                .client
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&body)
                .send();

            async move {
                let response = tokio::select! {
                    _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                    result = request_future => result.map_err(|err| http::map_transport_error(ID, &err))?,
                };

                let payload: GenerateContentResponse = http::error_for_status(ID, response)
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

fn contents(request: &CompletionRequest) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = request
        .history
        .iter()
        .map(|turn| {
            serde_json::json!({
                "role": match turn.role {
                    zscribe_core::Role::User => "user",
                    zscribe_core::Role::Assistant => "model",
                },
                "parts": [{ "text": turn.content }],
            })
        })
        .collect();

    out.push(serde_json::json!({
        "role": "user",
        "parts": [{ "text": request.prompt.user }],
    }));
    out
}

fn parse_completion(payload: GenerateContentResponse) -> Result<Completion, ProviderError> {
    if let Some(feedback) = &payload.prompt_feedback {
        if let Some(reason) = &feedback.block_reason {
            return Err(ProviderError::Filtered {
                provider: ID,
                reason: Some(reason.clone()),
            });
        }
    }

    let candidate =
        payload.candidates.into_iter().next().ok_or_else(|| {
            ProviderError::Malformed("response contained no candidates".to_owned())
        })?;

    match candidate.finish_reason.as_deref() {
        Some("MAX_TOKENS") => return Err(ProviderError::Truncated),
        Some("SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST") => {
            return Err(ProviderError::Filtered {
                provider: ID,
                reason: candidate.finish_reason,
            })
        }
        _ => {}
    }

    let text: String = candidate
        .content
        .map(|content| {
            content
                .parts
                .into_iter()
                .filter_map(|part| part.text)
                .collect()
        })
        .unwrap_or_default();

    if text.is_empty() {
        return Err(ProviderError::Malformed(
            "response contained no text".to_owned(),
        ));
    }

    Ok(Completion {
        text,
        usage: payload
            .usage_metadata
            .map(|usage| TokenUsage {
                input: usage.prompt_token_count,
                output: usage.candidates_token_count,
            })
            .unwrap_or_default(),
    })
}

#[derive(Debug, Deserialize)]
struct ModelListResponse {
    #[serde(default)]
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModel {
    name: String,
    display_name: Option<String>,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
}

impl GeminiModel {
    fn short_name(&self) -> &str {
        self.name.strip_prefix("models/").unwrap_or(&self.name)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    usage_metadata: Option<UsageMetadata>,
    prompt_feedback: Option<PromptFeedback>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<Content>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
struct Part {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    #[serde(default)]
    prompt_token_count: u32,
    #[serde(default)]
    candidates_token_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptFeedback {
    block_reason: Option<String>,
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
    fn the_model_turn_is_called_model_rather_than_assistant() {
        let rendered = contents(&request_with_history());

        assert_eq!(rendered.len(), 3, "two history turns and the question");
        assert_eq!(rendered[0]["role"], "user");
        assert_eq!(rendered[1]["role"], "model");
        assert_eq!(rendered[2]["parts"][0]["text"], "and who was that?");
    }

    #[test]
    fn summarising_sends_only_the_question() {
        let plain = CompletionRequest::new(
            "m",
            zscribe_core::Prompt {
                system: "s".to_owned(),
                user: "u".to_owned(),
            },
        );
        assert_eq!(contents(&plain).len(), 1);
    }

    #[test]
    fn extracts_the_text_and_the_token_counts() {
        let result = parse(serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": "## Decisions\n\n- Ship it" }] },
                "finishReason": "STOP"
            }],
            "usageMetadata": { "promptTokenCount": 1840, "candidatesTokenCount": 210 }
        }))
        .expect("parses");

        assert_eq!(result.text, "## Decisions\n\n- Ship it");
        assert_eq!(result.usage.input, 1840);
        assert_eq!(result.usage.output, 210);
    }

    #[test]
    fn a_long_answer_split_across_parts_is_rejoined() {
        let result = parse(serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": "## Decisions\n" }, { "text": "\n- Ship it" }] },
                "finishReason": "STOP"
            }]
        }))
        .expect("parses");
        assert_eq!(result.text, "## Decisions\n\n- Ship it");
    }

    #[test]
    fn missing_usage_is_zero_rather_than_an_error() {
        let result = parse(serde_json::json!({
            "candidates": [{ "content": { "parts": [{ "text": "hi" }] } }]
        }))
        .expect("parses");
        assert_eq!(result.usage, TokenUsage::default());
    }

    #[test]
    fn hitting_the_output_limit_is_an_error_not_a_short_summary() {
        let result = parse(serde_json::json!({
            "candidates": [{
                "content": { "parts": [{ "text": "## Decisions\n\n- Shi" }] },
                "finishReason": "MAX_TOKENS"
            }]
        }));
        assert!(matches!(result, Err(ProviderError::Truncated)));
    }

    #[test]
    fn a_safety_finish_reason_is_reported_as_filtered() {
        let result = parse(serde_json::json!({ "candidates": [{ "finishReason": "SAFETY" }] }));
        assert!(matches!(result, Err(ProviderError::Filtered { .. })));
    }

    #[test]
    fn a_prompt_level_block_is_reported_as_filtered() {
        let result = parse(serde_json::json!({
            "candidates": [],
            "promptFeedback": { "blockReason": "SAFETY" }
        }));
        assert!(matches!(result, Err(ProviderError::Filtered { .. })));
    }

    #[test]
    fn no_candidates_is_malformed() {
        assert!(matches!(
            parse(serde_json::json!({ "candidates": [] })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn an_empty_answer_is_malformed() {
        assert!(matches!(
            parse(serde_json::json!({
                "candidates": [{ "content": { "parts": [] }, "finishReason": "STOP" }]
            })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn model_names_lose_their_prefix() {
        let model = GeminiModel {
            name: "models/gemini-2.5-flash".to_owned(),
            display_name: None,
            supported_generation_methods: vec![],
        };
        assert_eq!(model.short_name(), "gemini-2.5-flash");
    }

    #[test]
    fn a_model_without_a_display_name_is_labelled_by_its_short_name() {
        let model = GeminiModel {
            name: "models/gemini-2.5-flash".to_owned(),
            display_name: None,
            supported_generation_methods: vec!["generateContent".to_owned()],
        };
        let label = model
            .display_name
            .clone()
            .unwrap_or_else(|| model.short_name().to_owned());
        assert_eq!(label, "gemini-2.5-flash");
    }
}
