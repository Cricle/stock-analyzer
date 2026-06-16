#[cfg(test)]
use super::*;
use crate::models::AnalysisResult;

fn default_result() -> AnalysisResult {
    AnalysisResult {
        task_id: "test".to_string(),
        report_id: "report-test".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "Test Corp".to_string(),
        analysis_date: "2026-06-05".to_string(),
        market_type: "US".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-05T00:00:00Z".to_string(),
    }
}

#[test]
fn test_probability_normalization() {
    let mut result = default_result();
    result.report.probability_view.upside_probability_pct = 60.0;
    result.report.probability_view.downside_probability_pct = 50.0;
    result.report.probability_view.sideways_probability_pct = 30.0;
    result.report.probability_view.risk_probability_pct = 10.0;
    // Directional sum = 140%, should be normalized to ~100%.

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let prob_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_probabilities")
        .collect();
    assert_eq!(prob_issues.len(), 1);

    let pv = &result.report.probability_view;
    let directional_sum = pv.upside_probability_pct
        + pv.downside_probability_pct
        + pv.sideways_probability_pct;
    assert!(
        (directional_sum - 100.0).abs() < 2.0,
        "directional trio should sum to ~100, got {}",
        directional_sum
    );
}

#[test]
fn test_probabilities_within_tolerance_are_unchanged() {
    let mut result = default_result();
    result.report.probability_view.upside_probability_pct = 45.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 28.0;
    result.report.probability_view.risk_probability_pct = 35.0;
    // Directional sum = 103%, within 5% tolerance of 100. Risk >= downside.

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let prob_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_probabilities")
        .collect();
    assert!(prob_issues.is_empty());
}

#[test]
fn test_entry_stop_guard() {
    let mut result = default_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "100.00".to_string();

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let stop_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_entry_stop")
        .collect();
    assert_eq!(stop_issues.len(), 1);

    let stop = parse_price(&result.report.trader_plan.stop_loss).unwrap();
    assert!(
        (stop - 98.0).abs() < 0.1,
        "stop should be ~98.0, got {}",
        stop
    );
}

#[test]
fn test_risk_reward_widening() {
    let mut result = default_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = crate::models::LocalText::new("103.00");
    // R:R = (103-100)/(100-95) = 0.6 < 1.5

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let rr_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_risk_reward")
        .collect();
    assert_eq!(rr_issues.len(), 1);

    let target = parse_price(result.report.decision_view.target_reference.as_str()).unwrap();
    // Expected: 100 + 1.5 * 5 = 107.50
    assert!(
        (target - 107.5).abs() < 0.1,
        "target should be ~107.5, got {}",
        target
    );
}

#[test]
fn test_position_sizing_cap() {
    let mut result = default_result();
    result.report.trader_plan.position_sizing = "30% of portfolio".to_string();

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let sizing_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_position_sizing")
        .collect();
    assert_eq!(sizing_issues.len(), 1);
    assert!(
        result.report.trader_plan.position_sizing.contains("20%"),
        "should be capped at 20%, got: {}",
        result.report.trader_plan.position_sizing
    );
}

#[test]
fn test_recommendation_downgrade_buy_with_high_downside() {
    let mut result = default_result();
    result.report.recommendation = "Buy".into();
    result.report.probability_view.downside_probability_pct = 70.0;

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let rec_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_recommendation_consistency")
        .collect();
    assert_eq!(rec_issues.len(), 1);
    assert_eq!(result.report.recommendation, "Hold".into());
}

#[test]
fn test_fill_missing_recommendation() {
    let mut result = default_result();
    result.report.recommendation = "".into();
    result.report.direction_score = 10;

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let fill_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fill_missing_fields" && i.field == "recommendation")
        .collect();
    assert_eq!(fill_issues.len(), 1);
    assert_eq!(result.report.recommendation, "Buy".into());
}

#[test]
fn test_fill_missing_confidence() {
    let mut result = default_result();
    result.report.confidence = "".into();

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let fill_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fill_missing_fields" && i.field == "confidence")
        .collect();
    assert_eq!(fill_issues.len(), 1);
    assert_eq!(result.report.confidence, "Low".into());
}

