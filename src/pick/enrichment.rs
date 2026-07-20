//! Quality tier classification and enrichment attempt tracking.
//!
//! This module provides:
//! - Quality tier classification (actively used)
//! - One-time retry for recoverable data gaps

use crate::pick::{DataProvenance, EnrichedCandidate};
use crate::{StockPickObjectiveAssessment, data::MarketDataClient};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Quality tier classification for stock picks based on objective assessment.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub enum StockPickQualityTier {
    /// Score >= 80, no major violations, ready flag is true
    ProductionReady,
    /// Score >= 60 or has issues preventing production readiness
    ReviewRequired,
    /// Score < 60 or insufficient data
    #[default]
    DataInsufficient,
}

/// Record of an enrichment attempt to improve data quality.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnrichmentAttempt {
    /// ISO 8601 timestamp when enrichment was attempted
    pub attempted_at: String,
    /// Fields that were targeted for enrichment
    pub target_fields: Vec<String>,
    /// Fields successfully refreshed during this attempt.
    #[serde(default)]
    pub obtained_fields: Vec<String>,
    /// Fields that remain unavailable after the attempt.
    #[serde(default)]
    pub still_missing_fields: Vec<String>,
    /// Whether the enrichment succeeded
    pub success: bool,
    /// Error message if enrichment failed
    pub error: Option<String>,
}

/// Return data sources that can be re-fetched without re-running LLM reasoning.
pub(crate) fn enrichment_targets(candidate: &EnrichedCandidate) -> Vec<String> {
    let mut targets = Vec::new();
    if candidate.provenance.market_data.is_none() {
        targets.push("market_data".to_string());
    }
    if candidate.provenance.fundamentals.is_none() {
        targets.push("fundamentals".to_string());
    }
    if candidate.provenance.technicals.is_none() {
        targets.push("technicals".to_string());
    }
    if candidate.provenance.news.is_none() {
        targets.push("news".to_string());
    }
    targets
}

/// Re-fetch only the data sources missing from a selected candidate.
///
/// This function is intentionally single-shot: callers store its result on the
/// pick and do not invoke it again once an attempt exists.
pub async fn attempt_enrichment(
    candidate: &mut EnrichedCandidate,
    market_data: &MarketDataClient,
    analysis_date: &str,
) -> EnrichmentAttempt {
    let target_fields = enrichment_targets(candidate);
    let attempted_at = chrono::Utc::now().to_rfc3339();
    if target_fields.is_empty() {
        return EnrichmentAttempt {
            attempted_at,
            target_fields,
            obtained_fields: Vec::new(),
            still_missing_fields: Vec::new(),
            success: false,
            error: Some("no recoverable data gaps".to_string()),
        };
    }

    let mut obtained_fields = Vec::new();
    let mut still_missing_fields = Vec::new();
    let mut errors = Vec::new();

    for field in &target_fields {
        let result = match field.as_str() {
            "market_data" => match market_data.fetch_quote(&candidate.symbol).await {
                Ok(quote) if quote.close > 0.0 => {
                    candidate.price = Some(quote.close);
                    Ok(())
                }
                Ok(_) => Err("quote has no positive close".to_string()),
                Err(error) => Err(error.to_string()),
            },
            "fundamentals" => match market_data.fetch_fundamentals(&candidate.symbol).await {
                Ok(fundamentals) => {
                    candidate.market_cap = fundamentals.market_cap;
                    candidate.fundamentals = Some(fundamentals);
                    Ok(())
                }
                Err(error) => Err(error.to_string()),
            },
            "technicals" => match market_data
                .fetch_candles(&candidate.symbol, "qfq", 260)
                .await
            {
                Ok(candles) if candles.len() >= 5 => {
                    candidate.candles = candles;
                    Ok(())
                }
                Ok(candles) => Err(format!("only {} candles returned", candles.len())),
                Err(error) => Err(error.to_string()),
            },
            "news" => match market_data
                .fetch_news(&candidate.symbol, 5, None, None)
                .await
            {
                Ok(news) if !news.is_empty() => {
                    candidate.news = news;
                    Ok(())
                }
                Ok(_) => Err("no news returned".to_string()),
                Err(error) => Err(error.to_string()),
            },
            _ => Err("unsupported enrichment target".to_string()),
        };

        match result {
            Ok(()) => obtained_fields.push(field.clone()),
            Err(error) => {
                still_missing_fields.push(field.clone());
                errors.push(format!("{field}: {error}"));
            }
        }
    }

    crate::pick::scoring::refresh_candidate_state(candidate);
    refresh_provenance(candidate, analysis_date, &obtained_fields);

    let success = still_missing_fields.is_empty();
    EnrichmentAttempt {
        attempted_at,
        target_fields,
        obtained_fields,
        still_missing_fields,
        success,
        error: (!success).then(|| errors.join("; ")),
    }
}

