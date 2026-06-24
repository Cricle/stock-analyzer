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

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn analysis_graph_serde_roundtrip() {
        let g = AnalysisGraph {
            analysts: vec![AgentReportNode::default()],
            investment_debate: InvestmentDebateState::default(),
            risk_debate: RiskDebateState::default(),
            reflection: ReflectionState::default(),
            checkpoints: vec![],
        };
        let json = serde_json::to_string(&g).unwrap();
        let restored: AnalysisGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.analysts.len(), 1);
    }

    #[test]
    fn agent_state_snapshot_serde_roundtrip() {
        let s = AgentStateSnapshot {
            company_of_interest: "AAPL".into(),
            trade_date: "2025-01-15".into(),
            sender: "system".into(),
            market_report: "mr".into(),
            sentiment_report: "sr".into(),
            news_report: "nr".into(),
            fundamentals_report: "fr".into(),
            investment_debate_state: InvestmentDebateState::default(),
            investment_plan: "ip".into(),
            structured_research_plan: StructuredResearchPlan::default(),
            trader_investment_plan: "tip".into(),
            structured_trader_plan: StructuredTraderPlan::default(),
            risk_debate_state: RiskDebateState::default(),
            final_trade_decision: "buy".into(),
            structured_portfolio_decision: StructuredPortfolioDecision::default(),
            past_context: "pc".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: AgentStateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.company_of_interest, "AAPL");
    }

    #[test]
    fn analysis_artifacts_serde_roundtrip() {
        let a = AnalysisArtifacts {
            full_state_log_path: "/tmp/log".into(),
            checkpoint_thread_id: "t1".into(),
            resumed_from_node: "n1".into(),
            resumed_from_step: 5,
            runtime_nodes: vec![],
            analyst_runtime_states: vec![],
            memory_context: MemoryContextSnapshot::default(),
            llm_token_usage: LlmTokenUsageSummary::default(),
            market_chart: ReportMarketChart::default(),
            user_context: AnalysisUserContext::default(),
            scenario_context: AnalysisScenarioContext::default(),
            scenario_data: AnalysisScenarioData::default(),
            calibration_memo: "memo".into(),
            diagnosis_summary: Some(DiagnosisSummary {
                total_issues: 2,
                fixed_count: 1,
                unfixed_count: 1,
                issues: vec![],
                validated_at: "2025-01-01".into(),
            }),
        };
        let json = serde_json::to_string(&a).unwrap();
        let restored: AnalysisArtifacts = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.resumed_from_step, 5);
        assert!(restored.diagnosis_summary.is_some());
    }

    #[test]
    fn diagnosis_summary_from_issues() {
        let issues = vec![
            DiagnosisIssue {
                severity: "error".into(), check_name: "c1".into(),
                field: "f1".into(), original_value: "old".into(),
                fixed_value: "new".into(), message: "fixed".into(),
            },
            DiagnosisIssue {
                severity: "warning".into(), check_name: "c2".into(),
                field: "f2".into(), original_value: "val".into(),
                fixed_value: "".into(), message: "unfixed".into(),
            },
        ];
        let summary = DiagnosisSummary::from_issues(&issues);
        assert_eq!(summary.total_issues, 2);
        assert_eq!(summary.fixed_count, 1);
        assert_eq!(summary.unfixed_count, 1);
    }

    #[test]
    fn diagnosis_issue_serde_roundtrip() {
        let i = DiagnosisIssue {
            severity: "error".into(), check_name: "check".into(),
            field: "field".into(), original_value: "old".into(),
            fixed_value: "new".into(), message: "msg".into(),
        };
        let json = serde_json::to_string(&i).unwrap();
        let restored: DiagnosisIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.severity, "error");
    }

    #[test]
    fn diagnosis_summary_default() {
        let d = DiagnosisSummary::default();
        assert_eq!(d.total_issues, 0);
        assert!(d.issues.is_empty());
    }

    #[test]
    fn analysis_graph_default() {
        let g = AnalysisGraph::default();
        assert!(g.analysts.is_empty());
        assert!(g.checkpoints.is_empty());
    }
}
