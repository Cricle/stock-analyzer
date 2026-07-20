//! Quality tier classification and enrichment attempt tracking.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::StockPickObjectiveAssessment;

/// Quality tier classification for stock picks based on objective assessment.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
pub enum StockPickQualityTier {
    /// Score >= 80, no major violations, ready flag is true
    ProductionReady,
    /// Score >= 60 or has issues preventing production readiness
    ReviewRequired,
    /// Score < 60 or insufficient data
    DataInsufficient,
}

impl Default for StockPickQualityTier {
    fn default() -> Self {
        Self::DataInsufficient
    }
}

/// Record of an enrichment attempt to improve data quality.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnrichmentAttempt {
    /// ISO 8601 timestamp when enrichment was attempted
    pub attempted_at: String,
    /// Fields that were targeted for enrichment
    pub target_fields: Vec<String>,
    /// Whether the enrichment succeeded
    pub success: bool,
    /// Error message if enrichment failed
    pub error: Option<String>,
}

/// Classify a stock pick into a quality tier based on objective assessment.
pub fn classify_quality_tier(assessment: &StockPickObjectiveAssessment) -> StockPickQualityTier {
    let score = assessment.final_score;
    let has_major_violations = assessment.gaps.iter()
        .any(|g| g.starts_with("reasoning_violation:"));

    if score >= 80 && !has_major_violations && assessment.ready {
        StockPickQualityTier::ProductionReady
    } else if score >= 60 {
        StockPickQualityTier::ReviewRequired
    } else {
        StockPickQualityTier::DataInsufficient
    }
}

/// Attempt to enrich a stock pick with missing data (stub for pipeline integration).
pub async fn attempt_enrichment(
    _pick: &mut crate::StockPickItem,
    _candidate: &mut crate::pick::EnrichedCandidate,
    _market_data: &crate::data::MarketDataClient,
    _llm_client: &crate::llm::LlmClient,
) -> EnrichmentAttempt {
    EnrichmentAttempt {
        attempted_at: chrono::Utc::now().to_rfc3339(),
        target_fields: vec![],
        success: false,
        error: Some("not implemented".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
