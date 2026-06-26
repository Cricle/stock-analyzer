use sa::scoring::{ScoreWeights, score_label};

// --- ScoreWeights::validate ---

#[test]
fn score_weights_validate_valid() {
    let weights = ScoreWeights::default();
    assert!(weights.validate().is_ok());
}

#[test]
fn score_weights_validate_custom() {
    let weights = ScoreWeights {
        technical: 40,
        fundamental: 30,
        sentiment: 15,
        llm_analysis: 15,
    };
    assert!(weights.validate().is_ok());
}

#[test]
fn score_weights_validate_invalid() {
    let weights = ScoreWeights {
        technical: 50,
        fundamental: 50,
        sentiment: 50,
        llm_analysis: 50,
    };
    assert!(weights.validate().is_err());
}

#[test]
fn score_weights_validate_zero() {
    let weights = ScoreWeights {
        technical: 0,
        fundamental: 0,
        sentiment: 0,
        llm_analysis: 0,
    };
    assert!(weights.validate().is_err());
}

// --- score_label ---

#[test]
fn score_label_strong_buy() {
    assert_eq!(score_label(80), "strong_buy");
    assert_eq!(score_label(100), "strong_buy");
    assert_eq!(score_label(90), "strong_buy");
}

#[test]
fn score_label_buy() {
    assert_eq!(score_label(65), "buy");
    assert_eq!(score_label(79), "buy");
    assert_eq!(score_label(70), "buy");
}

#[test]
fn score_label_neutral() {
    assert_eq!(score_label(45), "neutral");
    assert_eq!(score_label(64), "neutral");
    assert_eq!(score_label(55), "neutral");
}

#[test]
fn score_label_cautious() {
    assert_eq!(score_label(30), "cautious");
    assert_eq!(score_label(44), "cautious");
    assert_eq!(score_label(35), "cautious");
}

#[test]
fn score_label_avoid() {
    assert_eq!(score_label(0), "avoid");
    assert_eq!(score_label(29), "avoid");
    assert_eq!(score_label(15), "avoid");
}
