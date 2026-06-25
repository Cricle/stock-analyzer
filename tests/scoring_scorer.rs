use sa::scoring::scorer::weighted_total;
use sa::scoring::{DimensionScore, ScoreWeights};

#[test]
fn test_weighted_total_equal_weights() {
    let w = ScoreWeights {
        technical: 25,
        fundamental: 25,
        sentiment: 25,
        llm_analysis: 25,
    };
    let tech = DimensionScore {
        score: 80,
        reason: String::new(),
    };
    let fund = DimensionScore {
        score: 60,
        reason: String::new(),
    };
    let sent = DimensionScore {
        score: 40,
        reason: String::new(),
    };
    let llm = DimensionScore {
        score: 70,
        reason: String::new(),
    };
    let total = weighted_total(&w, &tech, &fund, &sent, &llm);
    assert_eq!(total, 62); // (80*25 + 60*25 + 40*25 + 70*25) / 100 = 62.5 -> 62
}

#[test]
fn test_weighted_total_unequal_weights() {
    let w = ScoreWeights {
        technical: 50,
        fundamental: 20,
        sentiment: 15,
        llm_analysis: 15,
    };
    let tech = DimensionScore {
        score: 90,
        reason: String::new(),
    };
    let fund = DimensionScore {
        score: 30,
        reason: String::new(),
    };
    let sent = DimensionScore {
        score: 30,
        reason: String::new(),
    };
    let llm = DimensionScore {
        score: 30,
        reason: String::new(),
    };
    let total = weighted_total(&w, &tech, &fund, &sent, &llm);
    assert_eq!(total, 60);
}

#[test]
fn test_weighted_total_all_max() {
    let w = ScoreWeights::default();
    let d = DimensionScore {
        score: 100,
        reason: String::new(),
    };
    let total = weighted_total(&w, &d, &d, &d, &d);
    assert_eq!(total, 100);
}

#[test]
fn test_weighted_total_all_min() {
    let w = ScoreWeights::default();
    let d = DimensionScore {
        score: 0,
        reason: String::new(),
    };
    let total = weighted_total(&w, &d, &d, &d, &d);
    assert_eq!(total, 0);
}