#[test]
fn test_fill_missing_scenario_paths() {
    let mut result = default_result();
    // Both buyer and holder paths are empty by default.

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let path_issues: Vec<_> = issues
        .iter()
        .filter(|i| {
            i.check_name == "fill_missing_fields"
                && i.field == "action_guides.buyers.scenario_paths"
        })
        .collect();
    assert_eq!(path_issues.len(), 1);
    assert!(!result.report.action_guides.buyers.scenario_paths.is_empty());
}

#[test]
fn test_no_issues_on_clean_result() {
    let mut result = default_result();
    result.report.recommendation = "Hold".into();
    result.report.confidence = "Medium".into();
    result.report.probability_view.upside_probability_pct = 30.0;
    result.report.probability_view.downside_probability_pct = 25.0;
    result.report.probability_view.sideways_probability_pct = 45.0;
    result.report.probability_view.risk_probability_pct = 30.0;
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.trader_plan.position_sizing = "10%".to_string();
    result.report.decision_view.target_reference = crate::models::LocalText::new("120.00");

    use crate::models::{ActionScenarioPath, LocalText};
    result
        .report
        .action_guides
        .buyers
        .scenario_paths
        .push(ActionScenarioPath {
            key: "test".to_string(),
            name: LocalText::new("Test"),
            trigger: LocalText::new("Test"),
            action: LocalText::new("Test"),
            risk_boundary: LocalText::new("Test"),
            position_sizing: LocalText::new("10%"),
            stop_level: LocalText::new("95"),
            sizing_blocked: false,
        });

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    assert!(
        issues.is_empty(),
        "clean result should have no issues, got {}",
        issues.len()
    );
}

#[test]
fn test_probability_normalization_directional_only() {
    let mut result = default_result();
    result.report.probability_view.upside_probability_pct = 60.0;
    result.report.probability_view.downside_probability_pct = 50.0;
    result.report.probability_view.sideways_probability_pct = 30.0;
    result.report.probability_view.risk_probability_pct = 40.0;
    // Directional sum = 140%, risk = 40% (independent, stays unchanged).

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let prob_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_probabilities")
        .collect();
    assert_eq!(prob_issues.len(), 1, "should trigger directional normalization");

    let pv = &result.report.probability_view;
    let directional_sum = pv.upside_probability_pct
        + pv.downside_probability_pct
        + pv.sideways_probability_pct;
    assert!(
        (directional_sum - 100.0).abs() < 2.0,
        "directional trio should sum to ~100, got {}",
        directional_sum
    );
    assert!(
        (pv.risk_probability_pct - 40.0).abs() < 1.0,
        "risk should stay at ~40, got {}",
        pv.risk_probability_pct
    );
}

#[test]
fn test_risk_at_least_downside() {
    let mut result = default_result();
    result.report.probability_view.upside_probability_pct = 30.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 30.0;
    result.report.probability_view.risk_probability_pct = 20.0;
    // Directional sum = 90%, within tolerance. But risk (20) < downside (30).

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let risk_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_risk_invariant")
        .collect();
    assert_eq!(risk_issues.len(), 1, "should trigger risk >= downside fix");

    let pv = &result.report.probability_view;
    assert!(
        pv.risk_probability_pct >= pv.downside_probability_pct,
        "risk ({}) should be >= downside ({})",
        pv.risk_probability_pct,
        pv.downside_probability_pct
    );
}

#[test]
fn test_risk_clamped_to_valid_range() {
    let mut result = default_result();
    result.report.probability_view.upside_probability_pct = 30.0;
    result.report.probability_view.downside_probability_pct = 25.0;
    result.report.probability_view.sideways_probability_pct = 35.0;
    result.report.probability_view.risk_probability_pct = 98.0;
    // Directional sum = 90%, within tolerance. Risk = 98, should clamp to 95.

    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    let risk_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.check_name == "fix_risk_range")
        .collect();
    assert_eq!(risk_issues.len(), 1, "should trigger risk range clamp");

    let pv = &result.report.probability_view;
    assert!(
        pv.risk_probability_pct <= 95.0,
        "risk should be clamped to <= 95, got {}",
        pv.risk_probability_pct
    );
}
