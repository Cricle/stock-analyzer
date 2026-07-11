//! Data structures for the daily guidance system.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Localized text with i18n key and parameters.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct I18nText {
    pub key: String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

impl I18nText {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            params: std::collections::HashMap::new(),
        }
    }

    pub fn with_param(
        mut self,
        name: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.params.insert(name.into(), value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }
}

impl Default for I18nText {
    fn default() -> Self {
        Self::new("")
    }
}

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
    pub metadata: GuidanceMetadata,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketSentiment {
    pub score: i32,
    pub label: String,
    pub rationale: String,
    pub drivers: Vec<String>,
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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SectorHighlight {
    pub sector_name: String,
    pub direction: String,
    pub key_driver: String,
    pub representative_stocks: Vec<String>,
}

/// Price level for support/resistance analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub level_type: String,
    pub significance: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockGuidance {
    pub symbol: String,
    pub stock_name: String,
    pub market: String,
    pub current_price: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub guidance_action: I18nText,
    pub confidence: i32,
    pub rationale: I18nText,
    pub key_risks: Vec<I18nText>,
    pub memory_relevance: f64,
    // New actionable fields
    #[serde(default)]
    pub entry_zone: Option<String>,
    #[serde(default)]
    pub resistance_level: Option<String>,
    pub suggested_action: I18nText,
    pub action_rationale: I18nText,
    #[serde(default)]
    pub key_levels: Vec<PriceLevel>,
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
    pub mitigation: String,
    pub affected_markets: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserProfileGuide {
    pub profile: String,
    pub summary: String,
    pub recommended_actions: Vec<String>,
    pub watch_list: Vec<String>,
    pub avoid_list: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GuidanceMetadata {
    pub news_count: usize,
    pub news_sources: Vec<String>,
    pub vector_memory_queries: usize,
    pub vector_memory_hits: usize,
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
