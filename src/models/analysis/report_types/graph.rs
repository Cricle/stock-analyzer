#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisGraph {
    #[serde(default)]
    pub analysts: Vec<AgentReportNode>,
    #[serde(default)]
    pub investment_debate: InvestmentDebateState,
    #[serde(default)]
    pub risk_debate: RiskDebateState,
    #[serde(default)]
    pub reflection: ReflectionState,
    #[serde(default)]
    pub checkpoints: Vec<AnalysisCheckpoint>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentStateSnapshot {
    pub company_of_interest: String,
    pub trade_date: String,
    pub sender: String,
    /// i18n key for `sender`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_key: Option<String>,
    pub market_report: String,
    pub sentiment_report: String,
    pub news_report: String,
    pub fundamentals_report: String,
    pub investment_debate_state: InvestmentDebateState,
    pub investment_plan: String,
    #[serde(default)]
    pub structured_research_plan: StructuredResearchPlan,
    pub trader_investment_plan: String,
    #[serde(default)]
    pub structured_trader_plan: StructuredTraderPlan,
    pub risk_debate_state: RiskDebateState,
    pub final_trade_decision: String,
    #[serde(default)]
    pub structured_portfolio_decision: StructuredPortfolioDecision,
    pub past_context: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisArtifacts {
    pub full_state_log_path: String,
    #[serde(default)]
    pub checkpoint_thread_id: String,
    #[serde(default)]
    pub resumed_from_node: String,
    #[serde(default)]
    pub resumed_from_step: i64,
    #[serde(default)]
    pub runtime_nodes: Vec<RuntimeNodeTrace>,
    #[serde(default)]
    pub analyst_runtime_states: Vec<AnalystRuntimeState>,
    #[serde(default)]
    pub memory_context: MemoryContextSnapshot,
    #[serde(default)]
    pub llm_token_usage: LlmTokenUsageSummary,
    #[serde(default)]
    pub market_chart: ReportMarketChart,
    #[serde(default)]
    pub user_context: AnalysisUserContext,
    #[serde(default)]
    pub scenario_context: AnalysisScenarioContext,
    #[serde(default)]
    pub scenario_data: AnalysisScenarioData,
    #[serde(default, skip_serializing)]
    pub calibration_memo: String,
    #[serde(default)]
    pub diagnosis_summary: Option<DiagnosisSummary>,
}

/// Summary of consistency validation issues found and auto-fixed by the diagnosis pipeline.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiagnosisSummary {
    /// Total number of issues detected.
    #[serde(default)]
    pub total_issues: usize,
    /// Number of issues auto-fixed in-place.
    #[serde(default)]
    pub fixed_count: usize,
    /// Number of issues that could not be auto-fixed (warnings/info).
    #[serde(default)]
    pub unfixed_count: usize,
    /// Machine-readable issue list for downstream consumers.
    #[serde(default)]
    pub issues: Vec<DiagnosisIssue>,
    /// When the diagnosis ran (RFC 3339).
    #[serde(default)]
    pub validated_at: String,
}

/// A single consistency issue detected by the diagnosis pipeline.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiagnosisIssue {
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub check_name: String,
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub original_value: String,
    #[serde(default)]
    pub fixed_value: String,
    #[serde(default)]
    pub message: String,
}

impl DiagnosisSummary {
    pub fn from_issues(issues: &[DiagnosisIssue]) -> Self {
        let fixed_count = issues
            .iter()
            .filter(|i| !i.fixed_value.is_empty() && i.fixed_value != i.original_value)
            .count();
        Self {
            total_issues: issues.len(),
            fixed_count,
            unfixed_count: issues.len() - fixed_count,
            issues: issues.to_vec(),
            validated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}