fn refresh_provenance(candidate: &mut EnrichedCandidate, analysis_date: &str, fields: &[String]) {
    for field in fields {
        let provenance = DataProvenance {
            source: "enrichment_retry".to_string(),
            fetched_at: analysis_date.to_string(),
            confidence: 0.8,
            field_coverage: vec![field.clone()],
        };
        match field.as_str() {
            "market_data" => candidate.provenance.market_data = Some(provenance),
            "fundamentals" => candidate.provenance.fundamentals = Some(provenance),
            "technicals" => candidate.provenance.technicals = Some(provenance),
            "news" => candidate.provenance.news = Some(provenance),
            _ => {}
        }
    }
}

/// Classify a stock pick into a quality tier based on objective assessment.
pub fn classify_quality_tier(assessment: &StockPickObjectiveAssessment) -> StockPickQualityTier {
    let score = assessment.final_score;
    let has_major_violations = assessment
        .gaps
        .iter()
        .any(|g| g.starts_with("reasoning_violation:"));

    if score >= 80 && !has_major_violations && assessment.ready {
        StockPickQualityTier::ProductionReady
    } else if score >= 60 {
        StockPickQualityTier::ReviewRequired
    } else {
        StockPickQualityTier::DataInsufficient
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pick::provenance::ProvenanceSnapshot;
    use crate::pick::types::{EnrichedCandidate, FactorBreakdown};

    fn create_assessment_with_score(score: i32, ready: bool) -> StockPickObjectiveAssessment {
        StockPickObjectiveAssessment {
            final_score: score,
            ready,
            gaps: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn test_classify_tier_production_ready() {
        let assessment = create_assessment_with_score(85, true);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::ProductionReady));
    }

    #[test]
    fn test_classify_tier_review_required() {
        let assessment = create_assessment_with_score(70, false);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::ReviewRequired));
    }

    #[test]
    fn test_classify_tier_data_insufficient() {
        let assessment = create_assessment_with_score(50, false);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::DataInsufficient));
    }

    #[test]
    fn test_classify_tier_with_violations() {
        let mut assessment = create_assessment_with_score(85, true);
        assessment.gaps = vec!["reasoning_violation:inconsistent".to_string()];
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::ReviewRequired));
    }

    #[test]
    fn test_classify_tier_boundary_80() {
        let assessment = create_assessment_with_score(80, true);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::ProductionReady));
    }

    #[test]
    fn test_classify_tier_boundary_60() {
        let assessment = create_assessment_with_score(60, false);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::ReviewRequired));
    }

    #[test]
    fn test_classify_tier_boundary_59() {
        let assessment = create_assessment_with_score(59, false);
        let tier = classify_quality_tier(&assessment);
        assert!(matches!(tier, StockPickQualityTier::DataInsufficient));
    }

    #[test]
    fn targets_only_missing_recoverable_data_sources() {
        let candidate = EnrichedCandidate {
            provenance: ProvenanceSnapshot {
                market_data: None,
                fundamentals: None,
                technicals: None,
                news: Some(Default::default()),
            },
            ..test_candidate()
        };

        assert_eq!(
            enrichment_targets(&candidate),
            vec!["market_data", "fundamentals", "technicals"]
        );
    }

    #[test]
    fn does_not_retry_when_candidate_has_no_recoverable_gaps() {
        let candidate = EnrichedCandidate {
            provenance: ProvenanceSnapshot {
                market_data: Some(Default::default()),
                fundamentals: Some(Default::default()),
                technicals: Some(Default::default()),
                news: Some(Default::default()),
            },
            ..test_candidate()
        };

        assert!(enrichment_targets(&candidate).is_empty());
    }

    fn test_candidate() -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            industry: "Technology".to_string(),
            price: Some(100.0),
            change_pct: None,
            market_cap: Some(1_000_000_000.0),
            theme_key: "tech".to_string(),
            fundamentals: None,
            analyst_consensus: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![],
            technical_snapshot: Default::default(),
            market_snapshot: Default::default(),
            fundamental_snapshot: Default::default(),
            news_snapshot: Default::default(),
            history_match_snapshot: Default::default(),
            risk_snapshot: Default::default(),
            data_quality_snapshot: Default::default(),
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: vec![],
            description: String::new(),
            provenance: ProvenanceSnapshot::default(),
        }
    }
}
