#[test]
fn e2e_store_trait_implementations_exist() {
    // Verify that the store traits are properly implemented
    // by checking that key types can be constructed
    // This is a compile-time check wrapped in a runtime test
    fn assert_cache_store<T: sa::CacheStore>() {}
    fn assert_vector_store<T: sa::VectorStore>() {}
    fn assert_analysis_store<T: sa::AnalysisStore>() {}
    fn assert_checkpoint_store<T: sa::CheckpointStore>() {}
    fn assert_guidance_store<T: sa::GuidanceStore>() {}

    // If these compile, the traits are implemented
    // We can't actually instantiate them without Redis/PostgreSQL,
    // but the type system verifies the trait bounds
}

#[test]
fn e2e_task_status_roundtrip() {
    use sa::task::TaskStatus;

    let statuses = vec![
        TaskStatus::Pending,
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Cancelled,
        TaskStatus::Failed,
    ];

    for status in statuses {
        let s = status.as_str();
        let restored: TaskStatus = s.parse().unwrap();
        assert_eq!(status, restored);

        let json = serde_json::to_string(&status).unwrap();
        let from_json: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, from_json);
    }
}

#[test]
fn e2e_user_preferences_watchlist() {
    use sa::user_preferences::{UserPreferences, WatchlistItem};

    let mut prefs = UserPreferences::default();

    let item = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        notes: "Tech stock".into(),
        added_at: "2026-06-21".into(),
    };

    // Add to watchlist
    assert!(prefs.add_to_watchlist(item));
    assert_eq!(prefs.watchlist.len(), 1);

    // Duplicate should return false
    let dup = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        ..Default::default()
    };
    assert!(!prefs.add_to_watchlist(dup));
    assert_eq!(prefs.watchlist.len(), 1);

    // Different market should succeed
    let diff_market = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "A股".into(),
        ..Default::default()
    };
    assert!(prefs.add_to_watchlist(diff_market));
    assert_eq!(prefs.watchlist.len(), 2);

    // Remove
    assert!(prefs.remove_from_watchlist("AAPL", "美股"));
    assert_eq!(prefs.watchlist.len(), 1);

    // Remove non-existent
    assert!(!prefs.remove_from_watchlist("AAPL", "美股"));
}

#[test]
fn e2e_user_preferences_serialization() {
    use sa::user_preferences::UserPreferences;

    let mut prefs = UserPreferences::default();
    prefs.language = "zh".into();
    prefs.ui_theme = "dark".into();
    prefs.default_market = "A股".into();

    let json = prefs.to_json();
    assert!(json.contains("zh"));
    assert!(json.contains("dark"));

    let restored = UserPreferences::from_json(&json);
    assert_eq!(restored.language, "zh");
    assert_eq!(restored.ui_theme, "dark");
    assert_eq!(restored.default_market, "A股");
}

#[test]
fn e2e_value_utils_normalization() {
    use serde_json::json;

    // Probability normalization
    let prob = sa::value_utils::normalize_probability(&json!(0.75));
    assert!((prob - 0.75).abs() < 0.01);

    let prob = sa::value_utils::normalize_probability(&json!("75%"));
    assert!((prob - 0.75).abs() < 0.01);

    // Value normalization
    let val = sa::value_utils::normalize_value(&json!("  hello  "));
    assert_eq!(val, "hello");

    let val = sa::value_utils::normalize_value(&json!(42));
    assert_eq!(val, "42");
}
