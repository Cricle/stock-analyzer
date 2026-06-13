//! Shared initialization helpers for CLI and MCP binaries.

use std::sync::Arc;

use crate::data::MarketDataClient;
use crate::engine::llm::LlmClient;
use crate::engine::guidance::{GuidanceMemory, GuidanceMemoryBundle};

/// No-op memory implementation for standalone CLI/MCP usage.
pub struct NoopMemory;

#[async_trait::async_trait]
impl GuidanceMemory for NoopMemory {
    async fn past_context_bundle(
        &self,
        _query: &str,
        _same_ticker_limit: usize,
        _cross_ticker_limit: usize,
    ) -> GuidanceMemoryBundle {
        GuidanceMemoryBundle::default()
    }
}

/// Build a MarketDataClient from environment.
pub async fn build_market_data_client() -> anyhow::Result<MarketDataClient> {
    Ok(MarketDataClient::new().await)
}

/// Build an LlmClient from environment variables:
/// - LLM_BASE_URL (required)
/// - LLM_API_KEY (required)
/// - LLM_MODEL (default: "claude-sonnet-4-20250514")
/// - LLM_PROVIDER (default: "openai", also supports "anthropic")
/// - LLM_TIMEOUT_SECS (default: 120)
pub fn build_llm_client() -> anyhow::Result<LlmClient> {
    let base_url = std::env::var("LLM_BASE_URL")
        .map_err(|_| anyhow::anyhow!("LLM_BASE_URL not set"))?;
    let api_key = std::env::var("LLM_API_KEY")
        .map_err(|_| anyhow::anyhow!("LLM_API_KEY not set"))?;
    let model = std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    let provider = std::env::var("LLM_PROVIDER")
        .unwrap_or_else(|_| "openai".to_string());
    let timeout_secs: u64 = std::env::var("LLM_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    match provider.as_str() {
        "anthropic" => Ok(LlmClient::anthropic(
            &base_url,
            &api_key,
            &model,
            timeout_secs,
        )),
        _ => Ok(LlmClient::openai_compatible(
            &base_url,
            &api_key,
            &model,
            timeout_secs,
        )),
    }
}

/// Build a no-op memory for standalone usage.
pub fn build_memory() -> Arc<dyn GuidanceMemory> {
    Arc::new(NoopMemory)
}
