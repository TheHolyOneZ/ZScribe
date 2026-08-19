use crate::error::ProviderError;
use crate::history_messages;
use crate::http;
use crate::{Completion, CompletionRequest, ModelInfo, Provider};
use async_trait::async_trait;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use zscribe_core::{ProviderId, TokenUsage};

const ID: ProviderId = ProviderId::OpenAiCompatible;

pub struct OpenAiCompatible {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAiCompatible {
    pub fn new(base_url: String, api_key: String) -> Result<Self, ProviderError> {
        Ok(Self {
            client: http::client()?,
            base_url,
            api_key,
        })
    }
}

#[async_trait]
impl Provider for OpenAiCompatible {
    fn id(&self) -> ProviderId {
        ID
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self
            .client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|err| http::map_transport_error(ID, &err))?;

        let body: ModelListResponse = http::error_for_status(ID, response)
            .await?
            .json()
            .await
            .map_err(|err| ProviderError::Malformed(err.to_string()))?;

        let mut models: Vec<ModelInfo> = body
            .data
            .into_iter()
            .map(|m| ModelInfo::new(m.id))
            .collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(models)
    }

    async fn complete(
        &self,
        request: &CompletionRequest,
        cancel: &CancellationToken,
    ) -> Result<Completion, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = serde_json::json!({
            "model": request.model,
            "messages": messages(request),
            "temperature": request.params.temperature,


            "max_tokens": request.params.max_output_tokens,
        });

        http::with_retry(cancel, |_attempt| {
            let request_future = self
                .client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send();

            async move {
                let response = tokio::select! {
                    _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                    result = request_future => result.map_err(|err| http::map_transport_error(ID, &err))?,
                };

                let payload: ChatCompletionResponse = http::error_for_status(ID, response)
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

fn parse_completion(payload: ChatCompletionResponse) -> Result<Completion, ProviderError> {
    let choice = payload
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Malformed("response contained no choices".to_owned()))?;

    match choice.finish_reason.as_deref() {
        Some("length") => return Err(ProviderError::Truncated),
        Some("content_filter") => {
            return Err(ProviderError::Filtered {
                provider: ID,
                reason: Some("content_filter".to_owned()),
            })
        }
        _ => {}
    }

    let text = choice
        .message
        .and_then(|message| message.content)
        .unwrap_or_default();

    if text.is_empty() {
        return Err(ProviderError::Malformed(
            "response contained no text".to_owned(),
        ));
    }

    Ok(Completion {
        text,
        usage: payload
            .usage
            .map(|usage| TokenUsage {
                input: usage.prompt_tokens,
                output: usage.completion_tokens,
            })
            .unwrap_or_default(),
    })
}

#[derive(Debug, Deserialize)]
struct ModelListResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
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

        assert_eq!(rendered.len(), 4);
        assert_eq!(rendered[0]["role"], "system");
        assert_eq!(rendered[2]["role"], "assistant");
        assert_eq!(rendered[3]["content"], "and who was that?");
    }

    #[test]
    fn extracts_the_text_and_the_token_counts() {
        let result = parse(serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": "## Decisions\n\n- Ship it" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 1840, "completion_tokens": 210 }
        }))
        .expect("parses");

        assert_eq!(result.text, "## Decisions\n\n- Ship it");
        assert_eq!(result.usage.input, 1840);
        assert_eq!(result.usage.output, 210);
    }

    #[test]
    fn hitting_the_output_limit_is_an_error_not_a_short_summary() {
        let result = parse(serde_json::json!({
            "choices": [{ "message": { "content": "## Deci" }, "finish_reason": "length" }]
        }));
        assert!(matches!(result, Err(ProviderError::Truncated)));
    }

    #[test]
    fn a_content_filter_stop_is_reported_as_filtered() {
        let result = parse(serde_json::json!({
            "choices": [{ "message": { "content": "" }, "finish_reason": "content_filter" }]
        }));
        assert!(matches!(result, Err(ProviderError::Filtered { .. })));
    }

    #[test]
    fn no_choices_is_malformed() {
        assert!(matches!(
            parse(serde_json::json!({ "choices": [] })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn a_null_content_is_malformed_rather_than_an_empty_summary() {
        assert!(matches!(
            parse(serde_json::json!({
                "choices": [{ "message": { "content": null }, "finish_reason": "stop" }]
            })),
            Err(ProviderError::Malformed(_))
        ));
    }

    #[test]
    fn a_server_that_reports_no_usage_is_not_an_error() {
        let result = parse(serde_json::json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }]
        }))
        .expect("parses");
        assert_eq!(result.usage, TokenUsage::default());
    }
}
