use sa::analysis::{
    AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph, AnalysisResult,
    DebateTurn, StructuredPortfolioDecision,
    StructuredResearchPlan, StructuredTraderPlan,
};
use sa::scoring::{
    score_catalyst_quality, score_cross_agent_consistency, score_data_quality, score_fundamentals,
    score_historical_transferability, score_risk_clarity, score_setup_direction_alignment,
    score_trend_confirmation,
};

fn make_analyst(key: &str, up: f64, down: f64, sideways: f64) -> AgentReportNode {
    AgentReportNode {
        key: key.into(),
        up_probability: up,
        down_probability: down,
        sideways_probability: sideways,
        evidence_points: vec!["evidence1".into(), "evidence2".into()],
        next_steps: vec!["step1".into()],
        ..Default::default()
    }
}

fn make_result_with_analysts(analysts: Vec<AgentReportNode>) -> AnalysisResult {
    let mut result = AnalysisResult {
        task_id: "test".into(),
        report_id: "rpt-test".into(),
        symbol: "TEST".into(),
        stock_name: "Test Corp".into(),
        analysis_date: "2026-06-22".into(),
        market_type: "美股".into(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: AnalysisArtifacts::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-22T00:00:00Z".into(),
    };
    result.graph.analysts = analysts;
    result
}

// --- score_data_quality ---

#[test]
fn score_data_quality_all_present() {
    let d = score_data_quality(4, 4, 5, 0);
    assert_eq!(d.score, 20);
    assert_eq!(d.max_score, 20);
}

#[test]
fn score_data_quality_all_empty() {
    let d = score_data_quality(0, 0, 0, 0);
    assert_eq!(d.score, 0);
}

#[test]
fn score_data_quality_with_failures() {
    let d = score_data_quality(4, 2, 3, 2);
    assert_eq!(d.score, 13);
}

#[test]
fn score_data_quality_failures_capped() {
    let d = score_data_quality(0, 0, 0, 10);
    assert_eq!(d.score, 0);
}

#[test]
fn score_data_quality_partial() {
    let d = score_data_quality(2, 1, 2, 1);
    // 6 + 1 + 2 - 2 = 7
    assert_eq!(d.score, 7);
}

// --- score_trend_confirmation ---

#[test]
fn score_trend_confirmation_with_analyst() {
    let analyst = make_analyst("market", 0.6, 0.2, 0.2);
    let trader = StructuredTraderPlan {
        entry_price: "100".into(),
        stop_loss: "95".into(),
        ..Default::default()
    };
    let portfolio = StructuredPortfolioDecision {
        price_target: "110".into(),
        ..Default::default()
    };
    let d = score_trend_confirmation(
        Some(&analyst),
        "market report with 1234 levels",
        &trader,
        &portfolio,
    );
    assert!(d.score > 5, "expected non-trivial score, got {}", d.score);
    assert_eq!(d.max_score, 20);
}

#[test]
fn score_trend_confirmation_empty_report() {
    let d = score_trend_confirmation(
        None,
        "",
        &StructuredTraderPlan::default(),
        &StructuredPortfolioDecision::default(),
    );
    assert_eq!(d.score, 0);
}

// --- score_fundamentals ---

#[test]
fn score_fundamentals_with_data() {
    let analyst = make_analyst("fundamentals", 0.55, 0.25, 0.2);
    let d = score_fundamentals(Some(&analyst), "PE 15.2 ROE 18%");
    assert!(d.score > 5, "expected non-trivial score, got {}", d.score);
}

#[test]
fn score_fundamentals_empty() {
    let d = score_fundamentals(None, "");
    assert_eq!(d.score, 0);
}

// --- score_catalyst_quality ---

#[test]
fn score_catalyst_quality_with_dates() {
    let analyst = make_analyst("news", 0.5, 0.3, 0.2);
    let portfolio = StructuredPortfolioDecision {
        time_horizon: "2026-07-01 to 2026-12-31".into(),
        ..Default::default()
    };
    let d = score_catalyst_quality(Some(&analyst), "earnings on 2026-07-15", &portfolio);
    assert!(d.score > 4, "expected non-trivial score, got {}", d.score);
}

#[test]
fn score_catalyst_quality_empty() {
    let d = score_catalyst_quality(None, "", &StructuredPortfolioDecision::default());
    assert_eq!(d.score, 0);
}

// --- score_historical_transferability ---

#[test]
fn score_historical_transferability_no_history() {
    let result = make_result_with_analysts(vec![]);
    let d = score_historical_transferability(&result);
    assert_eq!(d.score, 0);
}

#[test]
fn score_historical_transferability_with_setup_filter() {
    let mut result = make_result_with_analysts(vec![]);
    result
        .artifacts
        .memory_context
        .used_setup_filtered_retrieval = true;
    result.artifacts.memory_context.setup_match_count = 3;
    result.artifacts.memory_context.setup_resolved_match_count = 3;
    result.artifacts.memory_context.setup_match_hit_rate = 0.7;
    result.artifacts.memory_context.setup_match_avg_alpha_return = 0.05;
    let d = score_historical_transferability(&result);
    assert!(d.score >= 7, "expected high score, got {}", d.score);
}

#[test]
fn score_historical_transferability_fallback() {
    let mut result = make_result_with_analysts(vec![]);
    result
        .artifacts
        .memory_context
        .used_setup_filtered_retrieval = true;
    result
        .artifacts
        .memory_context
        .used_setup_fallback_calibration = true;
    result.artifacts.memory_context.setup_match_count = 2;
    result.artifacts.memory_context.setup_resolved_match_count = 2;
    let d = score_historical_transferability(&result);
    assert!(d.score >= 2, "expected some score, got {}", d.score);
}

#[test]
fn score_historical_transferability_same_ticker_only() {
    let mut result = make_result_with_analysts(vec![]);
    result.artifacts.memory_context.same_ticker_count = 1;
    let d = score_historical_transferability(&result);
    assert_eq!(d.score, 4); // 3 (no setup filter, has same_ticker) + 1
}

// --- score_setup_direction_alignment ---

#[test]
fn score_setup_direction_alignment_no_history() {
    let mut result = make_result_with_analysts(vec![]);
    result.artifacts.memory_context.setup_resolved_match_count = 0;
    let d = score_setup_direction_alignment(&result);
    assert_eq!(d.score, 4);
}

// --- score_cross_agent_consistency ---

#[test]
fn score_cross_agent_consistency_all_bullish() {
    let analysts = vec![
        make_analyst("market", 0.7, 0.15, 0.15),
        make_analyst("fundamentals", 0.65, 0.2, 0.15),
        make_analyst("news", 0.6, 0.2, 0.2),
    ];
    let result = make_result_with_analysts(analysts);
    let d = score_cross_agent_consistency(&result);
    assert!(d.score >= 13, "expected high consistency, got {}", d.score);
}

#[test]
fn score_cross_agent_consistency_split() {
    let analysts = vec![
        make_analyst("market", 0.7, 0.15, 0.15),
        make_analyst("fundamentals", 0.2, 0.6, 0.2),
    ];
    let result = make_result_with_analysts(analysts);
    let d = score_cross_agent_consistency(&result);
    assert!(d.score <= 8, "expected low consistency, got {}", d.score);
}

#[test]
fn score_cross_agent_consistency_empty() {
    let result = make_result_with_analysts(vec![]);
    let d = score_cross_agent_consistency(&result);
    assert_eq!(d.score, 6);
}

#[test]
fn score_cross_agent_consistency_all_bearish() {
    let analysts = vec![
        make_analyst("market", 0.15, 0.7, 0.15),
        make_analyst("fundamentals", 0.2, 0.65, 0.15),
    ];
    let result = make_result_with_analysts(analysts);
    let d = score_cross_agent_consistency(&result);
    assert!(
        d.score >= 13,
        "expected high consistency for all bearish, got {}",
        d.score
    );
}

// --- score_risk_clarity ---

#[test]
fn score_risk_clarity_with_debate() {
    let mut result = make_result_with_analysts(vec![]);
    result.graph.risk_debate.turns = vec![
        crate::DebateTurn {
            stance: "aggressive".into(),
            ..Default::default()
        },
        crate::DebateTurn {
            stance: "conservative".into(),
            ..Default::default()
        },
    ];
    let research = StructuredResearchPlan {
        risk_assessment: "high risk at 1200".into(),
        ..Default::default()
    };
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    let d = score_risk_clarity(&result, &research, &trader, &portfolio);
    assert!(d.score > 0, "expected non-zero score, got {}", d.score);
}

#[test]
fn score_risk_clarity_empty() {
    let result = make_result_with_analysts(vec![]);
    let research = StructuredResearchPlan::default();
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    let d = score_risk_clarity(&result, &research, &trader, &portfolio);
    assert_eq!(d.score, 0);
}
