use stock_analyzer::scoring::dimensions::sentiment::parse_sentiment_response;
use stock_analyzer::scoring::dimensions::weighted_score;
use stock_analyzer::scoring::score_types::{DimensionScore, ScoreReliability};

#[test]
fn weighted_score_missing_when_no_data() {
    let result = weighted_score(0.0, 0.0, "no data", &[]);
    assert_eq!(result.score, 50);
    assert_eq!(result.reliability, ScoreReliability::Missing);
}

#[test]
fn weighted_score_high_when_data_present() {
    let result = weighted_score(75.0, 100.0, "ok", &["reason".into()]);
    assert_eq!(result.score, 75);
    assert_eq!(result.reliability, ScoreReliability::High);
}

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

#[test]
fn sentiment_parse_failure_is_missing_reliability() {
    let result = parse_sentiment_response("not json at all");
    assert_eq!(result.score, 50);
    assert_eq!(
        result.reliability,
        stock_analyzer::scoring::score_types::ScoreReliability::Missing
    );
}

#[test]
fn sentiment_empty_headlines_is_missing() {
    let result = parse_sentiment_response("{}");
    assert_eq!(
        result.reliability,
        stock_analyzer::scoring::score_types::ScoreReliability::Missing
    );
}

use stock_analyzer::scoring::dimensions::llm_analysis::{LlmAnalysisInput, score_llm_analysis};

#[test]
fn llm_analysis_missing_history_is_low_reliability() {
    let input = LlmAnalysisInput {
        confidence: 60.0,
        objective_final_score: 60.0,
        momentum_score: 50.0,
        hit_rate: None, // missing history
        catalyst_count: 0,
        hard_negative_count: 0,
        volume_ratio: None, // missing market
        period_return_pct: None,
    };
    let result = score_llm_analysis(&input);
    assert_eq!(
        result.reliability,
        stock_analyzer::scoring::score_types::ScoreReliability::Low,
        "missing history and market data should yield Low reliability"
    );
}
