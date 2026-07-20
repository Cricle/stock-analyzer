//! Quality gate types and helpers (future work).
//!
//! Pre-LLM data quality gates are planned but not yet implemented.
//! This module retains types and helpers that may be used when gates are added.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityGateRejection {
    pub symbol: String,
    pub reason: String,
    pub missing_fields: Vec<String>,
    pub retry_attempted: bool,
    pub retry_error: Option<String>,
}

/// Check if candidate passes critical field requirements
#[allow(dead_code)]
fn check_critical_fields(
    _symbol: &str,
    price: Option<f64>,
    market_cap: Option<f64>,
    has_any_fundamental: bool,
    candles_len: usize,
) -> Result<(), Vec<String>> {
    let mut missing = Vec::new();

    if price.is_none() || price.unwrap_or(0.0) <= 0.0 {
        missing.push("price".to_string());
    }

    if market_cap.is_none() || market_cap.unwrap_or(0.0) <= 0.0 {
        missing.push("market_cap".to_string());
    }

    if !has_any_fundamental {
        missing.push("fundamentals".to_string());
    }

    if candles_len < 5 {
        missing.push(format!("candles (has {}, need ≥5)", candles_len));
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_critical_fields_pass() {
        let result = check_critical_fields("AAPL", Some(100.0), Some(1_000_000_000.0), true, 20);

        assert!(result.is_ok());
    }

    #[test]
    fn test_check_critical_fields_fail_no_price() {
        let result = check_critical_fields("AAPL", None, Some(1_000_000_000.0), true, 20);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"price".to_string()));
    }

    #[test]
    fn test_check_critical_fields_fail_zero_price() {
        let result = check_critical_fields("AAPL", Some(0.0), Some(1_000_000_000.0), true, 20);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"price".to_string()));
    }

    #[test]
    fn test_check_critical_fields_fail_no_market_cap() {
        let result = check_critical_fields("AAPL", Some(100.0), None, true, 20);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"market_cap".to_string()));
    }

    #[test]
    fn test_check_critical_fields_fail_no_fundamentals() {
        let result = check_critical_fields("AAPL", Some(100.0), Some(1_000_000_000.0), false, 20);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"fundamentals".to_string()));
    }

    #[test]
    fn test_check_critical_fields_fail_insufficient_candles() {
        let result = check_critical_fields("AAPL", Some(100.0), Some(1_000_000_000.0), true, 4);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.iter().any(|s| s.contains("candles")));
    }

    #[test]
    fn test_check_critical_fields_fail_multiple() {
        let result = check_critical_fields("AAPL", None, None, false, 0);

        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert_eq!(missing.len(), 4);
        assert!(missing.contains(&"price".to_string()));
        assert!(missing.contains(&"market_cap".to_string()));
        assert!(missing.contains(&"fundamentals".to_string()));
        assert!(missing.iter().any(|s| s.contains("candles")));
    }
}
