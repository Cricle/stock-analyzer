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
    if trimmed.len() > max_len {
        return Ok(trimmed[..max_len].to_string());
    }
    Ok(trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_http_client_returns_same_instance() {
        let c1 = shared_http_client();
        let c2 = shared_http_client();
        assert_eq!(c1.builder().build().unwrap().timeout(), c2.builder().build().unwrap().timeout());
    }

    #[test]
    fn safe_ticker_normal() {
        assert_eq!(safe_ticker_component("AAPL", 20).unwrap(), "AAPL");
    }

    #[test]
    fn safe_ticker_with_dots() {
        assert_eq!(safe_ticker_component("600519.SH", 20).unwrap(), "600519.SH");
    }

    #[test]
    fn safe_ticker_special_chars() {
        assert_eq!(safe_ticker_component("A/B C", 20).unwrap(), "A_B_C");
    }

    #[test]
    fn safe_ticker_empty_fails() {
        assert!(safe_ticker_component("", 20).is_err());
        assert!(safe_ticker_component("   ", 20).is_err());
    }

    #[test]
    fn safe_ticker_truncation() {
        assert_eq!(safe_ticker_component("VERYLONGTICKERNAME", 5).unwrap(), "VERYL");
    }

    #[test]
    fn safe_ticker_exact_max_len() {
        assert_eq!(safe_ticker_component("ABCDE", 5).unwrap(), "ABCDE");
    }
}
