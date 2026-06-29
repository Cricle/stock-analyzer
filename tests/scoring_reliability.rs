use sa::scoring::score_types::{DimensionScore, ScoreReliability};

#[test]
fn dimension_score_has_reliability() {
    let score = DimensionScore {
        score: 50,
        reason: "test".into(),
        reliability: ScoreReliability::Missing,
    };
    assert_eq!(score.reliability, ScoreReliability::Missing);
}

#[test]
fn score_reliability_display() {
    assert_eq!(ScoreReliability::High.to_string(), "high");
    assert_eq!(ScoreReliability::Low.to_string(), "low");
    assert_eq!(ScoreReliability::Missing.to_string(), "missing");
}

#[test]
fn default_reliability_is_high() {
    let score = DimensionScore {
        score: 75,
        reason: "strong signals".into(),
        reliability: ScoreReliability::default(),
    };
    assert_eq!(score.reliability, ScoreReliability::High);
}
