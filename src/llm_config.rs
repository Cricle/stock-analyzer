//! LLM provider configuration models.

use serde::{Deserialize, Serialize};

/// Configuration for an LLM provider (base URL, models, pricing, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: String,
    pub display_name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
    pub quick_model: Option<String>,
    pub deep_model: Option<String>,
    pub quick_input_price_per_million: Option<f64>,
    pub quick_output_price_per_million: Option<f64>,
    pub deep_input_price_per_million: Option<f64>,
    pub deep_output_price_per_million: Option<f64>,
    pub enabled: bool,
    pub is_default: bool,
    pub provider_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
