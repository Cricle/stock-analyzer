use stock_analyzer::guide::classify_impact;

#[test]
fn stock_crash_is_negative() {
    assert_eq!(
        classify_impact("Stock crash wipes billions", ""),
        "negative"
    );
}

#[test]
fn not_bullish_detects_negation() {
    assert_eq!(
        classify_impact("Analysts not bullish on tech sector", ""),
        "negative"
    );
}

#[test]
fn empty_text_is_neutral() {
    assert_eq!(classify_impact("", ""), "neutral");
}

#[test]
fn plain_positive() {
    assert_eq!(
        classify_impact("Markets rally on strong earnings", ""),
        "positive"
    );
}

#[test]
fn negation_of_negative_flips_to_positive() {
    assert_eq!(
        classify_impact("Analysts say not bearish outlook", ""),
        "positive"
    );
}
