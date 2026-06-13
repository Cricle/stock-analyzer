use anyhow::Context;
use backoff::{Error as BackoffError, future::retry};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;

use super::LlmClient;

// Re-export async-openai types used by the rest of the module.
pub use async_openai::config::OpenAIConfig;
pub use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
};
pub use async_openai::types::chat::ResponseFormat;
pub use async_openai::Client as OpenAIClient;

// ---------------------------------------------------------------------------
// Anthropic thin wrapper (no mature Rust SDK)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

impl AnthropicResponse {
    fn content_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| block.text.as_deref())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl LlmClient {
    pub(crate) async fn generate_with_anthropic(&self, prompt: &str) -> anyhow::Result<String> {
        const MAX_ATTEMPTS: usize = 6;
        let mut attempt = 0usize;
        let backoff = llm_retry_backoff();

        let base_url = self.openai_base_url.trim_end_matches('/');
        let url = if base_url.ends_with("/v1") {
            format!("{}/messages", base_url)
        } else {
            format!("{}/v1/messages", base_url)
        };

        retry(backoff, || {
            attempt += 1;
            let url = url.clone();
            let prompt = prompt.to_string();
            let model = self.model.clone();
            let api_key = self.openai_api_key.clone();
            let http = self.anthropic_http.clone();
            let timeout = self.timeout;
            let tracker = self.usage_tracker.clone();
            async move {
                let request = serde_json::json!({
                    "model": model,
                    "max_tokens": 16384,
                    "system": "You must output valid JSON with no markdown fences.",
                    "messages": [{ "role": "user", "content": prompt }],
                    "temperature": 0.2
                });

                let response = tokio::time::timeout(
                    timeout,
                    http.post(&url)
                        .header("x-api-key", &api_key)
                        .header("anthropic-version", "2023-06-01")
                        .header(CONTENT_TYPE, "application/json")
                        .json(&request)
                        .send(),
                )
                .await
                .context("LLM request timed out")?
                .context("failed to call Anthropic API")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    let err = anyhow::anyhow!("anthropic request failed with {status}: {body}");
                    if is_retryable_llm_error(&err) && attempt < MAX_ATTEMPTS {
                        return Err(BackoffError::transient(err));
                    }
                    return Err(BackoffError::permanent(err));
                }

                let payload: AnthropicResponse = tokio::time::timeout(timeout, response.json())
                    .await
                    .context("Anthropic response decode timed out")?
                    .context("failed to decode Anthropic response")?;

                let content = payload.content_text();
                let input_tokens = payload.usage.input_tokens.unwrap_or(0);
                let output_tokens = payload.usage.output_tokens.unwrap_or(0);
                {
                    let mut t = tracker.lock().expect("usage tracker mutex poisoned");
                    t.total_requests += 1;
                    t.prompt_tokens += input_tokens;
                    t.completion_tokens += output_tokens;
                    t.total_tokens += input_tokens + output_tokens;
                    let entry = t.by_model.entry(model.clone()).or_default();
                    entry.requests += 1;
                    entry.prompt_tokens += input_tokens;
                    entry.completion_tokens += output_tokens;
                    entry.total_tokens += input_tokens + output_tokens;
                }

                if !content.trim().is_empty() {
                    return Ok(content);
                }
                Err(BackoffError::permanent(anyhow::anyhow!(
                    "Anthropic response contained no content"
                )))
            }
        })
        .await
    }

}

// ---------------------------------------------------------------------------
// Shared retry helpers
// ---------------------------------------------------------------------------

fn is_retryable_llm_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("LLM request timed out")
            || text.contains("failed to call OpenAI-compatible LLM endpoint")
            || text.contains("429 Too Many Requests")
            || text.contains("520")
            || text.contains("521")
            || text.contains("522")
            || text.contains("523")
            || text.contains("502 Bad Gateway")
            || text.contains("524")
            || text.contains("525")
            || text.contains("526")
            || text.contains("503 Service Unavailable")
            || text.contains("504 Gateway Timeout")
            || text.contains("Upstream stream ended without a terminal response event")
            || text.contains("connection reset")
            || text.contains("timed out")
    })
}

pub(super) fn llm_retry_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(std::time::Duration::from_millis(750))
        .with_multiplier(2.0)
        .with_randomization_factor(0.15)
        .with_max_interval(std::time::Duration::from_secs(8))
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(120)))
        .build()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_llm_error_detection_only_matches_transient_failures() {
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 502 Bad Gateway: upstream ended"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "LLM request timed out"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "failed to call OpenAI-compatible LLM endpoint"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 524 <unknown status code>: error code: 524"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 520 <unknown status code>: error code: 520"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 522 <unknown status code>: error code: 522"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 526 <unknown status code>: error code: 526"
        )));
        assert!(!is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 400 Bad Request: invalid schema"
        )));
        assert!(!is_retryable_llm_error(&anyhow::anyhow!(
            "LLM response contained no content"
        )));
    }
}
