use stock_analyzer::analysis::validation::check_consistency;

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
