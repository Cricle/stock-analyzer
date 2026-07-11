use stock_analyzer::data::{BillboardEntry, CapitalFlowPoint, MarketKind};
use stock_analyzer::pick::pipeline::filter::{
    billboard_source_score, capital_flow_source_score, market_display_label, market_exchange_code,
    market_kind_from_value, market_search_label,
};

// --- market_kind_from_value ---

#[test]
fn market_kind_a_share_variants() {
    assert_eq!(market_kind_from_value("a"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("A-share"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("a_share"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("ashare"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("cn"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("china"), MarketKind::AShare);
    assert_eq!(market_kind_from_value("a股"), MarketKind::AShare);
}

#[test]
fn market_kind_hong_kong_variants() {
    assert_eq!(market_kind_from_value("hk"), MarketKind::HongKong);
    assert_eq!(market_kind_from_value("HKEX"), MarketKind::HongKong);
    assert_eq!(market_kind_from_value("hongkong"), MarketKind::HongKong);
    assert_eq!(market_kind_from_value("hong_kong"), MarketKind::HongKong);
    assert_eq!(market_kind_from_value("港股"), MarketKind::HongKong);
}

#[test]
fn market_kind_us_equity_default() {
    assert_eq!(market_kind_from_value("us"), MarketKind::UsEquity);
    assert_eq!(market_kind_from_value("US"), MarketKind::UsEquity);
    assert_eq!(market_kind_from_value("random"), MarketKind::UsEquity);
    assert_eq!(market_kind_from_value(""), MarketKind::UsEquity);
}

#[test]
fn market_kind_from_value_whitespace_trimmed() {
    assert_eq!(market_kind_from_value("  hk  "), MarketKind::HongKong);
    assert_eq!(market_kind_from_value("  CN  "), MarketKind::AShare);
}

// --- market_display_label ---

#[test]
fn display_labels() {
    assert_eq!(market_display_label(MarketKind::AShare), "A-share");
    assert_eq!(market_display_label(MarketKind::HongKong), "HK");
    assert_eq!(market_display_label(MarketKind::UsEquity), "US");
}

// --- market_search_label ---

#[test]
fn search_label_matches_display() {
    assert_eq!(market_search_label(MarketKind::AShare), "A-share");
    assert_eq!(market_search_label(MarketKind::HongKong), "HK");
    assert_eq!(market_search_label(MarketKind::UsEquity), "US");
}

// --- market_exchange_code ---

#[test]
fn exchange_codes() {
    assert_eq!(market_exchange_code(MarketKind::AShare), "CN");
    assert_eq!(market_exchange_code(MarketKind::HongKong), "HK");
    assert_eq!(market_exchange_code(MarketKind::UsEquity), "US");
}

// --- capital_flow_source_score ---

fn make_capital_flow(main_net_inflow: f64, ratio_pct: f64, change_pct: f64) -> CapitalFlowPoint {
    CapitalFlowPoint {
        trade_date: "2024-01-01".to_string(),
        main_net_inflow,
        small_net_inflow: 0.0,
        medium_net_inflow: 0.0,
        large_net_inflow: 0.0,
        super_large_net_inflow: 0.0,
        main_net_inflow_ratio_pct: ratio_pct,
        small_net_inflow_ratio_pct: 0.0,
        medium_net_inflow_ratio_pct: 0.0,
        large_net_inflow_ratio_pct: 0.0,
        super_large_net_inflow_ratio_pct: 0.0,
        close: 10.0,
        change_pct,
    }
}

#[test]
fn capital_flow_empty_returns_zero() {
    assert_eq!(capital_flow_source_score(&[]), 0.0);
}

#[test]
fn capital_flow_positive_inflow() {
    let items = vec![make_capital_flow(500_000_000.0, 5.0, 2.0)];
    let score = capital_flow_source_score(&items);
    assert!((score - 7.75).abs() < 0.01);
}

#[test]
fn capital_flow_negative_inflow() {
    let items = vec![make_capital_flow(-300_000_000.0, -5.0, -3.0)];
    let score = capital_flow_source_score(&items);
    assert!((score - (-6.25)).abs() < 0.01);
}

#[test]
fn capital_flow_clamp_extreme_values() {
    let items = vec![make_capital_flow(2_000_000_000.0, 50.0, 20.0)];
    let score = capital_flow_source_score(&items);
    assert!((score - 25.0).abs() < 0.01);
}

#[test]
fn capital_flow_uses_first_item() {
    let items = vec![
        make_capital_flow(100_000_000.0, 1.0, 1.0),
        make_capital_flow(900_000_000.0, 9.0, 5.0),
    ];
    let score = capital_flow_source_score(&items);
    assert!((score - 1.85).abs() < 0.01);
}

// --- billboard_source_score ---

fn make_billboard(
    net_amount: Option<f64>,
    turnover_rate_pct: Option<f64>,
    change_rate_pct: f64,
) -> BillboardEntry {
    BillboardEntry {
        trade_date: "2024-01-01".to_string(),
        symbol: "SYM".to_string(),
        name: "Test".to_string(),
        close_price: 10.0,
        change_rate_pct,
        turnover_rate_pct,
        net_amount,
        buy_amount: None,
        sell_amount: None,
        explanation: None,
        reason: None,
    }
}

#[test]
fn billboard_empty_returns_zero() {
    assert_eq!(billboard_source_score(&[]), 0.0);
}

#[test]
fn billboard_with_net_amount() {
    let items = vec![make_billboard(Some(5000_0000.0), Some(10.0), 3.0)];
    let score = billboard_source_score(&items);
    assert!((score - 5.2).abs() < 0.01);
}

#[test]
fn billboard_no_net_amount_defaults() {
    let items = vec![make_billboard(None, Some(5.0), 2.0)];
    let score = billboard_source_score(&items);
    assert!((score - 5.05).abs() < 0.01);
}

#[test]
fn billboard_no_turnover_defaults_to_zero() {
    let items = vec![make_billboard(Some(1000_0000.0), None, 1.0)];
    let score = billboard_source_score(&items);
    assert!((score - 2.5).abs() < 0.01);
}

#[test]
fn billboard_negative_change_penalty() {
    let items = vec![make_billboard(Some(-2000_0000.0), Some(3.0), -4.0)];
    let score = billboard_source_score(&items);
    assert!((score - 0.65).abs() < 0.01);
}
