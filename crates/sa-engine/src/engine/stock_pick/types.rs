use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::data::FundamentalsSnapshot;
use crate::data::NewsItem;
use crate::engine::llm as llm;
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

#[derive(Debug, Clone)]
pub(crate) struct CandidateContext {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) market: String,
    pub(crate) exchange: String,
    pub(crate) source_score: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FactorBreakdown {
    pub(crate) momentum: f64,
    pub(crate) quality: f64,
    pub(crate) value: f64,
    pub(crate) growth: f64,
    pub(crate) profitability: f64,
    pub(crate) risk: f64,
    pub(crate) event: f64,
    pub(crate) evidence: f64,
    pub(crate) history: f64,
    pub(crate) penalty: f64,
    pub(crate) total: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CandidateEvidenceRecord {
    pub(crate) query: String,
    pub(crate) published_at: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) source: String,
    pub(crate) url: String,
    pub(crate) evidence_type: String,
    pub(crate) sentiment_hint: String,
    pub(crate) hard_negative_flag: bool,
    pub(crate) dedupe_key: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnrichmentData {
    pub(crate) pe_ttm: Option<f64>,
    pub(crate) pb: Option<f64>,
    pub(crate) peg: Option<f64>,
    pub(crate) ps: Option<f64>,
    pub(crate) revenue_yoy: Option<f64>,
    pub(crate) net_profit_yoy: Option<f64>,
    pub(crate) gross_margin: Option<f64>,
    pub(crate) fund_flow_net_ratio: Option<f64>,
    // Chip distribution
    pub(crate) chip_benefit_ratio: Option<f64>,
    pub(crate) chip_avg_cost: Option<f64>,
    pub(crate) chip_concentration_90: Option<f64>,
    // Dividend
    pub(crate) dividend_yield: Option<f64>,
    // Analyst coverage
    pub(crate) analyst_report_count: Option<i64>,
    pub(crate) analyst_buy_ratio: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnrichedCandidate {
    pub(crate) symbol: String,
    pub(crate) name: String,
    pub(crate) market: String,
    pub(crate) exchange: String,
    pub(crate) industry: String,
    pub(crate) price: Option<f64>,
    pub(crate) change_pct: Option<f64>,
    pub(crate) market_cap: Option<f64>,
    pub(crate) theme_key: String,
    pub(crate) fundamentals: Option<FundamentalsSnapshot>,
    pub(crate) enrichment: EnrichmentData,
    pub(crate) news: Vec<NewsItem>,
    pub(crate) evidence_records: Vec<CandidateEvidenceRecord>,
    pub(crate) candles: Vec<crate::data::CandlePoint>,
    pub(crate) technical_snapshot: StockPickTechnicalSnapshot,
    pub(crate) market_snapshot: StockPickMarketSnapshot,
    pub(crate) fundamental_snapshot: StockPickFundamentalSnapshot,
    pub(crate) news_snapshot: StockPickNewsSnapshot,
    pub(crate) history_match_snapshot: StockPickHistoryMatchSnapshot,
    pub(crate) risk_snapshot: StockPickRiskSnapshot,
    pub(crate) data_quality_snapshot: StockPickDataQualitySnapshot,
    pub(crate) factor: FactorBreakdown,
    pub(crate) pass_filter: bool,
    pub(crate) rejected_reasons: Vec<String>,
    pub(crate) description: String,
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
                        rationale: llm::parse::text_or_default(
                            map.get("rationale").cloned(),
                            "",
                        ),
                    })
                    .collect(),
                _ => Vec::new(),
            },
        }
    }
}

pub(crate) fn parse_generated_stock_pick(content: &str) -> anyhow::Result<GeneratedStockPickResponse> {
    let trimmed = content.trim();
    let mut candidates = vec![trimmed.to_string()];
    if let Some(start) = trimmed.find('{')
        && let Some(end) = trimmed.rfind('}')
        && start < end
    {
        candidates.push(trimmed[start..=end].trim().to_string());
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
