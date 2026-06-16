#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_zh_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "zh");
        assert_eq!(result.as_deref(), Some("分析摘要"));
    }

    #[test]
    fn resolve_en_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "en");
        assert_eq!(result.as_deref(), Some("Analysis Summary"));
    }

    #[test]
    fn resolve_missing_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("nonexistent.key", "zh");
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_with_params() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("symbol".to_string(), Value::String("AAPL".to_string()));
        let result = i18n.resolve_with_params("report.header.for_symbol", "zh", &params);
        assert_eq!(result.as_deref(), Some("AAPL 的分析报告"));
    }

    #[test]
    fn resolve_with_format_specifiers() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("pct".to_string(), Value::from(12.345));
        params.insert("days".to_string(), Value::from(30));
        let result = i18n.resolve_with_params("stock_pick.catalyst.strong_return", "zh", &params);
        assert_eq!(result.as_deref(), Some("近30日区间涨幅达12.3%，动量强劲"));
    }

    #[test]
    fn resolve_with_format_specifiers_rsi() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("rsi".to_string(), Value::from(62.5));
        let result = i18n.resolve_with_params("stock_pick.catalyst.rsi_bullish", "zh", &params);
        assert_eq!(result.as_deref(), Some("RSI为62.5，显示看多动量且未超买"));
    }

    #[test]
    fn format_value_integer_no_trailing_zeros() {
        let v = Value::from(60);
        assert_eq!(format_value(&v, None), "60");
    }

    #[test]
    fn format_value_float_one_decimal() {
        let v = Value::from(12.345);
        assert_eq!(format_value(&v, Some(".1")), "12.3");
    }

    #[test]
    fn format_value_signed_positive() {
        let v = Value::from(15.8);
        assert_eq!(format_value(&v, Some("+.1")), "+15.8");
    }

    #[test]
    fn format_value_signed_negative() {
        let v = Value::from(-15.8);
        assert_eq!(format_value(&v, Some("+.1")), "-15.8");
    }

    #[test]
    fn resolve_with_plus_format_specifier() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("ret".to_string(), Value::from(-15.8059));
        params.insert("start_price".to_string(), Value::from(1800.0));
        params.insert("end_price".to_string(), Value::from(1515.54));
        params.insert("name".to_string(), Value::from("贵州茅台".to_string()));
        params.insert("symbol".to_string(), Value::from("600519".to_string()));
        params.insert("direction".to_string(), Value::from("下跌".to_string()));
        let result = i18n.resolve_with_params("stock_pick.thesis.price_action", "zh", &params);
        let text = result.unwrap();
        assert!(text.contains("-15.8"), "should format with sign, got: {text}");
        assert!(!text.contains("+."), "should not have stray +., got: {text}");
    }
}
