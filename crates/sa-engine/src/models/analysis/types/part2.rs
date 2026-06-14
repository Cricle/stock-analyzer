
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
    /// i18n keys for catalysts (resolved by frontend or resolve_output).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalyst_keys: Vec<serde_json::Value>,
    /// i18n keys for risks (resolved by frontend or resolve_output).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_keys: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickSelectionDiagnostics {
    #[serde(default)]
    pub search_depth: String,
    #[serde(default)]
    pub vector_store_enabled: bool,
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
    pub vector_points_written: usize,
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
