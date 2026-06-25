use sa::analysis::{
    AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisCheckpoint, AnalysisGraph,
    AnalysisScenarioContext, AnalysisScenarioData, AnalysisUserContext, AnalystRuntimeState,
    DiagnosisIssue, DiagnosisSummary, InvestmentDebateState, LlmTokenUsageSummary,
    MemoryContextSnapshot, ReflectionState, ReportMarketChart, RiskDebateState, RuntimeNodeTrace,
    StructuredPortfolioDecision, StructuredResearchPlan, StructuredTraderPlan,
};

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
            severity: "error".into(),
            check_name: "c1".into(),
            field: "f1".into(),
            original_value: "old".into(),
            fixed_value: "new".into(),
            message: "fixed".into(),
        },
        DiagnosisIssue {
            severity: "warning".into(),
            check_name: "c2".into(),
            field: "f2".into(),
            original_value: "val".into(),
            fixed_value: "".into(),
            message: "unfixed".into(),
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
        severity: "error".into(),
        check_name: "check".into(),
        field: "field".into(),
        original_value: "old".into(),
        fixed_value: "new".into(),
        message: "msg".into(),
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
