//! Shared utilities ported from the backend.

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
