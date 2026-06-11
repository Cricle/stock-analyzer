
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisUserContext {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub position_state: String,
    #[serde(default)]
    pub workflow_intent: String,
    #[serde(default)]
    pub holding_cost: Option<f64>,
    #[serde(default)]
    pub holding_ratio_pct: Option<f64>,
    #[serde(default)]
    pub risk_preference: String,
    #[serde(default)]
    pub investment_horizon: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LlmTokenUsageSummary {
    #[serde(default)]
    pub total_requests: i64,
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
    #[serde(default)]
    pub by_model: Vec<LlmTokenUsageByModel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LlmTokenUsageByModel {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub requests: i64,
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryContextSnapshot {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub retrieval_mode: String,
    #[serde(default)]
    pub embedding_provider: String,
    #[serde(default)]
    pub embedding_failure_reason: Option<String>,
    #[serde(default)]
    pub same_ticker_count: usize,
    #[serde(default)]
    pub cross_ticker_count: usize,
    #[serde(default)]
    pub vector_hit_count: usize,
    #[serde(default)]
    pub effective_top_k: usize,
    #[serde(default)]
    pub market_sample_count: usize,
    #[serde(default)]
    pub used_market_profile: bool,
    #[serde(default)]
    pub setup_tags: Vec<String>,
    #[serde(default)]
    pub resolved_setup_tags: Vec<String>,
    #[serde(default)]
    pub used_setup_filtered_retrieval: bool,
    #[serde(default)]
    pub used_setup_fallback_calibration: bool,
    #[serde(default)]
    pub setup_calibration_sample_count: usize,
    #[serde(default)]
    pub setup_match_count: usize,
    #[serde(default)]
    pub setup_pending_match_count: usize,
    #[serde(default)]
    pub setup_resolved_match_count: usize,
    #[serde(default)]
    pub setup_match_hit_rate: f64,
    #[serde(default)]
    pub setup_match_avg_alpha_return: f64,
    #[serde(default)]
    pub setup_long_match_count: usize,
    #[serde(default)]
    pub setup_short_match_count: usize,
    #[serde(default)]
    pub setup_neutral_match_count: usize,
    #[serde(default)]
    pub historical_same_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    #[serde(default)]
    pub historical_cross_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    #[serde(default)]
    pub context_excerpt: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoricalMemoryHighlight {
    #[serde(default)]
    pub trade_date: String,
    #[serde(default)]
    pub ticker: String,
    #[serde(default)]
    pub rating: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub key_risk: String,
    #[serde(default)]
    pub lesson: String,
    #[serde(default)]
    pub same_ticker: bool,
}
