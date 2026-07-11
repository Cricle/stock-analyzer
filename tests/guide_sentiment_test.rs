use stock_analyzer::guide::sentiment_score;

#[test]
fn empty_news_returns_zero() {
    assert_eq!(sentiment_score(0, 0, 0), 0);
}

#[test]
fn single_positive_news_neutral_due_to_threshold() {
    assert_eq!(sentiment_score(1, 0, 1), 0);
}

#[test]
fn two_items_still_below_threshold() {
    assert_eq!(sentiment_score(2, 0, 2), 0);
}

#[test]
fn mixed_news_balanced_near_zero() {
    assert_eq!(sentiment_score(3, 3, 10), 0);
}

#[test]
fn all_negative_large_set_near_minus_100() {
    assert_eq!(sentiment_score(0, 10, 10), -100);
}

#[test]
fn all_positive_large_set_near_plus_100() {
    assert_eq!(sentiment_score(10, 0, 10), 100);
}

#[test]
fn score_clamped() {
    let s = sentiment_score(100, 0, 100);
    assert!(s >= -100 && s <= 100);
}
