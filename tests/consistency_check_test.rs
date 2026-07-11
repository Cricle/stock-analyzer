use stock_analyzer::report::diagnosis::consistency::check::{
    extract_pct, fix_entry_stop, fix_probabilities, fix_risk_reward, parse_price, round_price,
};
use stock_analyzer::{AnalysisResult, LocalText};

fn make_result() -> AnalysisResult {
    AnalysisResult {
        task_id: "test-task".to_string(),
        report_id: "test-report".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "Test Stock".to_string(),
        analysis_date: "2025-01-15".to_string(),
        market_type: "us".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2025-01-15T00:00:00Z".to_string(),
    }
}

// ---- parse_price ----

#[test]
fn parse_price_plain_number() {
    assert_eq!(parse_price("12.50"), Some(12.50));
}

#[test]
fn parse_price_dollar_prefix() {
    assert_eq!(parse_price("$12.50"), Some(12.50));
}

#[test]
fn parse_price_cny_suffix() {
    assert_eq!(parse_price("12.50 CNY"), Some(12.50));
}

#[test]
fn parse_price_yuan_suffix() {
    assert_eq!(parse_price("12.50元"), Some(12.50));
}

#[test]
fn parse_price_empty_string() {
    assert_eq!(parse_price(""), None);
}

#[test]
fn parse_price_no_digits() {
    assert_eq!(parse_price("abc"), None);
}

#[test]
fn parse_price_zero() {
    assert_eq!(parse_price("0"), Some(0.0));
}

#[test]
fn parse_price_negative() {
    assert_eq!(parse_price("-5.25"), Some(-5.25));
}

#[test]
fn parse_price_negative_with_currency() {
    assert_eq!(parse_price("$-3.10"), Some(-3.10));
}

#[test]
fn parse_price_integer() {
    assert_eq!(parse_price("100"), Some(100.0));
}

// ---- round_price ----

#[test]
fn round_price_rounds_up() {
    assert!((round_price(12.345) - 12.35).abs() < f64::EPSILON);
}

#[test]
fn round_price_rounds_down() {
    assert!((round_price(12.344) - 12.34).abs() < f64::EPSILON);
}

#[test]
fn round_price_zero() {
    assert!((round_price(0.0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn round_price_negative() {
    assert!((round_price(-3.456) - (-3.46)).abs() < f64::EPSILON);
}

#[test]
fn round_price_already_two_places() {
    assert!((round_price(10.50) - 10.50).abs() < f64::EPSILON);
}

#[test]
fn round_price_half_to_even_bankers() {
    let r = round_price(2.675);
    assert!((r - 2.68).abs() < 0.001);
}

// ---- extract_pct ----

#[test]
fn extract_pct_simple() {
    assert_eq!(extract_pct("30%"), Some(30.0));
}

#[test]
fn extract_pct_with_text() {
    assert_eq!(extract_pct("20% of portfolio"), Some(20.0));
}

#[test]
fn extract_pct_no_number() {
    assert_eq!(extract_pct("no number here"), None);
}

#[test]
fn extract_pct_empty() {
    assert_eq!(extract_pct(""), None);
}

#[test]
fn extract_pct_zero() {
    assert_eq!(extract_pct("0%"), None);
}

#[test]
fn extract_pct_decimal() {
    assert_eq!(extract_pct("12.5% upside"), Some(12.5));
}

#[test]
fn extract_pct_leading_text() {
    assert_eq!(extract_pct("risk is 45%"), Some(45.0));
}

// ---- fix_probabilities ----

#[test]
fn fix_probabilities_normalizes_sum_to_100() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 80.0;
    result.report.probability_view.downside_probability_pct = 60.0;
    result.report.probability_view.sideways_probability_pct = 60.0;

    let issues = fix_probabilities(&mut result);
    let pv = &result.report.probability_view;
    let sum = pv.upside_probability_pct + pv.downside_probability_pct + pv.sideways_probability_pct;
    assert!((sum - 100.0).abs() < 1.0, "sum should be ~100, got {sum}");
    assert!(!issues.is_empty());
}

#[test]
fn fix_probabilities_noop_when_sum_near_100() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 50.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 20.0;

    let issues = fix_probabilities(&mut result);
    assert!(issues.is_empty());
    assert!((result.report.probability_view.upside_probability_pct - 50.0).abs() < f64::EPSILON);
}

#[test]
fn fix_probabilities_clamps_risk_above_95() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 50.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 20.0;
    result.report.probability_view.risk_probability_pct = 99.0;

    let issues = fix_probabilities(&mut result);
    assert!(result.report.probability_view.risk_probability_pct <= 95.0);
    assert!(issues.iter().any(|i| i.check_name == "fix_risk_range"));
}

#[test]
fn fix_probabilities_clamps_risk_below_5() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 50.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 20.0;
    result.report.probability_view.risk_probability_pct = 2.0;

    let issues = fix_probabilities(&mut result);
    assert!(result.report.probability_view.risk_probability_pct >= 5.0);
    assert!(issues.iter().any(|i| i.check_name == "fix_risk_range"));
}

