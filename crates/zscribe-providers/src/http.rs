use crate::error::ProviderError;
use reqwest::{Response, StatusCode};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use zscribe_core::ProviderId;

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

pub const MAX_ATTEMPTS: u32 = 3;

const BASE_BACKOFF: Duration = Duration::from_millis(400);

pub fn client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("ZScribe/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| ProviderError::Network {
            provider: ProviderId::Ollama,
            message: format!("could not create HTTP client: {err}"),
        })
}

pub fn download_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(300))
        .user_agent(concat!("ZScribe/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| ProviderError::Network {
            provider: ProviderId::Ollama,
            message: format!("could not create HTTP client: {err}"),
        })
}

pub fn map_transport_error(provider: ProviderId, err: &reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Network {
            provider,
            message: err.to_string(),
        }
    }
}

pub fn map_status(provider: ProviderId, status: StatusCode, body: &str) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth { provider },
        404 => ProviderError::ModelNotFound {
            model: extract_model_hint(body),
        },
        429 => ProviderError::RateLimited { retry_after: None },

        400 if looks_like_an_auth_failure(body) => ProviderError::Auth { provider },
        status @ 400..=499 => ProviderError::BadRequest {
            provider,
            status,
            message: summarise(body),
        },
        status @ 500..=599 => ProviderError::Server { provider, status },
        status => ProviderError::Malformed(format!("unexpected HTTP status {status}")),
    }
}

pub fn retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

pub async fn error_for_status(
    provider: ProviderId,
    response: Response,
) -> Result<Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let after = retry_after(&response);
    let body = response.text().await.unwrap_or_default();

    Err(match map_status(provider, status, &body) {
        ProviderError::RateLimited { .. } => ProviderError::RateLimited { retry_after: after },
        other => other,
    })
}

pub async fn with_retry<F, Fut, T>(
    cancel: &CancellationToken,
    mut op: F,
) -> Result<T, ProviderError>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let mut attempt = 0;

    loop {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }

        let error = match op(attempt).await {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };

        attempt += 1;
        if attempt >= MAX_ATTEMPTS || !error.is_retryable() {
            return Err(error);
        }

        let delay = backoff_for(attempt, &error);
        tracing::warn!(
            attempt,
            delay_ms = delay.as_millis(),
            error = %error,
            "retrying after transient provider failure"
        );

        tokio::select! {
            _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
            _ = tokio::time::sleep(delay) => {}
        }
    }
}

fn backoff_for(attempt: u32, error: &ProviderError) -> Duration {
    if let ProviderError::RateLimited {
        retry_after: Some(after),
    } = error
    {
        return (*after).min(Duration::from_secs(10));
    }
    BASE_BACKOFF * 3u32.pow(attempt.saturating_sub(1))
}

fn summarise(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "no details provided".to_owned();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(message) = value
            .pointer("/error/message")
            .or_else(|| value.pointer("/message"))
            .or_else(|| value.pointer("/error"))
            .and_then(|v| v.as_str())
        {
            return truncate(message);
        }
    }
    truncate(trimmed)
}

