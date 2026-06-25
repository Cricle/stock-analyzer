use sa::guide::store::{GuidanceStore, market_ttl};

#[test]
fn market_ttl_a_share_market_hours() {
    let base = std::time::Duration::from_secs(60);
    let result = market_ttl("a_share", base);
    assert!(result.as_secs() > 0);
}

#[test]
fn market_ttl_us_equity() {
    let base = std::time::Duration::from_secs(60);
    let result = market_ttl("us_equity", base);
    assert!(result.as_secs() > 0);
}

#[test]
fn market_ttl_unknown_market() {
    let base = std::time::Duration::from_secs(60);
    let result = market_ttl("unknown", base);
    assert_eq!(result, base * 12);
}

#[test]
fn market_ttl_hong_kong() {
    let base = std::time::Duration::from_secs(120);
    let result = market_ttl("hong_kong", base);
    assert!(result.as_secs() > 0);
}

#[test]
fn qdrant_point_id_deterministic() {
    let a = GuidanceStore::qdrant_point_id("test-id");
    let b = GuidanceStore::qdrant_point_id("test-id");
    assert_eq!(a, b);
}

#[test]
fn qdrant_point_id_uuid_format() {
    let id = GuidanceStore::qdrant_point_id("entry-123");
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 4);
    assert_eq!(parts[2].len(), 4);
    assert_eq!(parts[3].len(), 4);
    assert_eq!(parts[4].len(), 12);
}

#[test]
fn qdrant_point_id_different_inputs() {
    let a = GuidanceStore::qdrant_point_id("id-a");
    let b = GuidanceStore::qdrant_point_id("id-b");
    assert_ne!(a, b);
}

#[test]
fn news_dedup_key_deterministic() {
    let a = GuidanceStore::news_dedup_key("Apple Earnings", "Reuters");
    let b = GuidanceStore::news_dedup_key("Apple Earnings", "Reuters");
    assert_eq!(a, b);
}

#[test]
fn news_dedup_key_case_insensitive() {
    let a = GuidanceStore::news_dedup_key("Apple Earnings", "Reuters");
    let b = GuidanceStore::news_dedup_key("apple earnings", "reuters");
    assert_eq!(a, b);
}

#[test]
fn news_dedup_key_different_content() {
    let a = GuidanceStore::news_dedup_key("Apple Earnings", "Reuters");
    let b = GuidanceStore::news_dedup_key("Google Earnings", "Bloomberg");
    assert_ne!(a, b);
}

#[test]
fn news_dedup_key_hex_length() {
    let key = GuidanceStore::news_dedup_key("test", "source");
    assert_eq!(key.len(), 32);
}
