use std::{collections::BTreeMap, sync::Arc};

use backoff::{Error as BackoffError, future::retry};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop};

mod client;
pub mod parse;
mod prompt;
pub mod retry;

use client::{OpenAIClient, OpenAIConfig};

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct LlmClient {
    #[zeroize(skip)]
    openai: OpenAIClient<OpenAIConfig>,
    #[zeroize(skip)]
    anthropic_http: reqwest::Client,
    #[zeroize(skip)]
    pub model: String,
    #[zeroize(skip)]
    pub timeout: std::time::Duration,
    #[zeroize(skip)]
    pub usage_tracker: Arc<Mutex<LlmUsageAccumulator>>,
    #[zeroize(skip)]
    pub provider_type: String,
    #[zeroize(skip)]
    pub openai_base_url: String,
    pub openai_api_key: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmUsageAccumulator {
    pub total_requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub by_model: BTreeMap<String, LlmUsageModelAccumulator>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmUsageModelAccumulator {
    pub requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

mod generated;
pub use generated::*;
pub use prompt::{
    AnalystDecisionParams, DebateTurnParams, PortfolioDecisionParams, ReflectionParams,
    ResearchManagerParams, TraderDecisionParams,
};

impl LlmClient {
    pub fn openai_compatible(
        base_url: &str,
        api_key: &str,
        model: &str,
        timeout_secs: u64,
    ) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let config = OpenAIConfig::new()
            .with_api_base(&base_url)
            .with_api_key(api_key);
        Self {
            openai: OpenAIClient::with_config(config),
            anthropic_http: reqwest::Client::new(),
            model: model.to_string(),
            timeout: std::time::Duration::from_secs(timeout_secs),
            usage_tracker: Arc::new(Mutex::new(LlmUsageAccumulator::default())),
            provider_type: "openai".to_string(),
            openai_base_url: base_url,
            openai_api_key: api_key.to_string(),
        }
    }

    pub fn anthropic(
        base_url: &str,
        api_key: &str,
        model: &str,
        timeout_secs: u64,
    ) -> Self {
        Self {
            openai: OpenAIClient::new(),
            anthropic_http: reqwest::Client::new(),
            model: model.to_string(),
            timeout: std::time::Duration::from_secs(timeout_secs),
            usage_tracker: Arc::new(Mutex::new(LlmUsageAccumulator::default())),
            provider_type: "anthropic".to_string(),
            openai_base_url: base_url.trim_end_matches('/').to_string(),
            openai_api_key: api_key.to_string(),
        }
    }

    pub fn with_model(&self, model: Option<&str>) -> Self {
        let mut next = self.clone();
        if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
            next.model = model.to_string();
        }
        next
    }

    pub fn with_base_url(&self, base_url: Option<&str>) -> Self {
        let mut next = self.clone();
        if let Some(base_url) = base_url.map(str::trim).filter(|value| !value.is_empty()) {
            let base_url = base_url.trim_end_matches('/').to_string();
            let config = OpenAIConfig::new()
                .with_api_base(&base_url)
                .with_api_key(&next.openai_api_key);
            next.openai = OpenAIClient::with_config(config);
            next.openai_base_url = base_url;
        }
        next
    }

    pub fn with_api_key(&self, api_key: Option<&str>) -> Self {
        let mut next = self.clone();
        if let Some(api_key) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
            let config = OpenAIConfig::new()
                .with_api_base(&next.openai_base_url)
                .with_api_key(api_key);
            next.openai = OpenAIClient::with_config(config);
            next.openai_api_key = api_key.to_string();
        }
        next
    }

    #[tracing::instrument(skip_all, fields(model = %self.model, provider = %self.provider_type, prompt_len = prompt.len()))]
    pub async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        match self.provider_type.as_str() {
            "anthropic" => self.generate_with_anthropic(prompt).await,
            _ => self.generate_with_openai(prompt).await,
        }
    }

    pub async fn usage_summary(&self) -> crate::models::LlmTokenUsageSummary {
        let tracker = self.usage_tracker.lock().expect("usage tracker mutex poisoned").clone();
        crate::models::LlmTokenUsageSummary {
            total_requests: tracker.total_requests,
            prompt_tokens: tracker.prompt_tokens,
            completion_tokens: tracker.completion_tokens,
            total_tokens: tracker.total_tokens,
            by_model: tracker
                .by_model
                .into_iter()
                .map(|(model, item)| crate::models::LlmTokenUsageByModel {
                    model,
                    requests: item.requests,
                    prompt_tokens: item.prompt_tokens,
                    completion_tokens: item.completion_tokens,
                    total_tokens: item.total_tokens,
                })
                .collect(),
        }
    }

    // -----------------------------------------------------------------------
    // OpenAI-compatible implementation using async-openai
    // -----------------------------------------------------------------------

    async fn generate_with_openai(&self, prompt: &str) -> anyhow::Result<String> {
        use client::{
            ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
            ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
            ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest, ResponseFormat,
        };

        let request = CreateChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(
                        "You must output valid JSON with no markdown fences.".to_string(),
                    ),
                    name: None,
                }),
                ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
                    name: None,
                }),
            ],
            temperature: Some(0.2),
            response_format: Some(ResponseFormat::JsonObject),
            ..Default::default()
        };

        const MAX_ATTEMPTS: usize = 10;
        let mut attempt = 0usize;
        let backoff = client::llm_retry_backoff();
        let openai = &self.openai;
        let model = &self.model;
        let tracker = &self.usage_tracker;

        retry(backoff, || {
            attempt += 1;
            let request = request.clone();
            let openai = openai.clone();
            let model = model.clone();
            let tracker = tracker.clone();
            async move {
                match openai.chat().create(request).await {
                    Ok(response) => {
                        let content = response
                            .choices
                            .first()
                            .and_then(|c| c.message.content.clone())
                            .unwrap_or_default();
                        let resolved_model = if response.model.is_empty() { model.clone() } else { response.model };
                        if let Some(usage) = &response.usage {
                            let mut t = tracker.lock().expect("usage tracker mutex poisoned");
                            t.total_requests += 1;
                            t.prompt_tokens += usage.prompt_tokens as i64;
                            t.completion_tokens += usage.completion_tokens as i64;
                            t.total_tokens += usage.total_tokens as i64;
                            let entry = t.by_model.entry(resolved_model.to_string()).or_default();
                            entry.requests += 1;
                            entry.prompt_tokens += usage.prompt_tokens as i64;
                            entry.completion_tokens += usage.completion_tokens as i64;
                            entry.total_tokens += usage.total_tokens as i64;
                        }
                        if !content.trim().is_empty() {
                            Ok(content)
                        } else {
                            Err(BackoffError::permanent(anyhow::anyhow!(
                                "LLM response contained no content"
                            )))
                        }
                    }
                    Err(e) => {
                        let err = anyhow::anyhow!("openai request failed: {e}");
                        if is_retryable_openai_error(&e) && attempt < MAX_ATTEMPTS {
                            tracing::warn!(
                                attempt,
                                max_attempts = MAX_ATTEMPTS,
                                error = %err,
                                "retrying transient LLM upstream failure"
                            );
                            Err(BackoffError::transient(err))
                        } else {
                            Err(BackoffError::permanent(err))
                        }
                    }
                }
            }
        })
        .await
    }
}

fn is_retryable_openai_error(error: &async_openai::error::OpenAIError) -> bool {
    let text = error.to_string();
    text.contains("429")
        || text.contains("502")
        || text.contains("503")
        || text.contains("504")
        || text.contains("520")
        || text.contains("521")
        || text.contains("522")
        || text.contains("523")
        || text.contains("524")
        || text.contains("525")
        || text.contains("526")
        || text.contains("timed out")
        || text.contains("connection reset")
}
