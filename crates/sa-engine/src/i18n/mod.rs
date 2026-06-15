//! Internationalization support with bundled locale JSON files.

use std::collections::HashMap;

use serde_json::Value;

/// Loads bundled zh/en locale files at compile time and resolves
/// dot-separated translation keys with optional `{param}` interpolation.
pub struct I18n {
    locales: HashMap<String, Value>,
}

impl I18n {
    /// Build a new `I18n` instance with the bundled zh and en locales.
    pub fn new() -> Self {
        let zh: Value =
            serde_json::from_str(include_str!("locales/zh.json")).expect("invalid zh.json");
        let en: Value =
            serde_json::from_str(include_str!("locales/en.json")).expect("invalid en.json");

        let mut locales = HashMap::new();
        locales.insert("zh".to_string(), zh);
        locales.insert("en".to_string(), en);
        Self { locales }
    }

    /// Look up a dot-separated key (e.g. `"report.summary.title"`) in the
    /// given language. Returns `None` if the language or key path is missing.
    pub fn resolve(&self, key: &str, lang: &str) -> Option<String> {
        let locale = self.locales.get(lang)?;
        let mut current = locale;
        for segment in key.split('.') {
            match current {
                Value::Object(map) => {
                    current = map.get(segment)?;
                }
                _ => return None,
            }
        }
        current.as_str().map(String::from)
    }

    /// Like [`resolve`](Self::resolve), but replaces `{param_name}` and
    /// `{param_name:format}` placeholders in the resolved string with values
    /// from `params`.  Format specifiers like `:.1` are applied to numeric values.
    pub fn resolve_with_params(
        &self,
        key: &str,
        lang: &str,
        params: &serde_json::Map<String, Value>,
    ) -> Option<String> {
        let template = self.resolve(key, lang)?;
        let re = regex::Regex::new(r"\{(\w+)(?::([^}]+))?\}").ok()?;
        let result = re.replace_all(&template, |caps: &regex::Captures| {
            let name = &caps[1];
            let fmt = caps.get(2).map(|m| m.as_str());
            match params.get(name) {
                Some(value) => format_value(value, fmt),
                None => caps[0].to_string(),
            }
        });
        Some(result.into_owned())
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a JSON value with an optional Rust-style format specifier.
/// Supports: `:.N` (N decimal places), `+.N` (signed, N decimal places), `:N` (zero-padded integer).
fn format_value(value: &Value, fmt: Option<&str>) -> String {
    match (value, fmt) {
        (Value::Number(n), Some(spec)) => {
            let f = match n.as_f64() {
                Some(f) => f,
                None => return n.to_string(),
            };
            // Parse optional `+` flag and precision.
            let (sign, rest) = if let Some(r) = spec.strip_prefix('+') {
                (true, r)
            } else {
                (false, spec)
            };
            if let Some(precision) = rest.strip_prefix('.')
                && let Ok(p) = precision.trim_end_matches('f').parse::<usize>()
            {
                if sign {
                    return format!("{:+.p$}", f, p = p);
                }
                return format!("{:.p$}", f, p = p);
            }
            if sign {
                return format!("{:+}", f);
            }
            n.to_string()
        }
        (Value::String(s), _) => s.clone(),
        (Value::Number(n), None) => {
            if let Some(f) = n.as_f64()
                && f == f.floor()
                && f.abs() < 1e15
            {
                return format!("{}", f as i64);
            }
            n.to_string()
        }
        (other, _) => other.to_string(),
    }
}

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
        let result = i18n.resolve_with_params("stock_pick.catalyst.strong_return", "zh", &params);
        assert_eq!(result.as_deref(), Some("区间涨幅达12.3%，动量强劲"));
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