fn truncate(text: &str) -> String {
    const LIMIT: usize = 300;
    if text.chars().count() <= LIMIT {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(LIMIT).collect();
    out.push('…');
    out
}

fn looks_like_an_auth_failure(body: &str) -> bool {
    let lowered = body.to_ascii_lowercase();
    [
        "api key not valid",
        "api_key_invalid",
        "invalid api key",
        "unauthenticated",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn extract_model_hint(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = value
        .pointer("/error/message")
        .or_else(|| value.pointer("/message"))?
        .as_str()?;

    if let Some(start) = message.find(['\'', '"']) {
        let rest = &message[start + 1..];
        if let Some(end) = rest.find(['\'', '"']) {
            return Some(rest[..end].to_owned());
        }
    }

    let after = message.split("models/").nth(1)?;
    let name: String = after
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ',' && *c != '.')
        .collect();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn status_of(code: u16, body: &str) -> ProviderError {
        map_status(
            ProviderId::Gemini,
            StatusCode::from_u16(code).unwrap(),
            body,
        )
    }

    #[test]
    fn auth_statuses_map_to_auth() {
        for status in [401, 403] {
            assert_eq!(status_of(status, "").code(), "auth", "HTTP {status}");
        }
    }

    #[test]
    fn server_statuses_map_to_server_and_are_retryable() {
        for status in [500, 502, 503] {
            let error = status_of(status, "");
            assert_eq!(error.code(), "server", "HTTP {status}");
            assert!(error.is_retryable());
        }
    }

    #[test]
    fn too_many_requests_maps_to_rate_limited() {
        assert_eq!(status_of(429, "").code(), "rate_limited");
    }

    #[test]
    fn a_bad_request_carries_the_providers_own_explanation() {
        let body = r#"{"error":{"message":"temperature must be <= 2"}}"#;
        assert!(status_of(400, body)
            .to_string()
            .contains("temperature must be <= 2"));
    }

    #[test]
    fn not_found_names_a_quoted_model() {
        let body = r#"{"error":{"message":"model 'qwen9:70b' was not found"}}"#;
        assert!(status_of(404, body).to_string().contains("qwen9:70b"));
    }

    #[test]
    fn not_found_names_an_unquoted_gemini_model() {
        let body = r#"{"error":{"message":"models/gemini-9-does-not-exist is not found for API version v1beta."}}"#;
        let error = status_of(404, body);
        assert!(
            error.to_string().contains("gemini-9-does-not-exist"),
            "got: {error}"
        );
    }

    #[test]
    fn not_found_without_a_parseable_body_still_reads_correctly() {
        let error = status_of(404, "<html>404</html>");
        assert_eq!(error.code(), "model_not_found");
        assert_eq!(error.to_string(), "the selected model is not available");
        assert!(!error.to_string().contains('\''), "no empty quotes");
    }

    #[test]
    fn a_bad_key_reported_as_400_is_still_an_auth_failure() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#;
        let error = status_of(400, body);

        assert_eq!(error.code(), "auth", "got: {error}");
        assert!(error.remedy().contains("aistudio.google.com"));
    }

    #[test]
    fn an_ordinary_400_is_still_a_bad_request() {
        let body = r#"{"error":{"message":"temperature must be <= 2"}}"#;
        assert_eq!(status_of(400, body).code(), "bad_request");
    }

    #[test]
    fn an_error_body_of_pure_html_does_not_end_up_in_the_message_verbatim() {
        let body = "x".repeat(5_000);
        assert!(status_of(400, &body).to_string().chars().count() < 500);
    }

    #[test]
    fn an_empty_error_body_still_produces_a_readable_message() {
        assert!(status_of(400, "").to_string().contains("no details"));
    }

    #[tokio::test]
    async fn a_successful_call_is_not_retried() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);

        let result: Result<u32, ProviderError> = with_retry(&CancellationToken::new(), move |_| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(7)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn transient_failures_are_retried_up_to_the_limit() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);

        let result: Result<u32, ProviderError> = with_retry(&CancellationToken::new(), move |_| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(ProviderError::Timeout)
            }
        })
        .await;

        assert!(matches!(result, Err(ProviderError::Timeout)));
        assert_eq!(calls.load(Ordering::SeqCst), MAX_ATTEMPTS);
    }

    #[tokio::test(start_paused = true)]
    async fn a_retry_can_succeed() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);

        let result: Result<u32, ProviderError> = with_retry(&CancellationToken::new(), move |_| {
            let counter = Arc::clone(&counter);
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(ProviderError::Timeout)
                } else {
                    Ok(1)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn permanent_failures_are_not_retried() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);

        let result: Result<u32, ProviderError> = with_retry(&CancellationToken::new(), move |_| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(ProviderError::Auth {
                    provider: ProviderId::Gemini,
                })
            }
        })
        .await;

        assert!(matches!(result, Err(ProviderError::Auth { .. })));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "auth failures must not retry"
        );
    }

    #[tokio::test]
    async fn cancellation_before_the_first_attempt_short_circuits() {
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result: Result<u32, ProviderError> = with_retry(&cancel, |_| async { Ok(1) }).await;
        assert!(matches!(result, Err(ProviderError::Cancelled)));
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_during_backoff_abandons_the_wait() {
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();

        let result: Result<u32, ProviderError> = with_retry(&cancel, move |attempt| {
            let trigger = trigger.clone();
            async move {
                if attempt == 0 {
                    trigger.cancel();
                }
                Err(ProviderError::Timeout)
            }
        })
        .await;

        assert!(matches!(result, Err(ProviderError::Cancelled)));
    }

    #[test]
    fn backoff_grows_and_respects_retry_after() {
        let transient = ProviderError::Timeout;
        assert!(backoff_for(2, &transient) > backoff_for(1, &transient));

        let limited = ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(3)),
        };
        assert_eq!(backoff_for(1, &limited), Duration::from_secs(3));
    }

    #[test]
    fn an_absurd_retry_after_is_capped() {
        let limited = ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(3600)),
        };
        assert_eq!(backoff_for(1, &limited), Duration::from_secs(10));
    }

    #[test]
    fn the_timeout_leaves_room_for_a_local_model_on_a_long_recording() {
        assert!(REQUEST_TIMEOUT >= Duration::from_secs(120));
    }

    #[test]
    fn downloads_and_requests_use_different_clients() {
        assert!(client().is_ok());
        assert!(download_client().is_ok());
    }
}
