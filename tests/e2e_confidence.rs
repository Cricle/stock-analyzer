use sa::scoring::config::ConfidenceCapsConfig;
use sa::{
    AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph, AnalysisResult,
    AnalystRuntimeState, Rating, StructuredPortfolioDecision, StructuredTraderPlan,
    ToolObservation, evaluate_confidence_score,
};

/// Build a well-formed AnalysisResult with 4 analysts, non-empty core reports,
/// complete execution boundary, and consistent bullish direction.
fn good_result() -> AnalysisResult {
    let make_analyst = |key: &str, up: f64, down: f64, sideways: f64| -> AgentReportNode {
        AgentReportNode {
            key: key.into(),
            agent: key.into(),
            up_probability: up,
            down_probability: down,
            sideways_probability: sideways,
            evidence_points: vec![
                "evidence_a".into(),
                "evidence_b".into(),
                "evidence_c".into(),
            ],
            next_steps: vec!["step_1".into(), "step_2".into()],
            ..Default::default()
        }
    };

    let analysts = vec![
        make_analyst("market", 0.65, 0.15, 0.20),
        make_analyst("fundamentals", 0.60, 0.18, 0.22),
        make_analyst("news", 0.55, 0.20, 0.25),
        make_analyst("sentiment", 0.58, 0.19, 0.23),
    ];

    let mut agent_state = AgentStateSnapshot::default();
    agent_state.market_report = "Strong uptrend with resistance at 150, support at 120".into();
    agent_state.fundamentals_report = "PE 18.5 ROE 22% growing revenue 15% YoY".into();
    agent_state.news_report = "Earnings beat expectations on 2026-07-15, new product launch".into();
    agent_state.sentiment_report =
        "Positive sentiment from institutional investors, 65% bullish".into();

    let mut trader_plan = StructuredTraderPlan::default();
    trader_plan.entry_price = "135.50".into();
    trader_plan.stop_loss = "128.00".into();
    trader_plan.execution_trigger_checklist = vec![
        "confirm_breakout".into(),
        "volume_surge".into(),
        "sector_momentum".into(),
    ];

    let mut portfolio_decision = StructuredPortfolioDecision::default();
    portfolio_decision.rating = Rating::Buy;
    portfolio_decision.raw_rating = "Buy".into();
    portfolio_decision.price_target = "155.00".into();
    portfolio_decision.confirmation_level = "145.00".into();
    portfolio_decision.time_horizon = "2026-07-01 to 2026-12-31".into();
    portfolio_decision.trigger_checklist = vec!["check_earnings".into(), "check_sector".into()];

    agent_state.structured_trader_plan = trader_plan;
    agent_state.structured_portfolio_decision = portfolio_decision;

    let mut artifacts = AnalysisArtifacts::default();
    artifacts.analyst_runtime_states = vec![AnalystRuntimeState {
        key: "market".into(),
        tool_history: vec![ToolObservation {
            tool_name: "get_market_data".into(),
            arguments: serde_json::Value::Null,
            output: "success".into(),
            meta: serde_json::Value::Null,
            success: true,
            created_at: "2026-06-29T00:00:00Z".into(),
        }],
        ..Default::default()
    }];

    AnalysisResult {
        task_id: "task-e2e".into(),
        report_id: "rpt-e2e".into(),
        symbol: "TEST".into(),
        stock_name: "Test Corp".into(),
        analysis_date: "2026-06-29".into(),
        market_type: "\u{7f8e}\u{80a1}".into(),
        graph: AnalysisGraph {
            analysts,
            ..Default::default()
        },
        agent_state,
        artifacts,
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-29T00:00:00Z".into(),
    }
}

#[test]
fn confidence_score_with_good_data_exceeds_60() {
    let result = good_result();
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment.final_score >= 60,
        "expected confidence >= 60 with good data, got {} (caps: {:?})",
        assessment.final_score,
        assessment.caps,
    );
}

#[test]
fn confidence_score_with_good_data_has_no_missing_core_data_cap() {
    let result = good_result();
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment.caps.iter().all(|c| c.key != "missing_core_data"),
        "should not trigger missing_core_data cap with 4 non-empty reports, got caps: {:?}",
        assessment.caps,
    );
}

#[test]
fn confidence_score_with_good_data_has_no_thin_evidence_cap() {
    let result = good_result();
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment
            .caps
            .iter()
            .all(|c| c.key != "thin_evidence_density"),
        "should not trigger thin_evidence_density cap with 3 evidence points per analyst, got caps: {:?}",
        assessment.caps,
    );
}

#[test]
fn confidence_score_with_good_data_has_no_execution_boundary_cap() {
    let result = good_result();
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment
            .caps
            .iter()
            .all(|c| c.key != "execution_boundary_missing"),
        "should not trigger execution_boundary_missing cap with complete execution boundary, got caps: {:?}",
        assessment.caps,
    );
}

#[test]
fn confidence_score_with_good_data_has_no_missing_follow_up_cap() {
    let result = good_result();
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment
            .caps
            .iter()
            .all(|c| c.key != "missing_follow_up_plan"),
        "should not trigger missing_follow_up_plan cap when next_steps are present, got caps: {:?}",
        assessment.caps,
    );
}

#[test]
fn confidence_score_with_empty_data_is_lower() {
    let result = AnalysisResult {
        task_id: "task-empty".into(),
        report_id: "rpt-empty".into(),
        symbol: "EMPTY".into(),
        stock_name: "Empty Corp".into(),
        analysis_date: "2026-06-29".into(),
        market_type: "\u{7f8e}\u{80a1}".into(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: AnalysisArtifacts::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-29T00:00:00Z".into(),
    };
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    assert!(
        assessment.final_score < 60,
        "expected confidence < 60 with empty data, got {}",
        assessment.final_score,
    );
}

#[test]
fn direction_confidence_is_not_halved_for_hold_recommendation() {
    // This test verifies the fix from Task 9: Hold no longer halves direction confidence.
    let mut result = good_result();
    result.agent_state.structured_portfolio_decision.rating = Rating::Hold;
    result.agent_state.structured_portfolio_decision.raw_rating = "Hold".into();

    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);

    // With good data and Hold rating, confidence should still be decent
    // (not crushed by a Hold penalty that no longer exists)
    assert!(
        assessment.profile.direction_confidence.score > 30,
        "direction confidence should not be halved for Hold, got {}",
        assessment.profile.direction_confidence.score,
    );
}
