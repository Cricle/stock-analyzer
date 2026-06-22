#[test]
fn e2e_rating_types() {
    use sa_models::Rating;

    let buy = Rating::parse("buy");
    assert!(buy.is_bullish());
    assert!(!buy.is_bearish());
    assert!(!buy.is_neutral());

    let sell = Rating::parse("sell");
    assert!(!sell.is_bullish());
    assert!(sell.is_bearish());
    assert!(!sell.is_neutral());

    let hold = Rating::parse("hold");
    assert!(!hold.is_bullish());
    assert!(!hold.is_bearish());
    assert!(hold.is_neutral());
}

#[test]
fn e2e_rating_bias_scoring() {
    use sa_models::Rating;

    assert_eq!(Rating::Buy.bias(100), 100);
    assert_eq!(Rating::Sell.bias(100), -100);
    assert_eq!(Rating::Hold.bias(100), 0);

    assert_eq!(Rating::Buy.to_score(), 2);
    assert_eq!(Rating::Sell.to_score(), -2);
    assert_eq!(Rating::Hold.to_score(), 0);
}

#[test]
fn e2e_local_text_i18n() {
    use sa_models::LocalText;

    let lt = LocalText::new("setup_gap_missing_data")
        .with_str("field", "cash_flow")
        .with_f64("threshold", 0.8);

    assert_eq!(lt.as_str(), "setup_gap_missing_data");
    assert!(!lt.is_empty());
    assert!(lt.contains("missing"));
    assert!(lt.starts_with("setup_gap"));
}

#[test]
fn e2e_scenario_market_classification() {
    use sa_models::AnalysisScenarioMarket;

    assert_eq!(
        AnalysisScenarioMarket::from_market_type("A股"),
        AnalysisScenarioMarket::AShare
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("美股"),
        AnalysisScenarioMarket::UsEquity
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("港股"),
        AnalysisScenarioMarket::HongKong
    );

    let a_share = AnalysisScenarioMarket::AShare;
    assert!(a_share.supports_company_news());
    assert_eq!(a_share.key(), "a_share");
    assert_eq!(a_share.label(), "A股");
}

#[test]
fn e2e_scenario_context_construction() {
    use sa_models::AnalysisScenarioContext;

    let ctx = AnalysisScenarioContext::from_market_type("A股");
    assert!(ctx.supports_company_news);
    assert!(ctx.supports_global_news);
    assert!(ctx.supports_insider_transactions);
    assert_eq!(ctx.market_key, "a_share");
}
