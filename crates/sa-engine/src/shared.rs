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
