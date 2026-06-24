use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop};

mod client;
pub mod parse;
mod prompt;
pub mod retry;

/// Configuration for the LLM client.
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
pub struct LlmConfig {
    #[zeroize(skip)]
    pub base_url: String,
    pub api_key: String,
    #[zeroize(skip)]
    pub model: String,
    #[zeroize(skip)]
    pub timeout_secs: u64,
    #[zeroize(skip)]
    pub provider_type: String,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct LlmClient {
    #[zeroize(skip)]
    pub http: reqwest_middleware::ClientWithMiddleware,
    #[zeroize(skip)]
    pub openai_base_url: String,
    pub openai_api_key: String,
    #[zeroize(skip)]
    pub model: String,
    #[zeroize(skip)]
    pub timeout: std::time::Duration,
    #[zeroize(skip)]
    pub usage_tracker: Arc<Mutex<LlmUsageAccumulator>>,
    #[zeroize(skip)]
    pub provider_type: String,
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
pub(crate) use prompt::{
    AnalystDecisionParams, DebateTurnParams, PortfolioDecisionParams, ReflectionParams,
    ResearchManagerParams, TraderDecisionParams,
};

impl LlmClient {
    /// Create a new LLM client with the given HTTP client and config.
    pub fn new(http: reqwest_middleware::ClientWithMiddleware, config: &LlmConfig) -> Self {
        Self {
            http,
            openai_base_url: config.base_url.trim_end_matches('/').to_string(),
            openai_api_key: config.api_key.clone(),
            model: config.model.clone(),
            timeout: std::time::Duration::from_secs(config.timeout_secs),
            usage_tracker: Arc::new(Mutex::new(LlmUsageAccumulator::default())),
            provider_type: config.provider_type.clone(),
        }
    }

    pub fn openai_compatible(
        http: reqwest_middleware::ClientWithMiddleware,
        base_url: &str,
        api_key: &str,
        model: &str,
        timeout_secs: u64,
    ) -> Self {
        Self {
            http,
            openai_base_url: base_url.trim_end_matches('/').to_string(),
            openai_api_key: api_key.to_string(),
            model: model.to_string(),
            timeout: std::time::Duration::from_secs(timeout_secs),
            usage_tracker: Arc::new(Mutex::new(LlmUsageAccumulator::default())),
            provider_type: "openai".to_string(),
        }
    }

    pub fn anthropic(
        http: reqwest_middleware::ClientWithMiddleware,
        base_url: &str,
        api_key: &str,
        model: &str,
        timeout_secs: u64,
    ) -> Self {
        Self {
            http,
            openai_base_url: base_url.trim_end_matches('/').to_string(),
            openai_api_key: api_key.to_string(),
            model: model.to_string(),
            timeout: std::time::Duration::from_secs(timeout_secs),
            usage_tracker: Arc::new(Mutex::new(LlmUsageAccumulator::default())),
            provider_type: "anthropic".to_string(),
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
            next.openai_base_url = base_url.trim_end_matches('/').to_string();
        }
        next
    }

    pub fn with_api_key(&self, api_key: Option<&str>) -> Self {
        let mut next = self.clone();
        if let Some(api_key) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
            next.openai_api_key = api_key.to_string();
        }
        next
    }

    pub fn from_provider_config(
        http: reqwest_middleware::ClientWithMiddleware,
        provider: &crate::models::LlmProviderConfig,
        timeout_secs: u64,
    ) -> Option<Self> {
        let api_key = provider
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        let provider_type = provider.provider_type.as_deref().unwrap_or("openai");
        match provider_type {
            "anthropic" => Some(Self::anthropic(
                http,
                &provider.base_url,
                api_key,
                &provider.default_model,
                timeout_secs,
            )),
            _ => Some(Self::openai_compatible(
                http,
                &provider.base_url,
                api_key,
                &provider.default_model,
                timeout_secs,
            )),
        }
    }

    #[tracing::instrument(skip_all, fields(model = %self.model, provider = %self.provider_type, prompt_len = prompt.len()))]
    pub async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        match self.provider_type.as_str() {
            "anthropic" => self.generate_with_anthropic(prompt).await,
            _ => self.generate_with_openai_compatible(prompt).await,
        }
    }

    pub async fn healthcheck(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        match self.provider_type.as_str() {
            "anthropic" => self.healthcheck_anthropic(base_url, api_key, model).await,
            _ => {
                self.healthcheck_openai_compatible(base_url, api_key, model)
                    .await
            }
        }
    }

