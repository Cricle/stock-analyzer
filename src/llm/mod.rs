use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub mod client;
pub mod parse;
pub mod prompt;
pub mod retry;
pub mod tools;

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

pub mod generated;
pub use generated::*;
pub use prompt::{
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

    /// Create an LLM client from environment variables.
    ///
    /// Supported env vars:
    /// - `LLM_PROVIDER` — "anthropic" or "deepseek" (default: auto-detect from URL)
    /// - `LLM_BASE_URL` or `ANTHROPIC_BASE_URL` — API base URL
    /// - `LLM_API_KEY` or `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN` — API key
    /// - `LLM_MODEL` or `ANTHROPIC_MODEL` — model name
    /// - `LLM_TIMEOUT_SECS` or `API_TIMEOUT_MS` — timeout (default: 600s)
    ///
    /// Returns `None` if required env vars are missing.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("LLM_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
            .ok()?;
        let api_key = std::env::var("LLM_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
            .ok()?;
        let model = std::env::var("LLM_MODEL")
            .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        let timeout_secs = std::env::var("LLM_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                std::env::var("API_TIMEOUT_MS")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|ms| ms / 1000)
            })
            .unwrap_or(600);

        let provider = std::env::var("LLM_PROVIDER").ok();
        let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();

        let client = match provider.as_deref() {
            Some("deepseek") => {
                Self::openai_compatible(http, &base_url, &api_key, &model, timeout_secs)
            }
            _ if base_url.contains("anthropic") || base_url.contains("claude") => {
                Self::anthropic(http, &base_url, &api_key, &model, timeout_secs)
            }
            _ => Self::openai_compatible(http, &base_url, &api_key, &model, timeout_secs),
        };
        Some(client)
    }

    pub async fn usage_summary(&self) -> crate::LlmTokenUsageSummary {
        let tracker = self
            .usage_tracker
            .lock()
            .expect("usage tracker mutex poisoned")
            .clone();
        crate::LlmTokenUsageSummary {
            total_requests: tracker.total_requests,
            prompt_tokens: tracker.prompt_tokens,
            completion_tokens: tracker.completion_tokens,
            total_tokens: tracker.total_tokens,
            by_model: tracker
                .by_model
                .into_iter()
                .map(|(model, item)| crate::LlmTokenUsageByModel {
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
