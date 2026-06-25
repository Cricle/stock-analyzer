use sa::memory::{
    MemoryEntry, SetupMatchStats, TradingMemoryLog, bucket_score, bucket_signed_score,
    extract_labeled_block, realized_call_hit, summarize_entries,
};

// --- realized_call_hit ---

#[test]
fn realized_call_hit_buy_positive() {
    assert!(realized_call_hit("Buy", 0.05, 0.03));
}

#[test]
fn realized_call_hit_buy_negative_raw() {
    assert!(!realized_call_hit("Buy", -0.01, 0.03));
}

#[test]
fn realized_call_hit_buy_negative_alpha() {
    assert!(!realized_call_hit("Buy", 0.05, -0.01));
}

#[test]
fn realized_call_hit_overweight_positive_alpha() {
    assert!(realized_call_hit("Overweight", -0.01, 0.02));
}

#[test]
fn realized_call_hit_overweight_negative_alpha() {
    assert!(!realized_call_hit("Overweight", 0.05, -0.01));
}

#[test]
fn realized_call_hit_hold_small_move() {
    assert!(realized_call_hit("Hold", 0.01, 0.01));
}

#[test]
fn realized_call_hit_hold_large_move() {
    assert!(!realized_call_hit("Hold", 0.10, 0.05));
}

#[test]
fn realized_call_hit_hold_alpha_boundary() {
    assert!(realized_call_hit("Hold", 0.10, 0.03));
    assert!(!realized_call_hit("Hold", 0.10, 0.04));
}

#[test]
fn realized_call_hit_underweight() {
    assert!(realized_call_hit("Underweight", 0.05, -0.02));
    assert!(!realized_call_hit("Underweight", 0.05, 0.02));
}

#[test]
fn realized_call_hit_sell_both_negative() {
    assert!(realized_call_hit("Sell", -0.05, -0.03));
}

#[test]
fn realized_call_hit_sell_positive_raw() {
    assert!(!realized_call_hit("Sell", 0.01, -0.03));
}

#[test]
fn realized_call_hit_unknown_rating() {
    assert!(!realized_call_hit("Unknown", 0.05, 0.03));
}

// --- bucket_score ---

#[test]
fn bucket_score_none() {
    assert_eq!(bucket_score(None, &[20, 40, 60, 80]), "unknown");
}

#[test]
fn bucket_score_below_first() {
    assert_eq!(bucket_score(Some(10), &[20, 40, 60, 80]), "<20");
}

#[test]
fn bucket_score_first_range() {
    assert_eq!(bucket_score(Some(30), &[20, 40, 60, 80]), "20-39");
}

#[test]
fn bucket_score_second_range() {
    assert_eq!(bucket_score(Some(50), &[20, 40, 60, 80]), "40-59");
}

#[test]
fn bucket_score_third_range() {
    assert_eq!(bucket_score(Some(70), &[20, 40, 60, 80]), "60-79");
}

#[test]
fn bucket_score_above_last() {
    assert_eq!(bucket_score(Some(90), &[20, 40, 60, 80]), ">=80");
}

#[test]
fn bucket_score_exact_boundary() {
    assert_eq!(bucket_score(Some(40), &[20, 40, 60, 80]), "40-59");
}

// --- bucket_signed_score ---

#[test]
fn bucket_signed_score_none() {
    assert_eq!(bucket_signed_score(None, &[-40, -20, 20, 40]), "unknown");
}

#[test]
fn bucket_signed_score_below_first() {
    assert_eq!(bucket_signed_score(Some(-50), &[-40, -20, 20, 40]), "<-40");
}

#[test]
fn bucket_signed_score_first_range() {
    assert_eq!(
        bucket_signed_score(Some(-30), &[-40, -20, 20, 40]),
        "-40..-21"
    );
}

#[test]
fn bucket_signed_score_middle_range() {
    assert_eq!(bucket_signed_score(Some(0), &[-40, -20, 20, 40]), "-20..19");
}

#[test]
fn bucket_signed_score_above_last() {
    assert_eq!(bucket_signed_score(Some(50), &[-40, -20, 20, 40]), ">=40");
}

// --- extract_labeled_block ---

#[test]
fn extract_labeled_block_meta() {
    let text = "META:\n{\"rating\":\"Buy\"}\n\nDECISION:\nhold\n";
    assert_eq!(
        extract_labeled_block(text, "META"),
        Some("{\"rating\":\"Buy\"}\n")
    );
}

#[test]
fn extract_labeled_block_decision() {
    let text = "META:\nmeta content\n\nDECISION:\ndecision content\n\nREFLECTION:\nreflect\n";
    assert_eq!(
        extract_labeled_block(text, "DECISION"),
        Some("decision content\n")
    );
}

#[test]
fn extract_labeled_block_missing() {
    let text = "DECISION:\nhold\n";
    assert_eq!(extract_labeled_block(text, "META"), None);
}

#[test]
fn extract_labeled_block_reflection() {
    let text = "DECISION:\nhold\n\nREFLECTION:\nI learned something\n";
    assert_eq!(
        extract_labeled_block(text, "REFLECTION"),
        Some("I learned something\n")
    );
}

// --- SetupMatchStats default ---

#[test]
fn setup_match_stats_default() {
    let stats = SetupMatchStats::default();
    assert_eq!(stats.total_match_count, 0);
    assert_eq!(stats.hit_rate, 0.0);
    assert!(!stats.used_fallback);
}

// --- build_stats_from_resolved_entries ---

#[test]
fn build_stats_empty() {
    let stats = TradingMemoryLog::build_stats_from_resolved_entries(&[]);
    assert_eq!(stats.total_match_count, 0);
}

#[test]
fn build_stats_with_entries() {
    let entries = vec![
        MemoryEntry {
            ticker: "AAPL".into(),
            trade_date: "2025-01-01".into(),
            rating: "Buy".into(),
            action: "Buy".into(),
            alpha_return: Some(0.05),
            raw_return: Some(0.08),
            ..Default::default()
        },
        MemoryEntry {
            ticker: "AAPL".into(),
            trade_date: "2025-02-01".into(),
            rating: "Sell".into(),
            action: "Sell".into(),
            alpha_return: Some(-0.03),
            raw_return: Some(-0.05),
            ..Default::default()
        },
    ];
    let stats = TradingMemoryLog::build_stats_from_resolved_entries(&entries);
    assert_eq!(stats.total_match_count, 2);
    assert_eq!(stats.resolved_match_count, 2);
    assert_eq!(stats.long_match_count, 1);
    assert_eq!(stats.short_match_count, 1);
    assert_eq!(stats.neutral_match_count, 0);
    assert!((stats.avg_alpha_return - 0.01).abs() < 0.001);
    assert!((stats.hit_rate - 0.5).abs() < 0.001);
}

// --- summarize_entries ---

#[test]
fn summarize_entries_empty() {
    let result = summarize_entries(&[]);
    assert_eq!(result["count"], 0);
}

#[test]
fn summarize_entries_with_data() {
    let entries = vec![
        MemoryEntry {
            rating: "Buy".into(),
            raw_return: Some(0.05),
            alpha_return: Some(0.03),
            ..Default::default()
        },
        MemoryEntry {
            rating: "Sell".into(),
            raw_return: Some(-0.02),
            alpha_return: Some(-0.01),
            ..Default::default()
        },
    ];
    let result = summarize_entries(&entries);
    assert_eq!(result["count"], 2);
    assert!(result["avg_raw_return"].as_f64().unwrap() > 0.0);
}
