//! Data structures for the daily guidance system.

use serde::{Deserialize, Serialize};

/// Market scope for guidance generation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceMarket {
    AShare,
    HongKong,
    UsEquity,
    All,
}

impl GuidanceMarket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AShare => "a_share",
            Self::HongKong => "hong_kong",
            Self::UsEquity => "us_equity",
            Self::All => "all",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "a_share" | "a-share" | "cn" | "ashare" => Self::AShare,
            "hong_kong" | "hk" | "hongkong" => Self::HongKong,
            "us_equity" | "us" => Self::UsEquity,
            _ => Self::All,
        }
    }
}

/// Structured daily guidance report (JSON only, frontend handles i18n).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DailyGuidanceReport {
    pub report_id: String,
    pub generated_at: String,
    pub date: String,
    pub market: String,
    pub market_sentiment: MarketSentiment,
    pub key_news: Vec<GuidanceNewsItem>,
    pub sector_highlights: Vec<SectorHighlight>,
    pub stock_guidances: Vec<StockGuidance>,
    pub historical_insights: Vec<HistoricalInsight>,
    pub risk_alerts: Vec<RiskAlert>,
    pub user_guides: Vec<UserProfileGuide>,
    pub recent_stock_picks: Option<RecentStockPickSummary>,
    pub market_indices: Vec<MarketIndex>,
    pub executive_summary: String,
    /// i18n key + params for `executive_summary`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executive_summary_key: Option<serde_json::Value>,
    pub metadata: GuidanceMetadata,
    /// LLM token usage for this report generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_token_usage: Option<crate::models::LlmTokenUsageSummary>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketSentiment {
    pub score: i32,
    pub label: String,
    /// i18n key for `label` (e.g. `"guidance.sentiment.bullish"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_key: Option<String>,
    pub rationale: String,
    /// i18n key + params for `rationale`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale_key: Option<serde_json::Value>,
    pub drivers: Vec<String>,
    /// i18n keys for `drivers` (resolved by frontend or resolve_output).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub driver_keys: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GuidanceNewsItem {
    pub title: String,
    pub summary: String,
    pub source: String,
    pub published_at: String,
    pub url: Option<String>,
    pub impact: String,
    pub affected_entities: Vec<String>,
    pub sector: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SectorHighlight {
    pub sector_name: String,
    /// i18n key for `sector_name` (e.g. `"guidance.sector.technology"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector_key: Option<String>,
    pub direction: String,
    /// i18n key for `direction` (e.g. `"guidance.direction.positive"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_key: Option<String>,
    pub key_driver: String,
    pub representative_stocks: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockGuidance {
    pub symbol: String,
    pub stock_name: String,
    pub market: String,
    pub current_price: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub guidance_action: String,
    pub confidence: i32,
    pub rationale: String,
    pub key_risks: Vec<String>,
    pub memory_relevance: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoricalInsight {
    pub pattern_type: String,
    pub description: String,
    pub relevant_tickers: Vec<String>,
    pub confidence: f64,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskAlert {
    pub severity: String,
    pub category: String,
    pub description: String,
    /// i18n key + params for `description`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_key: Option<serde_json::Value>,
    pub mitigation: String,
    /// i18n key for `mitigation`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mitigation_key: Option<String>,
    pub affected_markets: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserProfileGuide {
    pub profile: String,
    /// i18n key for `profile`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_key: Option<String>,
    pub summary: String,
    /// i18n key + params for `summary`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_key: Option<serde_json::Value>,
    pub recommended_actions: Vec<String>,
    /// i18n keys for each action in `recommended_actions`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_action_keys: Option<Vec<String>>,
    /// Original English action strings (fallback when i18n is not available).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_texts: Vec<String>,
    pub watch_list: Vec<String>,
    pub avoid_list: Vec<String>,
    /// i18n key + params for sector info embedded in `summary`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector_info_key: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GuidanceMetadata {
    pub news_count: usize,
    pub news_sources: Vec<String>,
    pub historical_query_count: usize,
    pub historical_hit_count: usize,
    pub cache_hit: bool,
    pub generation_time_ms: u64,
    pub data_freshness: String,
}

/// Request parameters for daily guidance generation.
#[derive(Clone, Debug, Deserialize)]
pub struct DailyGuidanceRequest {
    pub market: Option<String>,
    pub tickers: Option<Vec<String>>,
    pub refresh: Option<bool>,
    /// Language for LLM output (e.g. "zh", "en").
    #[serde(default)]
    pub lang: Option<String>,
}

impl DailyGuidanceRequest {
    pub fn market(&self) -> GuidanceMarket {
        self.market
            .as_deref()
            .map(GuidanceMarket::from_str)
            .unwrap_or(GuidanceMarket::All)
    }
}

/// Major market index snapshot for the guidance overview.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketIndex {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_pct: f64,
    pub market: String,
}

/// Recent stock pick summary for inclusion in guidance reports.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentStockPickSummary {
    pub run_id: String,
    pub analysis_date: String,
    pub market: String,
    pub strategy: String,
    pub picks: Vec<StockPickGuidanceEntry>,
    pub average_score: f64,
    pub average_alpha: Option<f64>,
}

/// Stock pick entry adapted for guidance display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickGuidanceEntry {
    pub symbol: String,
    pub name: String,
    pub score: f64,
    pub confidence: f64,
    pub thesis: String,
    pub current_price: Option<f64>,
    pub alpha_return: Option<f64>,
}
