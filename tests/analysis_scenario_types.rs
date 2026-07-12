use stock_analyzer::analysis::{
    AnalysisScenarioContext, AnalysisScenarioData, AnalysisScenarioMarket,
};

// --- AnalysisScenarioMarket::from_market_type ---

#[test]
fn analysis_scenario_market_a_share() {
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("A股"),
        AnalysisScenarioMarket::AShare
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("a_share"),
        AnalysisScenarioMarket::AShare
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("a-share"),
        AnalysisScenarioMarket::AShare
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("cn"),
        AnalysisScenarioMarket::AShare
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("china"),
        AnalysisScenarioMarket::AShare
    );
}

#[test]
fn analysis_scenario_market_hong_kong() {
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("港股"),
        AnalysisScenarioMarket::HongKong
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("hk"),
        AnalysisScenarioMarket::HongKong
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("hongkong"),
        AnalysisScenarioMarket::HongKong
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("hong_kong"),
        AnalysisScenarioMarket::HongKong
    );
}

#[test]
fn analysis_scenario_market_us_equity() {
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("美股"),
        AnalysisScenarioMarket::UsEquity
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("us"),
        AnalysisScenarioMarket::UsEquity
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("us_equity"),
        AnalysisScenarioMarket::UsEquity
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("usa"),
        AnalysisScenarioMarket::UsEquity
    );
}

#[test]
fn analysis_scenario_market_unknown() {
    assert_eq!(
        AnalysisScenarioMarket::from_market_type("unknown"),
        AnalysisScenarioMarket::Unknown
    );
    assert_eq!(
        AnalysisScenarioMarket::from_market_type(""),
        AnalysisScenarioMarket::Unknown
    );
}

// --- AnalysisScenarioMarket::key ---

#[test]
fn analysis_scenario_market_key() {
    assert_eq!(AnalysisScenarioMarket::AShare.key(), "a_share");
    assert_eq!(AnalysisScenarioMarket::HongKong.key(), "hk_equity");
    assert_eq!(AnalysisScenarioMarket::UsEquity.key(), "us_equity");
    assert_eq!(AnalysisScenarioMarket::Unknown.key(), "unknown");
}

// --- AnalysisScenarioMarket::label ---

#[test]
fn analysis_scenario_market_label() {
    assert_eq!(AnalysisScenarioMarket::AShare.label(), "A股");
    assert_eq!(AnalysisScenarioMarket::HongKong.label(), "港股");
    assert_eq!(AnalysisScenarioMarket::UsEquity.label(), "美股");
    assert_eq!(AnalysisScenarioMarket::Unknown.label(), "未知市场");
}

// --- AnalysisScenarioMarket supports_* ---

#[test]
fn analysis_scenario_market_supports_company_news() {
    assert!(AnalysisScenarioMarket::AShare.supports_company_news());
    assert!(AnalysisScenarioMarket::HongKong.supports_company_news());
    assert!(AnalysisScenarioMarket::UsEquity.supports_company_news());
    assert!(!AnalysisScenarioMarket::Unknown.supports_company_news());
}

#[test]
fn analysis_scenario_market_supports_global_news() {
    assert!(AnalysisScenarioMarket::AShare.supports_global_news());
    assert!(AnalysisScenarioMarket::HongKong.supports_global_news());
    assert!(AnalysisScenarioMarket::UsEquity.supports_global_news());
    assert!(!AnalysisScenarioMarket::Unknown.supports_global_news());
}

#[test]
fn analysis_scenario_market_supports_insider_transactions() {
    assert!(AnalysisScenarioMarket::AShare.supports_insider_transactions());
    assert!(AnalysisScenarioMarket::HongKong.supports_insider_transactions());
    assert!(AnalysisScenarioMarket::UsEquity.supports_insider_transactions());
    assert!(!AnalysisScenarioMarket::Unknown.supports_insider_transactions());
}

// --- AnalysisScenarioContext::from_market_type ---

#[test]
fn analysis_scenario_context_a_share() {
    let ctx = AnalysisScenarioContext::from_market_type("A股");
    assert_eq!(ctx.market, AnalysisScenarioMarket::AShare);
    assert_eq!(ctx.market_key, "a_share");
    assert_eq!(ctx.market_label, "A股");
    assert!(ctx.supports_company_news);
    assert!(ctx.supports_global_news);
    assert!(ctx.supports_insider_transactions);
}

#[test]
fn analysis_scenario_context_unknown() {
    let ctx = AnalysisScenarioContext::from_market_type("");
    assert_eq!(ctx.market, AnalysisScenarioMarket::Unknown);
    assert_eq!(ctx.market_key, "unknown");
    assert_eq!(ctx.market_label, "未知市场");
    assert!(!ctx.supports_company_news);
}

// --- AnalysisScenarioData::add_issue ---

#[test]
fn analysis_scenario_data_add_issue() {
    let mut data = AnalysisScenarioData::default();
    data.add_issue("quote", "fetch_failed", "warning", "timeout");
    assert_eq!(data.issues.len(), 1);
    assert_eq!(data.issues[0].domain, "quote");
    assert_eq!(data.issues[0].code, "fetch_failed");
    assert_eq!(data.issues[0].severity, "warning");
    assert_eq!(data.issues[0].message, "timeout");
}

#[test]
fn analysis_scenario_data_add_multiple_issues() {
    let mut data = AnalysisScenarioData::default();
    data.add_issue("quote", "q1", "warning", "msg1");
    data.add_issue("candles", "c1", "error", "msg2");
    assert_eq!(data.issues.len(), 2);
}

// --- serde roundtrip ---

#[test]
fn analysis_scenario_market_serde_roundtrip() {
    let markets = [
        AnalysisScenarioMarket::AShare,
        AnalysisScenarioMarket::HongKong,
        AnalysisScenarioMarket::UsEquity,
        AnalysisScenarioMarket::Unknown,
    ];
    for market in &markets {
        let json = serde_json::to_string(market).unwrap();
        let restored: AnalysisScenarioMarket = serde_json::from_str(&json).unwrap();
        assert_eq!(*market, restored);
    }
}

#[test]
fn analysis_scenario_context_serde_roundtrip() {
    let ctx = AnalysisScenarioContext::from_market_type("A股");
    let json = serde_json::to_string(&ctx).unwrap();
    let restored: AnalysisScenarioContext = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.market, ctx.market);
    assert_eq!(restored.market_key, ctx.market_key);
}
