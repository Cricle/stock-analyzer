use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use crate::models::task::TaskStatus;


#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickItem {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub confidence: f64,
    pub thesis: String,
    #[serde(default)]
    pub catalysts: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub evidence_points: Vec<String>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub change_pct: Option<f64>,
    #[serde(default)]
    pub market_cap: Option<f64>,
    #[serde(default)]
    pub priority_label: String,
    #[serde(default)]
    pub priority_rank: i32,
    #[serde(default)]
    pub sort_key: f64,
    #[serde(default)]
    pub objective_assessment: StockPickObjectiveAssessment,
    #[serde(default)]
    pub factor_breakdown: StockPickFactorBreakdown,
    #[serde(default)]
    pub market_snapshot: StockPickMarketSnapshot,
    #[serde(default)]
    pub technical_snapshot: StockPickTechnicalSnapshot,
    #[serde(default)]
    pub fundamental_snapshot: StockPickFundamentalSnapshot,
    #[serde(default)]
    pub news_snapshot: StockPickNewsSnapshot,
    #[serde(default)]
    pub history_match_snapshot: StockPickHistoryMatchSnapshot,
    #[serde(default)]
    pub risk_snapshot: StockPickRiskSnapshot,
    #[serde(default)]
    pub data_quality_snapshot: StockPickDataQualitySnapshot,
    #[serde(default)]
    pub selection_reason_codes: Vec<String>,
    #[serde(default)]
    pub rejection_risk_flags: Vec<String>,
    #[serde(default)]
    pub evidence_quality_score: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickSelectionDiagnostics {
    #[serde(default)]
    pub search_depth: String,
    #[serde(default)]
    pub qdrant_enabled: bool,
    #[serde(default)]
    pub redis_enabled: bool,
    #[serde(default)]
    pub history_retrieval_enabled: bool,
    #[serde(default)]
    pub agreement_with_system_rank: String,
    #[serde(default)]
    pub override_count: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickEvidenceCoverageSummary {
    #[serde(default)]
    pub light_search_symbols: usize,
    #[serde(default)]
    pub deep_search_symbols: usize,
    #[serde(default)]
    pub evidence_records_indexed: usize,
    #[serde(default)]
    pub history_records_matched: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickStorageWriteSummary {
    #[serde(default)]
    pub redis_keys_written: usize,
    #[serde(default)]
    pub qdrant_points_written: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickFailureInfo {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickResponse {
    pub market: String,
    pub strategy: String,
    pub analysis_date: String,
    pub candidate_count: usize,
    pub evaluated_count: usize,
    #[serde(default)]
    pub coarse_candidate_count: usize,
    #[serde(default)]
    pub deep_evaluated_count: usize,
    #[serde(default)]
    pub winner_count: usize,
    pub picks: Vec<StockPickItem>,
    pub summary: String,
    #[serde(default)]
    pub rejected_symbols: Vec<String>,
    #[serde(default)]
    pub objective_overview: StockPickObjectiveOverview,
    #[serde(default)]
    pub selection_engine_version: String,
    #[serde(default)]
    pub selection_diagnostics: StockPickSelectionDiagnostics,
    #[serde(default)]
    pub evidence_coverage_summary: StockPickEvidenceCoverageSummary,
    #[serde(default)]
    pub history_match_summary: StockPickHistoryMatchSnapshot,
    #[serde(default)]
    pub storage_write_summary: StockPickStorageWriteSummary,
    #[serde(default)]
    pub failure: Option<StockPickFailureInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockPickRunRecord {
    pub id: String,
    pub owner_username: String,
    pub request: StockPickRequest,
    pub result: StockPickResponse,
    pub remaining_credits: f64,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub worker_base_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickObjectiveAssessment {
    #[serde(default)]
    pub final_score: i32,
    #[serde(default)]
    pub grade: String,
    #[serde(default)]
    pub ready: bool,
    #[serde(default)]
    pub headline: String,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub breakdown: StockPickObjectiveBreakdown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickObjectiveBreakdown {
    #[serde(default)]
    pub data_completeness: ScoreDimension,
    #[serde(default)]
    pub market_validation: ScoreDimension,
    #[serde(default)]
    pub reasoning_structure: ScoreDimension,
    #[serde(default)]
    pub risk_balance: ScoreDimension,
    #[serde(default)]
    pub evidence_density: ScoreDimension,
    #[serde(default)]
    pub total_score: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickObjectiveOverview {
    #[serde(default)]
    pub average_score: f64,
    #[serde(default)]
    pub average_grade: String,
    #[serde(default)]
    pub min_score: i32,
    #[serde(default)]
    pub max_score: i32,
    #[serde(default)]
    pub ready_picks: usize,
    #[serde(default)]
    pub incomplete_picks: usize,
    #[serde(default)]
    pub distribution: Vec<StockPickObjectiveBucket>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickObjectiveBucket {
    pub label: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub task_id: String,
    pub report_id: String,
    pub symbol: String,
    pub stock_name: String,
    pub analysis_date: String,
    pub market_type: String,
    #[serde(default)]
    pub graph: AnalysisGraph,
    #[serde(default)]
    pub agent_state: AgentStateSnapshot,
    #[serde(default)]
    pub artifacts: AnalysisArtifacts,
    #[serde(default)]
    pub report: StructuredReport,
    #[serde(default)]
    pub ic_report: StructuredReport,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_pick_item_default() {
        let item = StockPickItem::default();
        assert!(item.symbol.is_empty());
        assert!(item.catalysts.is_empty());
        assert!(item.risks.is_empty());
    }

    #[test]
    fn stock_pick_item_roundtrip() {
        let item = StockPickItem {
            symbol: "AAPL".to_string(),
            name: "Apple Inc.".to_string(),
            market: "us_equity".to_string(),
            exchange: "NASDAQ".to_string(),
            score: 85.5,
            confidence: 0.75,
            thesis: "Strong growth".to_string(),
            catalysts: vec!["AI".to_string()],
            risks: vec!["Competition".to_string()],
            price: Some(150.0),
            priority_label: "A".to_string(),
            priority_rank: 1,
            sort_key: 85.5,
            ..Default::default()
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: StockPickItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.symbol, "AAPL");
        assert_eq!(parsed.score, 85.5);
    }

    #[test]
    fn stock_pick_response_default() {
        let resp = StockPickResponse::default();
        assert!(resp.market.is_empty());
        assert!(resp.picks.is_empty());
    }

    #[test]
    fn stock_pick_response_roundtrip() {
        let resp = StockPickResponse {
            market: "a_share".to_string(),
            strategy: "momentum".to_string(),
            analysis_date: "2026-06-23".to_string(),
            candidate_count: 10,
            evaluated_count: 5,
            picks: vec![],
            summary: "Top picks".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: StockPickResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.market, "a_share");
        assert_eq!(parsed.candidate_count, 10);
    }

    #[test]
    fn stock_pick_selection_diagnostics_default() {
        let diag = StockPickSelectionDiagnostics::default();
        assert!(diag.search_depth.is_empty());
        assert!(!diag.qdrant_enabled);
    }

    #[test]
    fn stock_pick_evidence_coverage_default() {
        let cov = StockPickEvidenceCoverageSummary::default();
        assert_eq!(cov.light_search_symbols, 0);
    }

    #[test]
    fn stock_pick_failure_info_roundtrip() {
        let info = StockPickFailureInfo {
            code: "timeout".to_string(),
            message: "Request timed out".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: StockPickFailureInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code, "timeout");
    }

    #[test]
    fn stock_pick_objective_assessment_default() {
        let assess = StockPickObjectiveAssessment::default();
        assert_eq!(assess.final_score, 0);
        assert!(!assess.ready);
    }

    #[test]
    fn stock_pick_objective_bucket_roundtrip() {
        let bucket = StockPickObjectiveBucket {
            label: "A".to_string(),
            count: 3,
        };
        let json = serde_json::to_string(&bucket).unwrap();
        let parsed: StockPickObjectiveBucket = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.count, 3);
    }

    #[test]
    fn analysis_result_default() {
        let result = AnalysisResult::default();
        assert!(result.symbol.is_empty());
        assert!(result.task_id.is_empty());
    }
}
