use stock_analyzer::shared::safe_ticker_component;

#[test]
fn safe_ticker_component_basic() {
    assert_eq!(safe_ticker_component("AAPL", 10).unwrap(), "AAPL");
}

#[test]
fn safe_ticker_component_special_chars() {
    assert_eq!(safe_ticker_component("AAPL/US", 10).unwrap(), "AAPL_US");
    assert_eq!(safe_ticker_component("600519.SH", 20).unwrap(), "600519.SH");
}

#[test]
fn safe_ticker_component_truncate() {
    let result = safe_ticker_component("VERY_LONG_TICKER_NAME", 5).unwrap();
    assert_eq!(result, "VERY_");
}

#[test]
fn safe_ticker_component_empty() {
    assert!(safe_ticker_component("", 10).is_err());
    assert!(safe_ticker_component("  ", 10).is_err());
}

#[test]
fn safe_ticker_component_whitespace() {
    assert_eq!(safe_ticker_component(" AAPL ", 10).unwrap(), "AAPL");
}

#[test]
fn safe_ticker_component_chinese() {
    assert_eq!(safe_ticker_component("贵州茅台", 20).unwrap(), "____");
}

#[test]
fn safe_ticker_component_chinese_truncate() {
    let result = safe_ticker_component("贵州茅台", 3).unwrap();
    assert_eq!(result, "___");
}
