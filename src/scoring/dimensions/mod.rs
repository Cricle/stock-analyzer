pub mod fundamental;
pub mod llm_analysis;
pub mod sentiment;
pub mod technical;

use super::score_types::{DimensionScore, ScoreReliability};

/// Compute a weighted score from total and weight_sum, with a default of 50 when no data.
pub fn weighted_score(
    total: f64,
    weight_sum: f64,
    default_reason: &str,
    reasons: &[String],
) -> DimensionScore {
    if weight_sum <= 0.0 {
        return DimensionScore {
            score: 50,
            reason: default_reason.into(),
            reliability: ScoreReliability::Missing,
        };
    }
    let score = (total / weight_sum * 100.0).clamp(0.0, 100.0) as u8;
    let reliability = if reasons.is_empty() {
        ScoreReliability::Missing
    } else {
        ScoreReliability::High
    };
    DimensionScore {
        score,
        reason: if reasons.is_empty() {
            default_reason.into()
        } else {
            reasons.join("；")
        },
        reliability,
    }
}
