//! Shared utilities ported from the backend.

use std::sync::OnceLock;

/// Shared HTTP client reused across the process to enable connection pooling.
pub fn shared_http_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| reqwest::Client::builder().build().unwrap_or_default())
        .clone()
}

/// Sanitize a ticker into a safe path component.
pub fn safe_ticker_component(value: &str, max_len: usize) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("ticker must be a non-empty string");
    }
    let sanitized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.len() > max_len {
        return Ok(sanitized[..max_len].to_string());
    }
    Ok(sanitized)
}

#[cfg(test)]
mod shared_tests {
    use super::*;

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
        // 贵州茅台 → "____" (4 underscores), truncate to 3
        let result = safe_ticker_component("贵州茅台", 3).unwrap();
        assert_eq!(result, "___");
    }
}
