use stock_analyzer::analysis::{StructuredPortfolioDecision, StructuredTraderPlan};
use stock_analyzer::analysis::{compute_reward_risk_hint, extract_first_price};

#[test]
fn extract_first_price_simple() {
    assert_eq!(extract_first_price("150"), Some(150.0));
}

#[test]
fn extract_first_price_decimal() {
    assert_eq!(extract_first_price("150.50"), Some(150.5));
}

#[test]
fn extract_first_price_with_text() {
    assert_eq!(extract_first_price("目标价160元"), Some(160.0));
}

#[test]
fn extract_first_price_empty() {
    assert_eq!(extract_first_price(""), None);
}

#[test]
fn extract_first_price_no_digits() {
    assert_eq!(extract_first_price("no numbers"), None);
}

#[test]
fn extract_first_price_with_percent() {
    // "上涨20%" - 20 is preceded by a Chinese char, not alphabetic, so it's extracted
    assert_eq!(extract_first_price("上涨20%"), Some(20.0));
}

#[test]
fn extract_first_price_skips_alpha_prefix() {
    assert_eq!(extract_first_price("x150"), None);
}

#[test]
fn extract_first_price_skips_chinese_period_suffix() {
    assert_eq!(extract_first_price("200日均线"), None);
}

#[test]
fn extract_first_price_skips_year_suffix() {
    assert_eq!(extract_first_price("2025年"), None);
}

#[test]
fn extract_first_price_allows_after_chinese() {
    assert_eq!(extract_first_price("目标价160"), Some(160.0));
}

#[test]
fn compute_reward_risk_hint_basic() {
    let trader = StructuredTraderPlan {
        entry_price: "150".into(),
        stop_loss: "145".into(),
        ..Default::default()
    };
    let portfolio = StructuredPortfolioDecision {
        price_target: "165".into(),
        ..Default::default()
    };
    let hint = compute_reward_risk_hint(&trader, &portfolio);
    // (165-150)/(150-145) = 15/5 = 3.0
    assert!(hint.is_some());
    assert!((hint.unwrap() - 3.0).abs() < 0.01);
}

#[test]
fn compute_reward_risk_hint_uses_confirmation_as_fallback() {
    let trader = StructuredTraderPlan {
        entry_price: "150".into(),
        stop_loss: "145".into(),
        ..Default::default()
    };
    let portfolio = StructuredPortfolioDecision {
        price_target: "".into(),
        confirmation_level: "160".into(),
        ..Default::default()
    };
    let hint = compute_reward_risk_hint(&trader, &portfolio);
    assert!(hint.is_some());
}

#[test]
fn compute_reward_risk_hint_missing_entry() {
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    assert!(compute_reward_risk_hint(&trader, &portfolio).is_none());
}

#[test]
fn compute_reward_risk_hint_entry_below_stop() {
    let trader = StructuredTraderPlan {
        entry_price: "145".into(),
        stop_loss: "150".into(),
        ..Default::default()
    };
    let portfolio = StructuredPortfolioDecision {
        price_target: "160".into(),
        ..Default::default()
    };
    assert!(compute_reward_risk_hint(&trader, &portfolio).is_none());
}
