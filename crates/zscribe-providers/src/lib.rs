#![forbid(unsafe_code)]

pub mod catalogue;
pub mod error;
pub mod gemini;
pub mod http;
pub mod ollama;
pub mod openai;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use zscribe_core::{chat::Role, Prompt, ProviderId, ProviderProfile, TokenUsage, Turn};

pub use error::{ProviderError, ProviderErrorInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    pub id: String,

    pub label: String,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenerationParams {
    pub temperature: f32,
    pub max_output_tokens: u32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: 0.2,

            max_output_tokens: 8_192,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: Prompt,
    pub params: GenerationParams,

    pub history: Vec<Turn>,
}

impl CompletionRequest {
    pub fn new(model: impl Into<String>, prompt: Prompt) -> Self {
        Self {
            model: model.into(),
            prompt,
            params: GenerationParams::default(),
            history: Vec::new(),
        }
    }

    pub fn with_history(mut self, history: Vec<Turn>) -> Self {
        self.history = history;
        self
    }
}

pub(crate) fn history_messages(history: &[Turn]) -> Vec<serde_json::Value> {
    history
        .iter()
        .map(|turn| {
            serde_json::json!({
                "role": match turn.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                "content": turn.content,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    pub usage: TokenUsage,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    async fn complete(
        &self,
        request: &CompletionRequest,
        cancel: &CancellationToken,
    ) -> Result<Completion, ProviderError>;
}

pub fn build(
    profile: &ProviderProfile,
    api_key: Option<String>,
) -> Result<Box<dyn Provider>, ProviderError> {
    let key = match (profile.id.needs_api_key(), api_key) {
        (true, Some(key)) if !key.trim().is_empty() => Some(key),
        (true, _) => {
            return Err(ProviderError::NoApiKey {
                provider: profile.id,
            })
        }
        (false, key) => key,
    };

    let base_url = profile.base_url().to_owned();

    Ok(match profile.id {
        ProviderId::Ollama => Box::new(ollama::Ollama::new(base_url)?),
        ProviderId::Gemini => Box::new(gemini::Gemini::new(base_url, key.unwrap_or_default())?),
        ProviderId::OpenAiCompatible => Box::new(openai::OpenAiCompatible::new(
            base_url,
            key.unwrap_or_default(),
        )?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_labels_itself_by_id_when_the_provider_gives_no_name() {
        let model = ModelInfo::new("qwen2.5:7b");
        assert_eq!(model.id, "qwen2.5:7b");
        assert_eq!(model.label, "qwen2.5:7b");
    }

    #[test]
    fn summarising_defaults_to_a_low_temperature() {
        assert!(GenerationParams::default().temperature <= 0.3);
    }

    #[test]
    fn the_output_budget_fits_a_full_meeting_summary() {
        assert!(GenerationParams::default().max_output_tokens >= 2_048);
    }

    #[test]
    fn building_a_keyed_provider_without_a_key_fails_before_any_request() {
        let profile = ProviderProfile::new(ProviderId::Gemini);
        assert!(matches!(
            build(&profile, None),
            Err(ProviderError::NoApiKey { .. })
        ));
    }

    #[test]
    fn a_blank_key_counts_as_no_key() {
        let profile = ProviderProfile::new(ProviderId::Gemini);
        assert!(matches!(
            build(&profile, Some("   ".to_owned())),
            Err(ProviderError::NoApiKey { .. })
        ));
    }

    #[test]
    fn the_local_provider_needs_no_key() {
        let profile = ProviderProfile::new(ProviderId::Ollama);
        let provider = build(&profile, None).expect("ollama builds without a key");
        assert_eq!(provider.id(), ProviderId::Ollama);
    }

    #[test]
    fn each_provider_id_builds_its_own_backend() {
        for id in ProviderId::ALL {
            let profile = ProviderProfile::new(id);
            let provider = build(&profile, Some("test-key".to_owned())).expect("builds");
            assert_eq!(provider.id(), id);
        }
    }
}
