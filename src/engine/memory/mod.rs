use serde::{Deserialize, Serialize};

use crate::models::HistoricalMemoryHighlight;
use crate::models::{StructuredReflection, StructuredRiskAssessment};

pub(crate) const ENTRY_SEPARATOR: &str = "\n\n<!-- ENTRY_END -->\n\n";
pub(crate) const WEAK_SETUP_TAGS: &[&str] = &["watchlist_only"];

#[derive(Clone)]
pub struct TradingMemoryLog {
    pub store: std::sync::Arc<dyn MemoryStore>,
    pub max_entries: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub ticker: String,
    pub trade_date: String,
    pub rating: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub market: String,
    #[serde(default)]
    pub stock_name: String,
    #[serde(default)]
    pub direction_score: Option<i32>,
    #[serde(default)]
    pub confidence_score: Option<i32>,
    #[serde(default)]
    pub action_score: Option<i32>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub risk_assessment: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub structured_risk: StructuredRiskAssessment,
    #[serde(default)]
    pub structured_reflection: StructuredReflection,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    #[serde(default)]
    pub setup_tags: Vec<String>,
    #[serde(default)]
    pub execution_boundary_complete: Option<bool>,
    pub final_trade_decision: String,
    pub reflection: Option<String>,
    pub raw_return: Option<f64>,
    pub alpha_return: Option<f64>,
    pub holding_days: Option<usize>,
    #[serde(default)]
    pub user_id: String,
    pub pending: bool,
}

/// Parameters for storing a trading decision.
pub struct DecisionRecord<'a> {
    pub ticker: &'a str,
    pub trade_date: &'a str,
    pub final_trade_decision: &'a str,
    pub rating: &'a str,
    pub action: &'a str,
    pub market: &'a str,
    pub direction_score: i32,
    pub confidence_score: i32,
    pub action_score: i32,
    pub research: Option<&'a ResearchMemoryRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct ResearchMemoryRecord {
    pub stock_name: String,
    pub summary: String,
    pub risk_assessment: String,
    pub rationale: String,
    pub structured_risk: StructuredRiskAssessment,
    pub structured_reflection: StructuredReflection,
    pub trigger_checklist: Vec<String>,
    pub blocking_gaps: Vec<String>,
    pub setup_tags: Vec<String>,
    pub execution_boundary_complete: bool,
    pub structured_snapshot: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct MemoryContextBundle {
    pub context_text: String,
    pub source: String,
    pub retrieval_mode: String,
    pub embedding_provider: String,
    pub embedding_failure_reason: Option<String>,
    pub same_ticker_count: usize,
    pub cross_ticker_count: usize,
    pub vector_hit_count: usize,
    pub effective_top_k: usize,
    pub same_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub cross_ticker_highlights: Vec<HistoricalMemoryHighlight>,
}

impl Default for MemoryContextBundle {
    fn default() -> Self {
        Self {
            context_text: String::new(),
            source: String::new(),
            retrieval_mode: "disabled".to_string(),
            embedding_provider: "disabled".to_string(),
            embedding_failure_reason: None,
            same_ticker_count: 0,
            cross_ticker_count: 0,
            vector_hit_count: 0,
            effective_top_k: 0,
            same_ticker_highlights: Vec::new(),
            cross_ticker_highlights: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MemoryContextBundleWithTags {
    pub context_text: String,
    pub source: String,
    pub retrieval_mode: String,
    pub embedding_provider: String,
    pub embedding_failure_reason: Option<String>,
    pub same_ticker_count: usize,
    pub cross_ticker_count: usize,
    pub vector_hit_count: usize,
    pub effective_top_k: usize,
    pub same_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub cross_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub setup_tags: Vec<String>,
    pub used_setup_filtered_retrieval: bool,
    pub used_setup_fallback_calibration: bool,
    pub setup_calibration_sample_count: usize,
    pub setup_match_count: usize,
    pub setup_pending_match_count: usize,
    pub setup_resolved_match_count: usize,
    pub setup_match_hit_rate: f64,
    pub setup_match_avg_alpha_return: f64,
    pub setup_long_match_count: usize,
    pub setup_short_match_count: usize,
    pub setup_neutral_match_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryQuery {
    pub ticker: String,
    pub market: String,
    pub setup_tags: Vec<String>,
    pub user_id: String,
}

mod core;
mod format;
pub mod fs_store;
mod stats;
pub mod store;

pub use fs_store::FilesystemMemoryStore;
pub use store::MemoryStore;

pub(crate) use stats::*;
