//! Reasoning consistency validation — verifies LLM claims match actual data.

use crate::StockPickItem;
use crate::pick::types::EnrichedCandidate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningConsistencyCheck {
    pub claim: String,
    pub expected_condition: String,
    pub actual_value: Option<f64>,
    pub is_consistent: bool,
    pub severity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ReasoningConsistencyReport {
    pub checks: Vec<ReasoningConsistencyCheck>,
    pub consistency_score: i32,
    pub major_violations: usize,
    pub minor_violations: usize,
}

/// Parse technical claims from text
fn parse_technical_claims(text: &str) -> Vec<String> {
    let mut claims = Vec::new();
    let lower = text.to_lowercase();

    if lower.contains("rsi") && (lower.contains("overbought") || lower.contains("oversold")) {
        claims.push("RSI overbought/oversold".to_string());
    }
    if lower.contains("macd")
        && (lower.contains("golden") || lower.contains("bullish") || lower.contains("cross"))
    {
        claims.push("MACD cross".to_string());
    }
    if lower.contains("bollinger") {
        claims.push("Bollinger bands".to_string());
    }

    claims
}

/// Parse price claims from text
fn parse_price_claims(text: &str) -> Vec<String> {
    let mut claims = Vec::new();
    let lower = text.to_lowercase();

    if lower.contains("near 52-week high") || lower.contains("52 week high") {
        claims.push("Near 52-week high".to_string());
    }
    if lower.contains("near 52-week low") || lower.contains("52 week low") {
        claims.push("Near 52-week low".to_string());
    }
    if lower.contains("breakout") || lower.contains("all-time high") {
        claims.push("Breakout/ATH".to_string());
    }

    claims
}

/// Parse fundamental claims from text
fn parse_fundamental_claims(text: &str) -> Vec<String> {
    let mut claims = Vec::new();
    let lower = text.to_lowercase();

    if lower.contains("profitable") || lower.contains("positive earnings") {
        claims.push("Profitable".to_string());
    }
    if lower.contains("revenue growth") || lower.contains("growing revenue") {
        claims.push("Revenue growth".to_string());
    }
    if lower.contains("undervalued") || lower.contains("low pe") || lower.contains("low p/e") {
        claims.push("Undervalued".to_string());
    }

    claims
}

/// Validate reasoning consistency
pub fn validate_reasoning_consistency(
    pick: &StockPickItem,
    candidate: &EnrichedCandidate,
) -> ReasoningConsistencyReport {
    let mut checks = Vec::new();
    let mut major_violations = 0;
    let mut minor_violations = 0;

    // Collect all text to analyze
    let thesis_text = format!("{:?}", pick.thesis);
    let evidence_text = pick.evidence_points.join(" ");
    let rationale_text = format!(
        "{} {} {}",
        pick.entry_rationale.as_deref().unwrap_or(""),
        pick.stop_rationale.as_deref().unwrap_or(""),
        pick.target_rationale.as_deref().unwrap_or("")
    );
    let all_text = format!("{} {} {}", thesis_text, evidence_text, rationale_text);

    // Check technical claims
    let tech_claims = parse_technical_claims(&all_text);
    for claim in tech_claims {
        if claim.contains("RSI overbought") || claim.contains("RSI oversold") {
            let rsi = candidate.technical_snapshot.rsi;
            let lower = all_text.to_lowercase();

            let (expected, is_consistent) = if lower.contains("overbought") {
                ("RSI > 70".to_string(), rsi.map_or(false, |v| v > 70.0))
            } else if lower.contains("oversold") {
                ("RSI < 30".to_string(), rsi.map_or(false, |v| v < 30.0))
            } else {
                (
                    "RSI overbought (>70) or oversold (<30)".to_string(),
                    rsi.map_or(false, |v| v > 70.0 || v < 30.0),
                )
            };

            if !is_consistent && rsi.is_some() {
                major_violations += 1;
            }

            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: expected,
                actual_value: rsi,
                is_consistent,
                severity: if is_consistent {
                    "info".to_string()
                } else {
                    "major".to_string()
                },
            });
        }

        if claim.contains("MACD") {
            let macd = candidate.technical_snapshot.macd;
            let macd_signal = candidate.technical_snapshot.macd_signal;

            let is_consistent = if let (Some(m), Some(s)) = (macd, macd_signal) {
                m > s // Bullish cross
            } else {
                false
            };

            if !is_consistent && macd.is_some() && macd_signal.is_some() {
                minor_violations += 1;
            }

            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: "MACD > Signal".to_string(),
                actual_value: macd,
                is_consistent,
                severity: if is_consistent {
                    "info".to_string()
                } else {
                    "minor".to_string()
                },
            });
        }
    }

    // Check price claims
    let price_claims = parse_price_claims(&all_text);
    for claim in price_claims {
        // For 52-week high/low, we'd need historical data which isn't in the snapshot
        // Skip these checks for now as the data isn't available
        if claim.contains("52-week") {
            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: "Data not available in snapshot".to_string(),
                actual_value: None,
                is_consistent: true, // Don't penalize if data unavailable
                severity: "info".to_string(),
            });
        }
    }

    // Check fundamental claims
    let fundamental_claims = parse_fundamental_claims(&all_text);
    for claim in fundamental_claims {
        if claim.contains("Profitable") {
            let net_income = candidate.fundamental_snapshot.net_income_usd;

            let is_consistent = net_income.map_or(false, |v| v > 0.0);

            if !is_consistent && net_income.is_some() {
                major_violations += 1;
            }

            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: "Net income > 0".to_string(),
                actual_value: net_income,
                is_consistent,
                severity: if is_consistent {
                    "info".to_string()
                } else {
                    "major".to_string()
                },
            });
        }

        if claim.contains("Revenue growth") {
            let revenues = candidate.fundamental_snapshot.revenues_usd;

            // Without historical data, we can't verify growth
            // Just check if revenue exists
            let is_consistent = revenues.is_some();

            if !is_consistent {
                minor_violations += 1;
            }

            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: "Revenue data available".to_string(),
                actual_value: revenues,
                is_consistent,
                severity: if is_consistent {
                    "info".to_string()
                } else {
                    "minor".to_string()
                },
            });
        }

        if claim.contains("Undervalued") {
            let pe_like = candidate.fundamental_snapshot.pe_like;

            let is_consistent = pe_like.map_or(false, |v| v > 0.0 && v < 20.0);

            if !is_consistent && pe_like.is_some() {
                minor_violations += 1;
            }

            checks.push(ReasoningConsistencyCheck {
                claim: claim.clone(),
                expected_condition: "P/E-like ratio < 20".to_string(),
                actual_value: pe_like,
                is_consistent,
                severity: if is_consistent {
                    "info".to_string()
                } else {
                    "minor".to_string()
                },
            });
        }
    }

    // Score: start at 20, subtract violations
    // Major violations: -5 points each
    // Minor violations: -2 points each
    let score = (20 - (major_violations * 5) as i32 - (minor_violations * 2) as i32).max(0);

    ReasoningConsistencyReport {
        checks,
        consistency_score: score,
        major_violations,
        minor_violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::I18nText;
    use crate::pick::provenance::ProvenanceSnapshot;
    use crate::pick::types::{EnrichedCandidate, FactorBreakdown};
    use crate::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    fn create_test_pick_with_claim(claim: &str) -> StockPickItem {
        StockPickItem {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            thesis: I18nText::new(claim),
            evidence_points: vec![claim.to_string()],
            ..Default::default()
        }
    }

    fn create_test_candidate_with_rsi(rsi: f64) -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            industry: "Technology".to_string(),
            price: Some(100.0),
            change_pct: Some(2.0),
            market_cap: Some(1_000_000_000.0),
            theme_key: "test".to_string(),
            fundamentals: None,
            analyst_consensus: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![],
            technical_snapshot: StockPickTechnicalSnapshot {
                rsi: Some(rsi),
                ..Default::default()
            },
            market_snapshot: StockPickMarketSnapshot::default(),
            fundamental_snapshot: StockPickFundamentalSnapshot::default(),
            news_snapshot: StockPickNewsSnapshot::default(),
            history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
            risk_snapshot: StockPickRiskSnapshot::default(),
            data_quality_snapshot: StockPickDataQualitySnapshot::default(),
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: vec![],
            description: "Test candidate".to_string(),
            provenance: ProvenanceSnapshot::default(),
        }
    }

    #[test]
    fn test_parse_technical_claims() {
        let text = "RSI overbought at 75";
        let claims = parse_technical_claims(text);
        assert_eq!(claims.len(), 1);
        assert!(claims[0].contains("RSI overbought"));
    }

    #[test]
    fn test_validate_consistency_pass() {
        let pick = create_test_pick_with_claim("RSI overbought");
        let candidate = create_test_candidate_with_rsi(75.0);

        let report = validate_reasoning_consistency(&pick, &candidate);
        assert_eq!(report.major_violations, 0);
        assert!(report.consistency_score >= 15);
    }

    #[test]
    fn test_validate_consistency_fail() {
        let pick = create_test_pick_with_claim("RSI overbought");
        let candidate = create_test_candidate_with_rsi(45.0);

        let report = validate_reasoning_consistency(&pick, &candidate);
        assert!(report.major_violations > 0);
        assert!(report.consistency_score < 20);
    }

    #[test]
    fn test_reasoning_no_claims() {
        let pick = create_test_pick_with_claim("Generic investment thesis");
        let candidate = create_test_candidate_with_rsi(50.0);

        let report = validate_reasoning_consistency(&pick, &candidate);
        assert_eq!(report.major_violations, 0);
        assert!(report.consistency_score >= 15); // Neutral score
    }

    #[test]
    fn test_reasoning_multiple_violations() {
        let mut pick = create_test_pick_with_claim("RSI overbought and MACD bullish cross");
        pick.evidence_points = vec![
            "RSI overbought".to_string(),
            "MACD bullish cross".to_string(),
        ];

        let mut candidate = create_test_candidate_with_rsi(45.0); // RSI not overbought
        candidate.technical_snapshot.macd = Some(0.5);
        candidate.technical_snapshot.macd_signal = Some(1.0); // MACD not bullish

        let report = validate_reasoning_consistency(&pick, &candidate);
        assert!(report.major_violations >= 1); // At least RSI violation
        assert!(report.consistency_score < 15);
    }
}
