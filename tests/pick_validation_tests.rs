use sa::StockPickTechnicalSnapshot;
use sa::pick::validation::{PickQualityGate, apply_defaults, validate_pick};
use sa::pick::{EnrichedCandidate, FactorBreakdown};

fn make_candidate(price: Option<f64>, atr: Option<f64>) -> EnrichedCandidate {
    let mut technical = StockPickTechnicalSnapshot::default();
    technical.atr = atr;

    EnrichedCandidate {
        symbol: "TEST".to_string(),
        name: "Test Corp".to_string(),
        market: "us_equity".to_string(),
        exchange: "NASDAQ".to_string(),
        industry: "tech".to_string(),
        price,
        change_pct: Some(1.5),
        market_cap: Some(1e9),
        theme_key: "tech".to_string(),
        fundamentals: None,
        news: vec![],
        evidence_records: vec![],
        candles: vec![],
        technical_snapshot: technical,
        market_snapshot: Default::default(),
        fundamental_snapshot: Default::default(),
        news_snapshot: Default::default(),
        history_match_snapshot: Default::default(),
        risk_snapshot: Default::default(),
        data_quality_snapshot: Default::default(),
        factor: FactorBreakdown::default(),
        pass_filter: true,
        rejected_reasons: vec![],
        description: String::new(),
    }
}

fn make_pick(
    entry: Option<&str>,
    stop: Option<&str>,
    target: Option<&str>,
    catalysts: Vec<&str>,
    exit_triggers: Vec<&str>,
) -> sa::pick::types::GeneratedStockPickItem {
    use serde_json::Value;
    sa::pick::types::GeneratedStockPickItem {
        symbol: "TEST".to_string(),
        confidence: Value::from(0.7),
        thesis: "Test thesis".to_string(),
        catalysts: catalysts.into_iter().map(String::from).collect(),
        risks: vec![],
        evidence_points: vec![],
        decision_reason_codes: vec![],
        data_gaps: vec![],
        entry_price: entry.map(String::from),
        stop_loss: stop.map(String::from),
        target_price: target.map(String::from),
        holding_period: None,
        exit_triggers: exit_triggers.into_iter().map(String::from).collect(),
    }
}

#[test]
fn test_valid_pick_passes_validation() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("115"),
        vec!["earnings beat"],
        vec!["break below 95"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(validation.is_valid);
    assert!((validation.risk_reward_ratio - 3.0).abs() < 0.01);
}

#[test]
fn test_missing_fields_fails_validation() {
    let pick = make_pick(None, None, None, vec![], vec![]);
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
    assert!(validation.issues.len() >= 3);
}

#[test]
fn test_stop_above_entry_fails() {
    let pick = make_pick(
        Some("100"),
        Some("105"),
        Some("115"),
        vec!["catalyst"],
        vec!["break below 105"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
}

#[test]
fn test_low_rr_fails() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("102"),
        vec!["catalyst"],
        vec!["break below 95"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
}

#[test]
fn test_defaults_applied() {
    let mut pick = make_pick(None, None, None, vec![], vec![]);
    let candidate = make_candidate(Some(100.0), Some(2.0));
    apply_defaults(&mut pick, &candidate);
    assert_eq!(pick.entry_price, Some("100.00".to_string()));
    assert_eq!(pick.stop_loss, Some("96.00".to_string())); // 100 - 2*2
    assert_eq!(pick.target_price, Some("112.00".to_string())); // 100 + 3*4
    assert!(!pick.exit_triggers.is_empty());
}

#[test]
fn test_custom_config() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("110"),
        vec!["catalyst"],
        vec!["break below 95"],
    );
    let config = PickQualityGate {
        min_risk_reward_ratio: 2.0,
        require_catalyst: true,
        require_exit_strategy: true,
        max_stop_loss_pct: 10.0,
    };
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(validation.is_valid);
}

#[test]
fn test_defaults_without_atr() {
    let mut pick = make_pick(None, None, None, vec![], vec![]);
    let candidate = make_candidate(Some(100.0), None);
    apply_defaults(&mut pick, &candidate);
    assert_eq!(pick.entry_price, Some("100.00".to_string()));
    assert_eq!(pick.stop_loss, Some("95.00".to_string())); // 100 * 0.95
    assert_eq!(pick.target_price, Some("115.00".to_string())); // 100 + 3*5
}

#[test]
fn test_defaults_with_partial_fields() {
    let mut pick = make_pick(Some("100"), None, None, vec!["catalyst"], vec![]);
    let candidate = make_candidate(Some(105.0), Some(3.0));
    apply_defaults(&mut pick, &candidate);
    assert_eq!(pick.entry_price, Some("100".to_string())); // Keep existing
    assert_eq!(pick.stop_loss, Some("94.00".to_string())); // 100 - 2*3
    assert_eq!(pick.target_price, Some("118.00".to_string())); // 100 + 3*6
}
