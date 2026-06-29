use sa::analysis::{LocalText, Rating};

// --- LocalText ---

#[test]
fn local_text_new() {
    let lt = LocalText::new("test_key");
    assert_eq!(lt.as_str(), "test_key");
    assert!(lt.params.is_empty());
}

#[test]
fn local_text_with_param() {
    let lt = LocalText::new("key").with_param("name", serde_json::json!("value"));
    assert_eq!(lt.params.get("name"), Some(&serde_json::json!("value")));
}

#[test]
fn local_text_with_str() {
    let lt = LocalText::new("key").with_str("name", "value");
    assert_eq!(lt.params.get("name"), Some(&serde_json::json!("value")));
}

#[test]
fn local_text_with_f64() {
    let lt = LocalText::new("key").with_f64("price", 42.5);
    assert_eq!(lt.params.get("price"), Some(&serde_json::json!(42.5)));
}

#[test]
fn local_text_with_i32() {
    let lt = LocalText::new("key").with_i32("count", 10);
    assert_eq!(lt.params.get("count"), Some(&serde_json::json!(10)));
}

#[test]
fn local_text_with_bool() {
    let lt = LocalText::new("key").with_bool("flag", true);
    assert_eq!(lt.params.get("flag"), Some(&serde_json::json!(true)));
}

#[test]
fn local_text_is_empty() {
    assert!(LocalText::new("").is_empty());
    assert!(!LocalText::new("key").is_empty());
}

#[test]
fn local_text_trim() {
    let lt = LocalText::new("  hello  ");
    assert_eq!(lt.trim(), "hello");
}

#[test]
fn local_text_split() {
    let lt = LocalText::new("a,b,c");
    let parts: Vec<&str> = lt.split(",").collect();
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn local_text_contains() {
    let lt = LocalText::new("hello world");
    assert!(lt.contains("world"));
    assert!(!lt.contains("xyz"));
}

#[test]
fn local_text_starts_with() {
    let lt = LocalText::new("hello world");
    assert!(lt.starts_with("hello"));
    assert!(!lt.starts_with("world"));
}

#[test]
fn local_text_to_ascii_lowercase() {
    let lt = LocalText::new("Hello World");
    assert_eq!(lt.to_ascii_lowercase(), "hello world");
}

#[test]
fn local_text_display() {
    let lt = LocalText::new("test_key");
    assert_eq!(format!("{lt}"), "test_key");
}

#[test]
fn local_text_eq() {
    let a = LocalText::new("key").with_param("x", serde_json::json!(1));
    let b = LocalText::new("key").with_param("y", serde_json::json!(2));
    assert_eq!(a, b); // Only compares key
}

#[test]
fn local_text_from_str() {
    let lt: LocalText = "hello".into();
    assert_eq!(lt.as_str(), "hello");
}

#[test]
fn local_text_from_string() {
    let lt: LocalText = String::from("hello").into();
    assert_eq!(lt.as_str(), "hello");
}

#[test]
fn local_text_serde_roundtrip() {
    let lt = LocalText::new("key").with_param("x", serde_json::json!(42));
    let json = serde_json::to_string(&lt).unwrap();
    let restored: LocalText = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.as_str(), "key");
    assert_eq!(restored.params.get("x"), Some(&serde_json::json!(42)));
}

#[test]
fn local_text_deserialize_legacy_string() {
    let lt: LocalText = serde_json::from_str("\"hello\"").unwrap();
    assert_eq!(lt.as_str(), "hello");
    assert!(lt.params.is_empty());
}

// --- Rating ---

#[test]
fn rating_parse() {
    assert_eq!(Rating::parse("buy"), Rating::Buy);
    assert_eq!(Rating::parse("overweight"), Rating::Overweight);
    assert_eq!(Rating::parse("hold"), Rating::Hold);
    assert_eq!(Rating::parse("underweight"), Rating::Underweight);
    assert_eq!(Rating::parse("sell"), Rating::Sell);
    assert_eq!(Rating::parse("unknown"), Rating::Unknown);
}

#[test]
fn rating_is_bullish() {
    assert!(Rating::Buy.is_bullish());
    assert!(Rating::Overweight.is_bullish());
    assert!(!Rating::Hold.is_bullish());
    assert!(!Rating::Unknown.is_bullish());
    assert!(!Rating::Sell.is_bullish());
}

#[test]
fn rating_is_bearish() {
    assert!(Rating::Sell.is_bearish());
    assert!(Rating::Underweight.is_bearish());
    assert!(!Rating::Hold.is_bearish());
    assert!(!Rating::Unknown.is_bearish());
    assert!(!Rating::Buy.is_bearish());
}

#[test]
fn rating_is_neutral() {
    assert!(Rating::Hold.is_neutral());
    assert!(Rating::Unknown.is_neutral());
    assert!(!Rating::Buy.is_neutral());
    assert!(!Rating::Sell.is_neutral());
}

#[test]
fn rating_bias() {
    assert_eq!(Rating::Buy.bias(100), 100);
    assert_eq!(Rating::Overweight.bias(100), 75);
    assert_eq!(Rating::Hold.bias(100), 0);
    assert_eq!(Rating::Underweight.bias(100), -75);
    assert_eq!(Rating::Sell.bias(100), -100);
}

#[test]
fn rating_to_score() {
    assert_eq!(Rating::Buy.to_score(), 2);
    assert_eq!(Rating::Overweight.to_score(), 1);
    assert_eq!(Rating::Hold.to_score(), 0);
    assert_eq!(Rating::Underweight.to_score(), -1);
    assert_eq!(Rating::Sell.to_score(), -2);
}

#[test]
fn rating_to_action_group() {
    assert_eq!(Rating::Buy.to_action_group(), "Buy");
    assert_eq!(Rating::Overweight.to_action_group(), "Buy");
    assert_eq!(Rating::Hold.to_action_group(), "Hold");
    assert_eq!(Rating::Sell.to_action_group(), "Sell");
    assert_eq!(Rating::Underweight.to_action_group(), "Sell");
}

#[test]
fn rating_display() {
    assert_eq!(format!("{}", Rating::Buy), "Buy");
    assert_eq!(format!("{}", Rating::Overweight), "Overweight");
    assert_eq!(format!("{}", Rating::Hold), "Hold");
    assert_eq!(format!("{}", Rating::Underweight), "Underweight");
    assert_eq!(format!("{}", Rating::Sell), "Sell");
}

#[test]
fn rating_serde_roundtrip() {
    let ratings = [
        Rating::Buy,
        Rating::Overweight,
        Rating::Hold,
        Rating::Underweight,
        Rating::Sell,
    ];
    for rating in &ratings {
        let json = serde_json::to_string(rating).unwrap();
        let restored: Rating = serde_json::from_str(&json).unwrap();
        assert_eq!(*rating, restored);
    }
}

#[test]
fn rating_default() {
    assert_eq!(Rating::default(), Rating::Hold);
}
