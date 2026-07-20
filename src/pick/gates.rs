//! Quality gate types and validation logic.
//!
//! Pre-LLM data quality gates filter out candidates with missing critical data
//! before expensive LLM calls.

use serde::{Deserialize, Serialize};

use crate::pick::types::EnrichedCandidate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityGateRejection {
    pub symbol: String,
    pub reason: String,
    pub missing_fields: Vec<String>,
    pub retry_attempted: bool,
    pub retry_error: Option<String>,
}

/// Check if candidate passes critical field requirements
fn check_critical_fields(candidate: &EnrichedCandidate) -> Result<(), Vec<String>> {
    let mut missing = Vec::new();

    if candidate.price.is_none() || candidate.price.unwrap_or(0.0) <= 0.0 {
        missing.push("price".to_string());
    }

    if candidate.market_cap.is_none() || candidate.market_cap.unwrap_or(0.0) <= 0.0 {
        missing.push("market_cap".to_string());
    }

    if candidate.fundamentals.is_none() {
        missing.push("fundamentals".to_string());
    }

    if candidate.candles.len() < 5 {
        missing.push(format!("candles (has {}, need ≥5)", candidate.candles.len()));
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Apply pre-LLM quality gates to filter candidates with insufficient data.
///
/// Validates:
/// - price > 0
/// - market_cap exists and > 0
/// - fundamentals snapshot exists
/// - at least 5 candles for technical analysis
///
/// Returns (passed_candidates, rejected_records)
pub fn apply_quality_gates(
    candidates: Vec<EnrichedCandidate>,
    _analysis_date: &str,
) -> (Vec<EnrichedCandidate>, Vec<QualityGateRejection>) {
    let mut passed = Vec::new();
    let mut rejected = Vec::new();

    for candidate in candidates {
        match check_critical_fields(&candidate) {
            Ok(()) => {
                passed.push(candidate);
            }
            Err(missing_fields) => {
                let reason = format!("Critical data missing: {}", missing_fields.join(", "));
                rejected.push(QualityGateRejection {
                    symbol: candidate.symbol.clone(),
                    reason,
                    missing_fields,
                    retry_attempted: false,
                    retry_error: None,
                });
            }
        }
    }

    (passed, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{CandlePoint, FundamentalsSnapshot};
    use crate::pick::types::{EnrichedCandidate, FactorBreakdown};
    use crate::pick::provenance::ProvenanceSnapshot;
    use crate::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot,
        StockPickHistoryMatchSnapshot, StockPickMarketSnapshot, StockPickNewsSnapshot,
        StockPickRiskSnapshot, StockPickTechnicalSnapshot,
    };

    fn make_candle() -> CandlePoint {
        CandlePoint {
            trade_date: "2026-07-20".to_string(),
            open: 100.0,
            close: 101.0,
            high: 102.0,
            low: 99.0,
            volume: 1000000,
            amount: 100000000.0,
            amplitude_pct: 3.0,
            change_pct: 1.0,
            change_amount: 1.0,
            turnover_pct: 2.0,
        }
    }

    fn make_valid_candidate(symbol: &str) -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: symbol.to_string(),
            name: format!("{} Stock", symbol),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            industry: "Technology".to_string(),
            price: Some(100.0),
            change_pct: Some(1.5),
            market_cap: Some(1_000_000_000.0),
            theme_key: "tech".to_string(),
            fundamentals: Some(FundamentalsSnapshot::default()),
            analyst_consensus: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![make_candle(), make_candle(), make_candle(), make_candle(), make_candle()],
            technical_snapshot: StockPickTechnicalSnapshot::default(),
            market_snapshot: StockPickMarketSnapshot::default(),
            fundamental_snapshot: StockPickFundamentalSnapshot::default(),
            news_snapshot: StockPickNewsSnapshot::default(),
            history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
            risk_snapshot: StockPickRiskSnapshot::default(),
            data_quality_snapshot: StockPickDataQualitySnapshot::default(),
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: vec![],
            description: String::new(),
            provenance: ProvenanceSnapshot::default(),
        }
    }

    #[test]
    fn test_apply_quality_gates_filters_invalid_candidates() {
        let valid = make_valid_candidate("VALID");

        let mut no_price = make_valid_candidate("NOPRICE");
        no_price.price = None;

        let mut no_market_cap = make_valid_candidate("NOCAP");
        no_market_cap.market_cap = None;

        let mut no_fundamentals = make_valid_candidate("NOFUND");
        no_fundamentals.fundamentals = None;

        let mut insufficient_candles = make_valid_candidate("NOCANDLES");
        insufficient_candles.candles = vec![make_candle(), make_candle(), make_candle()];

        let candidates = vec![
            valid.clone(),
            no_price,
            no_market_cap,
            no_fundamentals,
            insufficient_candles,
        ];
        let (passed, rejected) = apply_quality_gates(candidates, "2026-07-20");

        assert_eq!(passed.len(), 1);
        assert_eq!(passed[0].symbol, "VALID");
        assert_eq!(rejected.len(), 4);

        let rejection_symbols: Vec<_> = rejected.iter().map(|r| r.symbol.as_str()).collect();
        assert!(rejection_symbols.contains(&"NOPRICE"));
        assert!(rejection_symbols.contains(&"NOCAP"));
        assert!(rejection_symbols.contains(&"NOFUND"));
        assert!(rejection_symbols.contains(&"NOCANDLES"));
    }

    #[test]
    fn test_quality_gate_rejection_reasons() {
        let mut no_market_cap = make_valid_candidate("NOCAP");
        no_market_cap.market_cap = None;

        let (_, rejected) = apply_quality_gates(vec![no_market_cap], "2026-07-20");

        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].reason.contains("market_cap"));
        assert_eq!(rejected[0].missing_fields, vec!["market_cap"]);
        assert!(!rejected[0].retry_attempted);
        assert!(rejected[0].retry_error.is_none());
    }

    #[test]
    fn test_quality_gate_zero_price_rejected() {
        let mut zero_price = make_valid_candidate("ZEROPRICE");
        zero_price.price = Some(0.0);

        let (passed, rejected) = apply_quality_gates(vec![zero_price], "2026-07-20");

        assert_eq!(passed.len(), 0);
        assert_eq!(rejected.len(), 1);
        assert!(rejected[0].missing_fields.contains(&"price".to_string()));
    }

    #[test]
    fn test_quality_gate_multiple_missing_fields() {
        let mut multiple_issues = make_valid_candidate("MULTI");
        multiple_issues.price = None;
        multiple_issues.market_cap = None;
        multiple_issues.fundamentals = None;
        multiple_issues.candles = vec![];

        let (_, rejected) = apply_quality_gates(vec![multiple_issues], "2026-07-20");

        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].missing_fields.len(), 4);
        assert!(rejected[0].missing_fields.contains(&"price".to_string()));
        assert!(rejected[0].missing_fields.contains(&"market_cap".to_string()));
        assert!(rejected[0]
            .missing_fields
            .contains(&"fundamentals".to_string()));
        assert!(rejected[0]
            .missing_fields
            .iter()
            .any(|s| s.contains("candles")));
    }

    #[test]
    fn test_quality_gate_all_pass() {
        let candidates = vec![
            make_valid_candidate("AAPL"),
            make_valid_candidate("MSFT"),
            make_valid_candidate("GOOGL"),
        ];

        let (passed, rejected) = apply_quality_gates(candidates, "2026-07-20");

        assert_eq!(passed.len(), 3);
        assert_eq!(rejected.len(), 0);
    }
}
