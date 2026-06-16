use super::*;

#[test]
fn market_kind_debug() {
assert_eq!(format!("{:?}", MarketKind::AShare), "AShare");
assert_eq!(format!("{:?}", MarketKind::HongKong), "HongKong");
assert_eq!(format!("{:?}", MarketKind::UsEquity), "UsEquity");
}

#[test]
fn market_kind_clone_copy() {
    let m = MarketKind::AShare;
    let m2 = m;
    assert_eq!(m, m2);
}

#[test]
fn market_kind_eq() {
    assert_eq!(MarketKind::AShare, MarketKind::AShare);
    assert_ne!(MarketKind::AShare, MarketKind::UsEquity);
}

#[test]
fn news_item_serialization() {
    let n = NewsItem {
        published_at: "2025-01-15T10:00:00Z".into(),
        title: "Test News".into(),
        summary: "A test article".into(),
        source: "Reuters".into(),
        url: Some("https://example.com".into()),
    };
    let json = serde_json::to_string(&n).unwrap();
    let n2: NewsItem = serde_json::from_str(&json).unwrap();
    assert_eq!(n.title, n2.title);
    assert_eq!(n.url, n2.url);
}

#[test]
fn news_item_no_url() {
    let n = NewsItem {
        published_at: "2025-01-15T10:00:00Z".into(),
        title: "Test".into(),
        summary: "Summary".into(),
        source: "Source".into(),
        url: None,
    };
    let json = serde_json::to_string(&n).unwrap();
    let n2: NewsItem = serde_json::from_str(&json).unwrap();
    assert!(n2.url.is_none());
}

#[test]
fn news_fetch_attempt_default() {
    let a = NewsFetchAttempt::default();
    assert!(!a.success);
    assert_eq!(a.item_count, 0);
    assert!(a.error.is_none());
    assert!(a.query.is_none());
}

#[test]
fn news_fetch_attempt_with_error() {
    let a = NewsFetchAttempt {
        source: "test".into(),
        query: Some("AAPL".into()),
        success: false,
        item_count: 0,
        error: Some("timeout".into()),
    };
    let json = serde_json::to_string(&a).unwrap();
    let a2: NewsFetchAttempt = serde_json::from_str(&json).unwrap();
    assert!(!a2.success);
    assert_eq!(a2.error.as_deref(), Some("timeout"));
}

#[test]
fn news_fetch_result_default() {
    let r = NewsFetchResult::default();
    assert!(r.items.is_empty());
    assert!(r.attempts.is_empty());
    assert!(!r.cacheable);
}

#[test]
fn news_fetch_result_with_items() {
    let r = NewsFetchResult {
        items: vec![NewsItem {
            published_at: "2025-01-15".into(),
            title: "Title".into(),
            summary: "Summary".into(),
            source: "Source".into(),
            url: None,
        }],
        attempts: vec![],
        cacheable: true,
    };
    let json = serde_json::to_string(&r).unwrap();
    let r2: NewsFetchResult = serde_json::from_str(&json).unwrap();
    assert_eq!(r2.items.len(), 1);
    assert!(r2.cacheable);
}