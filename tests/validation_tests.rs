use sa::analysis::validation::{check_consistency, check_execution_boundary, check_uniformity};

#[test]
fn consistency_flag_when_sell_with_oversold_rsi() {
    let result = check_consistency("Underweight", 25.0, "bullish_cross");
    assert!(result.consistency_flag);
    assert!(!result.consistency_reason.is_empty());
    assert!(result.confidence_adjustment < 0);
}

#[test]
fn no_consistency_flag_when_sell_with_bearish_indicators() {
    let result = check_consistency("Underweight", 55.0, "bearish_cross");
    assert!(!result.consistency_flag);
    assert_eq!(result.confidence_adjustment, 0);
}

#[test]
fn consistency_flag_when_buy_with_overbought_rsi() {
    let result = check_consistency("Overweight", 75.0, "bearish_cross");
    assert!(result.consistency_flag);
    assert!(result.confidence_adjustment < 0);
}

#[test]
fn uniformity_flag_when_outputs_are_identical() {
    let stocks = vec![
        ("StockA", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockB", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockC", "100.0", "95.0", "5%", "2-4 weeks"),
    ];
    let result = check_uniformity(&stocks);
    assert!(result.uniformity_flag);
    assert!(result.uniformity_pct > 70.0);
    assert!(result.action_adjustment < 0);
}

#[test]
fn no_uniformity_flag_when_outputs_differ() {
    let stocks = vec![
        ("StockA", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockB", "50.0", "47.0", "3%", "1-3 months"),
        ("StockC", "200.0", "190.0", "8%", "3-6 months"),
    ];
    let result = check_uniformity(&stocks);
    assert!(!result.uniformity_flag);
    assert!(result.uniformity_pct < 70.0);
    assert_eq!(result.action_adjustment, 0);
}

#[test]
fn missing_fields_detected_for_sell_recommendation() {
    let result = check_execution_boundary("Underweight", "", "95.0", "", "100.0");
    assert!(!result.missing_boundary_fields.is_empty());
    assert!(result.missing_boundary_fields.contains(&"entry_price".to_string()));
}

#[test]
fn no_missing_fields_when_all_present() {
    let result = check_execution_boundary("Underweight", "100.0", "95.0", "105.0", "90.0");
    assert!(result.missing_boundary_fields.is_empty());
}

#[test]
fn no_missing_fields_for_hold() {
    let result = check_execution_boundary("Hold", "", "", "", "");
    assert!(result.missing_boundary_fields.is_empty());
}
