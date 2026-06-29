use sa::scoring::{
    CalibrationProfile, calibrate_recommendation, calibrate_recommendation_with_profile,
    evaluate_confidence_score, evaluate_direction_score,
};
use sa::{
    AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph, AnalysisResult,
    AnalystRuntimeState, ToolObservation,
};

fn empty_result() -> AnalysisResult {
    AnalysisResult {
        task_id: "task-test".to_string(),
        report_id: "report-test".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "Test".to_string(),
        analysis_date: "2026-05-11".to_string(),
        market_type: "美股".to_string(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: AnalysisArtifacts::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-11T00:00:00Z".to_string(),
    }
}

#[test]
fn weak_evidence_buy_is_forced_back_to_hold() {
    let calibrated = calibrate_recommendation("Buy", 18, 52, 41, false);
    assert_eq!(calibrated.final_rating, "Hold");
    assert_eq!(calibrated.final_action, "Hold");
}

#[test]
fn strong_positive_evidence_can_promote_hold() {
    let calibrated = calibrate_recommendation("Hold", 72, 84, 81, true);
    assert_eq!(calibrated.final_rating, "Buy");
    assert_eq!(calibrated.final_action, "Buy");
}

#[test]
fn strong_negative_evidence_can_demote_hold() {
    let calibrated = calibrate_recommendation("Hold", -66, 82, 79, true);
    assert_eq!(calibrated.final_rating, "Sell");
    assert_eq!(calibrated.final_action, "Sell");
}

#[test]
fn missing_execution_boundary_allows_mild_upgrade_with_strong_scores() {
    let calibrated = calibrate_recommendation("Buy", 78, 84, 88, false);
    assert_eq!(calibrated.final_rating, "Overweight");
    assert_eq!(calibrated.final_action, "Buy");
}

#[test]
fn direction_score_uses_structured_probabilities() {
    let mut result = empty_result();
    result.graph.analysts = vec![
        AgentReportNode {
            key: "market".to_string(),
            up_probability: 0.72,
            down_probability: 0.14,
            sideways_probability: 0.14,
            ..Default::default()
        },
        AgentReportNode {
            key: "fundamentals".to_string(),
            up_probability: 0.66,
            down_probability: 0.18,
            sideways_probability: 0.16,
            ..Default::default()
        },
    ];
    result.agent_state.final_trade_decision = "**Recommendation**: Hold".to_string();
    let assessment = evaluate_direction_score(&result);
    assert!(assessment.final_score > 0);
}

#[test]
fn confidence_caps_missing_core_data_without_text_matching() {
    let mut result = empty_result();
    result.graph.analysts = vec![AgentReportNode {
        key: "market".to_string(),
        evidence_points: vec!["price".to_string()],
        up_probability: 0.55,
        down_probability: 0.20,
        sideways_probability: 0.25,
        ..Default::default()
    }];
    let caps = sa::scoring::config::ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    assert!(assessment.final_score <= 65);
    assert!(
        assessment
            .caps
            .iter()
            .any(|item| item.key == "missing_core_data")
    );
}

#[test]
fn setup_history_gap_adds_confidence_cap() {
    let mut result = empty_result();
    result
        .artifacts
        .memory_context
        .used_setup_filtered_retrieval = true;
    result.artifacts.memory_context.setup_match_count = 1;
    let caps = sa::scoring::config::ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    assert!(
        assessment
            .caps
            .iter()
            .any(|item| item.key == "thin_setup_history")
    );
}

#[test]
fn single_tool_failure_does_not_trigger_missing_core_data_cap_when_core_is_present() {
    let mut result = empty_result();
    result.agent_state.market_report = "market".to_string();
    result.agent_state.fundamentals_report = "fundamentals".to_string();
    result.agent_state.news_report = "news".to_string();
    result.graph.analysts = vec![
        AgentReportNode {
            key: "market".to_string(),
            evidence_points: vec!["price".to_string(), "trend".to_string()],
            up_probability: 0.60,
            down_probability: 0.15,
            sideways_probability: 0.25,
            next_steps: vec!["watch breakout".to_string()],
            ..Default::default()
        },
        AgentReportNode {
            key: "fundamentals".to_string(),
            evidence_points: vec!["cashflow".to_string(), "margin".to_string()],
            up_probability: 0.56,
            down_probability: 0.18,
            sideways_probability: 0.26,
            next_steps: vec!["verify filings".to_string()],
            ..Default::default()
        },
        AgentReportNode {
            key: "news".to_string(),
            evidence_points: vec!["earnings".to_string(), "buyback".to_string()],
            up_probability: 0.52,
            down_probability: 0.20,
            sideways_probability: 0.28,
            next_steps: vec!["track catalyst".to_string()],
            ..Default::default()
        },
    ];
    result.artifacts.analyst_runtime_states = vec![AnalystRuntimeState {
        key: "news".to_string(),
        tool_history: vec![ToolObservation {
            tool_name: "get_global_news".to_string(),
            arguments: serde_json::Value::Null,
            output: "upstream timeout".to_string(),
            meta: serde_json::Value::Null,
            success: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        }],
        ..Default::default()
    }];

    let caps = sa::scoring::config::ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    assert!(
        assessment
            .caps
            .iter()
            .all(|item| item.key != "missing_core_data")
    );
}

#[test]
fn setup_history_cap_is_relaxed_when_fallback_samples_exist() {
    let mut result = empty_result();
    result
        .artifacts
        .memory_context
        .used_setup_filtered_retrieval = true;
    result.artifacts.memory_context.setup_match_count = 1;
    result.artifacts.memory_context.same_ticker_count = 2;
    let caps = sa::scoring::config::ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    let cap = assessment
        .caps
        .iter()
        .find(|item| item.key == "thin_setup_history")
        .map(|item| item.cap)
        .unwrap_or_default();
    assert_eq!(cap, 92);
}

#[test]
fn semantic_analyst_matching_handles_noncanonical_keys() {
    let mut result = empty_result();
    result.graph.analysts = vec![
        AgentReportNode {
            key: "NVDA_news_2026-05-13".to_string(),
            title: "NVDA 近一周新闻催化".to_string(),
            agent: "news".to_string(),
            up_probability: 0.36,
            down_probability: 0.24,
            sideways_probability: 0.40,
            ..Default::default()
        },
        AgentReportNode {
            key: "NVDA".to_string(),
            title: "NVDA 资金情绪".to_string(),
            agent: "sentiment".to_string(),
            up_probability: 0.46,
            down_probability: 0.22,
            sideways_probability: 0.32,
            ..Default::default()
        },
        AgentReportNode {
            key: "NVDA".to_string(),
            title: "NVDA 基本面分析".to_string(),
            agent: "fundamentals".to_string(),
            up_probability: 0.52,
            down_probability: 0.18,
            sideways_probability: 0.30,
            ..Default::default()
        },
    ];
    let direction = evaluate_direction_score(&result);
    assert!(direction.breakdown.news.score > 0);
    assert!(direction.breakdown.sentiment.score > 0);
    assert!(direction.breakdown.fundamentals.score > 0);
}

#[test]
fn direction_penalty_can_block_marginal_buy_upgrade() {
    let profile = CalibrationProfile::default();
    let calibrated =
        calibrate_recommendation_with_profile("Hold", 22, 70, 65, true, &profile, 13, None);
    // With strong_direction_abs=35 and penalty=13, floor=25; direction=22 < 25 blocks upgrade
    assert_eq!(calibrated.final_rating, "Hold");
    assert_eq!(calibrated.final_action, "Hold");
}
