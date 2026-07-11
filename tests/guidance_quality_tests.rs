use stock_analyzer::guide::{PriceLevel, RiskAlert, StockGuidance, sentiment_label, sentiment_score};

#[test]
fn test_sentiment_score_calculation() {
    assert_eq!(sentiment_score(5, 1, 10), 40); // (5-1)/10 * 100
    assert_eq!(sentiment_score(1, 5, 10), -40);
    assert_eq!(sentiment_score(5, 5, 10), 0);
    assert_eq!(sentiment_score(1, 0, 2), 0); // Too few samples
}

#[test]
fn test_sentiment_labels() {
    assert_eq!(sentiment_label(50), "bullish");
    assert_eq!(sentiment_label(20), "slightly_bullish");
    assert_eq!(sentiment_label(0), "neutral");
    assert_eq!(sentiment_label(-20), "slightly_bearish");
    assert_eq!(sentiment_label(-50), "bearish");
}

#[test]
fn test_price_level_defaults() {
    let level = PriceLevel {
        price: 150.0,
        level_type: "support".to_string(),
        significance: "strong".to_string(),
    };
    assert_eq!(level.price, 150.0);
    assert_eq!(level.level_type, "support");
}

#[test]
fn test_stock_guidance_defaults() {
    let guidance = StockGuidance::default();
    assert!(guidance.symbol.is_empty());
    assert!(guidance.suggested_action.is_empty());
    assert!(guidance.key_levels.is_empty());
}

#[test]
fn test_risk_alert_structure() {
    let alert = RiskAlert {
        severity: "high".to_string(),
        category: "market_sentiment".to_string(),
        description: "Bearish market".to_string(),
        mitigation: "Reduce exposure".to_string(),
        affected_markets: vec!["us_equity".to_string()],
    };
    assert_eq!(alert.severity, "high");
    assert!(!alert.mitigation.is_empty());
}
