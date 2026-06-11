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

    pub fn from_provider_config(
        http: reqwest_middleware::ClientWithMiddleware,
        provider: &sa_models::LlmProviderConfig,
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

    pub async fn usage_summary(&self) -> sa_models::LlmTokenUsageSummary {
        let tracker = self.usage_tracker.lock().expect("usage tracker mutex poisoned").clone();
        sa_models::LlmTokenUsageSummary {
            total_requests: tracker.total_requests,
            prompt_tokens: tracker.prompt_tokens,
            completion_tokens: tracker.completion_tokens,
            total_tokens: tracker.total_tokens,
            by_model: tracker
                .by_model
                .into_iter()
                .map(|(model, item)| sa_models::LlmTokenUsageByModel {
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