    pub async fn usage_summary(&self) -> crate::models::LlmTokenUsageSummary {
        let tracker = self
            .usage_tracker
            .lock()
            .expect("usage tracker mutex poisoned")
            .clone();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> LlmConfig {
        LlmConfig {
            base_url: "https://api.example.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            timeout_secs: 60,
            provider_type: "openai".to_string(),
        }
    }

    #[test]
    fn test_llm_config_clone() {
        let config = make_config();
        let cloned = config.clone();
        assert_eq!(cloned.base_url, config.base_url);
        assert_eq!(cloned.api_key, config.api_key);
        assert_eq!(cloned.model, config.model);
    }

    #[test]
    fn test_llm_usage_accumulator_default() {
        let acc = LlmUsageAccumulator::default();
        assert_eq!(acc.total_requests, 0);
        assert_eq!(acc.prompt_tokens, 0);
        assert_eq!(acc.completion_tokens, 0);
        assert_eq!(acc.total_tokens, 0);
        assert!(acc.by_model.is_empty());
    }

    #[test]
    fn test_llm_usage_accumulator_serialization() {
        let mut acc = LlmUsageAccumulator::default();
        acc.total_requests = 10;
        acc.prompt_tokens = 1000;
        acc.by_model.insert(
            "gpt-4".to_string(),
            LlmUsageModelAccumulator {
                requests: 5,
                prompt_tokens: 500,
                completion_tokens: 200,
                total_tokens: 700,
            },
        );
        let json = serde_json::to_string(&acc).unwrap();
        let deserialized: LlmUsageAccumulator = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_requests, 10);
        assert_eq!(deserialized.by_model.len(), 1);
    }

    #[test]
    fn test_llm_usage_model_accumulator_default() {
        let acc = LlmUsageModelAccumulator::default();
        assert_eq!(acc.requests, 0);
        assert_eq!(acc.prompt_tokens, 0);
    }

    #[test]
    fn test_with_model_override() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "key",
            "gpt-4",
            60,
        );
        let updated = client.with_model(Some("claude-3"));
        assert_eq!(updated.model, "claude-3");
        assert_eq!(client.model, "gpt-4"); // original unchanged
    }

    #[test]
    fn test_with_model_none_keeps_original() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "key",
            "gpt-4",
            60,
        );
        let updated = client.with_model(None);
        assert_eq!(updated.model, "gpt-4");
    }

    #[test]
    fn test_with_model_empty_string_keeps_original() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "key",
            "gpt-4",
            60,
        );
        let updated = client.with_model(Some("  "));
        assert_eq!(updated.model, "gpt-4");
    }

    #[test]
    fn test_with_base_url_override() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com/",
            "key",
            "gpt-4",
            60,
        );
        let updated = client.with_base_url(Some("https://other.com/v2"));
        assert_eq!(updated.openai_base_url, "https://other.com/v2");
        // trailing slash stripped
        assert_eq!(client.openai_base_url, "https://api.example.com");
    }

    #[test]
    fn test_with_api_key_override() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "old-key",
            "gpt-4",
            60,
        );
        let updated = client.with_api_key(Some("new-key"));
        assert_eq!(updated.openai_api_key, "new-key");
        assert_eq!(client.openai_api_key, "old-key");
    }

    #[test]
    fn test_openai_compatible_constructor() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com/",
            "test-key",
            "gpt-4",
            30,
        );
        assert_eq!(client.model, "gpt-4");
        assert_eq!(client.provider_type, "openai");
        assert_eq!(client.timeout, std::time::Duration::from_secs(30));
        assert_eq!(client.openai_api_key, "test-key");
        // trailing slash stripped
        assert_eq!(client.openai_base_url, "https://api.example.com");
    }

    #[test]
    fn test_anthropic_constructor() {
        let client = LlmClient::anthropic(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.anthropic.com/",
            "test-key",
            "claude-3",
            120,
        );
        assert_eq!(client.model, "claude-3");
        assert_eq!(client.provider_type, "anthropic");
        assert_eq!(client.timeout, std::time::Duration::from_secs(120));
    }

    #[test]
    fn test_usage_summary_default() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "key",
            "gpt-4",
            60,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        let summary = rt.block_on(client.usage_summary());
        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.prompt_tokens, 0);
        assert!(summary.by_model.is_empty());
    }

    #[test]
    fn test_usage_tracker_shared() {
        let client = LlmClient::openai_compatible(
            reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            "https://api.example.com",
            "key",
            "gpt-4",
            60,
        );
        {
            let mut tracker = client.usage_tracker.lock().unwrap();
            tracker.total_requests = 5;
            tracker.prompt_tokens = 100;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        let summary = rt.block_on(client.usage_summary());
        assert_eq!(summary.total_requests, 5);
        assert_eq!(summary.prompt_tokens, 100);
    }
}

pub(crate) use client::ChatMessageResponse;
pub(crate) use client::is_retryable_llm_error;

pub(crate) use parse::{parse_generated_debate_turn, parse_generated_research_manager};
