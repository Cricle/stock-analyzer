use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::data::FundamentalsSnapshot;
use crate::data::NewsItem;
use crate::guide::I18nText;
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
pub struct GeneratedStockPickItem {
    pub symbol: String,
    pub confidence: Value,
    pub thesis: I18nText,
    pub catalysts: Vec<I18nText>,
    pub risks: Vec<I18nText>,
    pub evidence_points: Vec<String>,
    #[serde(default)]
    pub decision_reason_codes: Vec<String>,
    #[serde(default)]
    pub data_gaps: Vec<String>,
    #[serde(default)]
    pub entry_price: Option<String>,
    #[serde(default)]
    pub entry_rationale: Option<String>,
    #[serde(default)]
    pub stop_loss: Option<String>,
    #[serde(default)]
    pub stop_rationale: Option<String>,
    #[serde(default)]
    pub target_price: Option<String>,
    #[serde(default)]
    pub target_rationale: Option<String>,
    #[serde(default)]
    pub holding_period: Option<String>,
    #[serde(default)]
    pub exit_triggers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OverrideActionKind {
    Remove,
    Raise,
    Lower,
}

impl Default for OverrideActionKind {
    fn default() -> Self {
        Self::Remove
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedOverrideAction {
    pub(crate) symbol: String,
    #[serde(default)]
    pub(crate) action: OverrideActionKind,
    pub(crate) reason_code: String,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgreementLevel {
    Agree,
    Partial,
    Disagree,
}

impl Default for AgreementLevel {
    fn default() -> Self {
        Self::Agree
    }
}

impl std::fmt::Display for AgreementLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agree => write!(f, "agree"),
            Self::Partial => write!(f, "partial"),
            Self::Disagree => write!(f, "disagree"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedStockPickResponse {
    pub(crate) summary: String,
    pub(crate) picks: Vec<GeneratedStockPickItem>,
    pub(crate) rejected_symbols: Vec<String>,
    #[serde(default)]
    pub(crate) agreement_with_system_rank: AgreementLevel,
    #[serde(default)]
    pub(crate) override_actions: Vec<GeneratedOverrideAction>,
}

fn parse_i18n_text(value: &serde_json::Value) -> I18nText {
    match value {
        serde_json::Value::String(s) => I18nText::new(s),
        serde_json::Value::Object(map) => {
            let key = map.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let params = map.get("params")
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            I18nText { key, params }
        }
        _ => I18nText::default(),
    }
}

fn parse_i18n_text_list(value: &serde_json::Value) -> Vec<I18nText> {
    match value {
        serde_json::Value::Array(arr) => arr.iter().map(parse_i18n_text).collect(),
        _ => Vec::new(),
    }
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
                    thesis: parse_i18n_text(map.get("thesis").unwrap_or(&serde_json::Value::Null)),
                    catalysts: parse_i18n_text_list(map.get("catalysts").unwrap_or(&serde_json::Value::Array(vec![]))),
                    risks: parse_i18n_text_list(map.get("risks").unwrap_or(&serde_json::Value::Array(vec![]))),
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
                    entry_price: map
                        .get("entry_price")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    entry_rationale: map
                        .get("entry_rationale")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    stop_loss: map
                        .get("stop_loss")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    stop_rationale: map
                        .get("stop_rationale")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    target_price: map
                        .get("target_price")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    target_rationale: map
                        .get("target_rationale")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    holding_period: map
                        .get("holding_period")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    exit_triggers: llm::parse::string_list_or_default(
                        map.get("exit_triggers").cloned(),
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
            agreement_with_system_rank: field("agreement_with_system_rank")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            override_actions: match field("override_actions") {
                Some(Value::Array(items)) => items
                    .into_iter()
                    .filter_map(|item| serde_json::from_value::<GeneratedOverrideAction>(item).ok())
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
