use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::data::FundamentalsSnapshot;
use crate::data::NewsItem;
use crate::llm;
use crate::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

#[derive(Debug, Clone)]
pub struct CandidateContext {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub source_score: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FactorBreakdown {
    pub momentum: f64,
    pub quality: f64,
    pub value: f64,
    pub profitability: f64,
    pub risk: f64,
    pub event: f64,
    pub evidence: f64,
    pub history: f64,
    pub penalty: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateEvidenceRecord {
    pub query: String,
    pub published_at: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub evidence_type: String,
    pub sentiment_hint: String,
    pub hard_negative_flag: bool,
    pub dedupe_key: String,
}

#[derive(Debug, Clone)]
pub struct EnrichedCandidate {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    pub industry: String,
    pub price: Option<f64>,
    pub change_pct: Option<f64>,
    pub market_cap: Option<f64>,
    pub theme_key: String,
    pub fundamentals: Option<FundamentalsSnapshot>,
    pub news: Vec<NewsItem>,
    pub evidence_records: Vec<CandidateEvidenceRecord>,
    pub candles: Vec<crate::data::CandlePoint>,
    pub technical_snapshot: StockPickTechnicalSnapshot,
    pub market_snapshot: StockPickMarketSnapshot,
    pub fundamental_snapshot: StockPickFundamentalSnapshot,
    pub news_snapshot: StockPickNewsSnapshot,
    pub history_match_snapshot: StockPickHistoryMatchSnapshot,
    pub risk_snapshot: StockPickRiskSnapshot,
    pub data_quality_snapshot: StockPickDataQualitySnapshot,
    pub factor: FactorBreakdown,
    pub pass_filter: bool,
    pub rejected_reasons: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedStockPickItem {
    pub(crate) symbol: String,
    pub(crate) confidence: Value,
    pub(crate) thesis: String,
    pub(crate) catalysts: Vec<String>,
    pub(crate) risks: Vec<String>,
    pub(crate) evidence_points: Vec<String>,
    #[serde(default)]
    pub(crate) decision_reason_codes: Vec<String>,
    #[serde(default)]
    pub(crate) data_gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedOverrideAction {
    pub(crate) symbol: String,
    pub(crate) action: String,
    pub(crate) reason_code: String,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedStockPickResponse {
    pub(crate) summary: String,
    pub(crate) picks: Vec<GeneratedStockPickItem>,
    pub(crate) rejected_symbols: Vec<String>,
    pub(crate) agreement_with_system_rank: String,
    #[serde(default)]
    pub(crate) override_actions: Vec<GeneratedOverrideAction>,
}

impl GeneratedStockPickResponse {
    fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();

        let picks = match field("picks") {
            Some(Value::Array(items)) => items
                .into_iter()
                .filter_map(|item| item.as_object().cloned())
                .map(|map| GeneratedStockPickItem {
                    symbol: llm::parse::text_or_default(map.get("symbol").cloned(), "UNKNOWN"),
                    confidence: map.get("confidence").cloned().unwrap_or(Value::from(0.0)),
                    thesis: llm::parse::text_or_default(
                        map.get("thesis").cloned(),
                        "No thesis returned.",
                    ),
                    catalysts: llm::parse::string_list_or_default(
                        map.get("catalysts").cloned(),
                        &["No catalyst returned"],
                    ),
                    risks: llm::parse::string_list_or_default(
                        map.get("risks").cloned(),
                        &["No risk returned"],
                    ),
                    evidence_points: llm::parse::string_list_or_default(
                        map.get("evidence_points").cloned(),
                        &["No evidence returned"],
                    ),
                    decision_reason_codes: llm::parse::string_list_or_default(
                        map.get("decision_reason_codes").cloned(),
                        &[],
                    ),
                    data_gaps: llm::parse::string_list_or_default(
                        map.get("data_gaps").cloned(),
                        &[],
                    ),
                })
                .collect(),
            _ => Vec::new(),
        };

        Self {
            summary: llm::parse::text_or_default(
                field("summary"),
                "No stock picking summary returned.",
            ),
            picks,
            rejected_symbols: llm::parse::string_list_or_default(field("rejected_symbols"), &[]),
            agreement_with_system_rank: llm::parse::text_or_default(
                field("agreement_with_system_rank"),
                "agree",
            ),
            override_actions: match field("override_actions") {
                Some(Value::Array(items)) => items
                    .into_iter()
                    .filter_map(|item| item.as_object().cloned())
                    .map(|map| GeneratedOverrideAction {
                        symbol: llm::parse::text_or_default(map.get("symbol").cloned(), "UNKNOWN"),
                        action: llm::parse::text_or_default(map.get("action").cloned(), ""),
                        reason_code: llm::parse::text_or_default(
                            map.get("reason_code").cloned(),
                            "",
                        ),
                        rationale: llm::parse::text_or_default(map.get("rationale").cloned(), ""),
                    })
                    .collect(),
                _ => Vec::new(),
            },
        }
    }
}

pub(crate) fn parse_generated_stock_pick(
    content: &str,
) -> anyhow::Result<GeneratedStockPickResponse> {
    let trimmed = content.trim();
    let mut candidates = vec![trimmed.to_string()];
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if start < end {
                candidates.push(trimmed[start..=end].trim().to_string());
            }
        }
    }

    let mut last_error = None;
    for candidate in candidates {
        match serde_json::from_str::<Value>(&candidate) {
            Ok(value) => return Ok(GeneratedStockPickResponse::from_value(value)),
            Err(error) => last_error = Some(error),
        }
    }

    Err(anyhow::anyhow!(
        "{}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown JSON parsing error".to_string())
    ))
}