#[test]
fn fix_probabilities_risk_less_than_downside() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 50.0;
    result.report.probability_view.downside_probability_pct = 40.0;
    result.report.probability_view.sideways_probability_pct = 10.0;
    result.report.probability_view.risk_probability_pct = 20.0;

    let issues = fix_probabilities(&mut result);
    assert!(
        result.report.probability_view.risk_probability_pct
            >= result.report.probability_view.downside_probability_pct
    );
    assert!(issues.iter().any(|i| i.check_name == "fix_risk_invariant"));
}

#[test]
fn fix_probabilities_risk_zero_skipped() {
    let mut result = make_result();
    result.report.probability_view.upside_probability_pct = 50.0;
    result.report.probability_view.downside_probability_pct = 30.0;
    result.report.probability_view.sideways_probability_pct = 20.0;
    result.report.probability_view.risk_probability_pct = 0.0;

    let issues = fix_probabilities(&mut result);
    assert!(
        issues
            .iter()
            .all(|i| i.check_name != "fix_risk_range" && i.check_name != "fix_risk_invariant")
    );
}

// ---- fix_entry_stop ----

#[test]
fn fix_entry_stop_equal_prices() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "100.00".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(!issues.is_empty());
    let new_stop: f64 = result.report.trader_plan.stop_loss.parse().unwrap();
    assert!((new_stop - 98.00).abs() < 0.01);
}

#[test]
fn fix_entry_stop_nearly_equal_within_tolerance() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "99.95".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(!issues.is_empty());
}

#[test]
fn fix_entry_stop_different_prices_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(issues.is_empty());
    assert_eq!(result.report.trader_plan.stop_loss, "95.00");
}

#[test]
fn fix_entry_stop_empty_entry_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(issues.is_empty());
}

#[test]
fn fix_entry_stop_empty_stop_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(issues.is_empty());
}

#[test]
fn fix_entry_stop_does_not_update_invalidation_level() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "50.00".to_string();
    result.report.trader_plan.stop_loss = "50.00".to_string();
    result.report.decision_view.invalidation_level = "50.00".to_string();

    let issues = fix_entry_stop(&mut result);
    assert!(!issues.is_empty());
    // Invalidation level should NOT be updated here - it's derived from portfolio_decision
    let inv: f64 = result
        .report
        .decision_view
        .invalidation_level
        .parse()
        .unwrap();
    assert!((inv - 50.00).abs() < 0.01);
}

// ---- fix_risk_reward ----

#[test]
fn fix_risk_reward_low_rr_widens_target() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("102.00");

    let issues = fix_risk_reward(&mut result);
    assert!(!issues.is_empty());
    let new_target = &result.report.decision_view.target_reference;
    let val: f64 = new_target.as_str().parse().unwrap();
    assert!((val - 107.50).abs() < 0.01);
}

#[test]
fn fix_risk_reward_good_rr_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("110.00");

    let issues = fix_risk_reward(&mut result);
    assert!(issues.is_empty());
    assert_eq!(
        result.report.decision_view.target_reference.as_str(),
        "110.00"
    );
}

#[test]
fn fix_risk_reward_exact_15_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("107.50");

    let issues = fix_risk_reward(&mut result);
    assert!(issues.is_empty());
}

#[test]
fn fix_risk_reward_missing_target_noop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("");
    result.report.trader_plan.target_reference = "".to_string();

    let issues = fix_risk_reward(&mut result);
    assert!(issues.is_empty());
}

#[test]
fn fix_risk_reward_falls_back_to_trader_plan_target() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "100.00".to_string();
    result.report.trader_plan.stop_loss = "95.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("");
    result.report.trader_plan.target_reference = "102.00".to_string();

    let issues = fix_risk_reward(&mut result);
    assert!(!issues.is_empty());
}

#[test]
fn fix_risk_reward_entry_below_stop() {
    let mut result = make_result();
    result.report.trader_plan.entry_price = "95.00".to_string();
    result.report.trader_plan.stop_loss = "100.00".to_string();
    result.report.decision_view.target_reference = LocalText::new("97.00");

    let issues = fix_risk_reward(&mut result);
    assert!(!issues.is_empty());
}
