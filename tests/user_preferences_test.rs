use sa::user_preferences::{UserPreferences, WatchlistItem};

// --- from_json / to_json ---

#[test]
fn from_json_valid() {
    let json = r#"{"language":"zh","ui_theme":"dark"}"#;
    let prefs = UserPreferences::from_json(json);
    assert_eq!(prefs.language, "zh");
    assert_eq!(prefs.ui_theme, "dark");
}

#[test]
fn from_json_invalid_returns_default() {
    let prefs = UserPreferences::from_json("not json");
    assert_eq!(prefs.language, "");
    assert_eq!(prefs.default_market, "");
}

#[test]
fn from_json_defaults() {
    let prefs = UserPreferences::from_json("{}");
    assert!(prefs.notifications_enabled);
    assert_eq!(prefs.refresh_interval, 60);
    assert_eq!(prefs.sidebar_width, 240);
    assert_eq!(prefs.default_market, "A股");
    assert_eq!(prefs.default_depth, "3");
    assert_eq!(
        prefs.default_analysts,
        vec!["market", "fundamentals", "news"]
    );
}

#[test]
fn to_json_roundtrip() {
    let mut prefs = UserPreferences::default();
    prefs.language = "en".into();
    let json = prefs.to_json();
    let restored = UserPreferences::from_json(&json);
    assert_eq!(restored.language, "en");
}

#[test]
fn to_json_not_empty() {
    let prefs = UserPreferences::default();
    let json = prefs.to_json();
    assert!(json.contains("default_market"));
}

// --- add_to_watchlist ---

#[test]
fn add_to_watchlist_new() {
    let mut prefs = UserPreferences::default();
    let item = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        notes: "".into(),
        added_at: "".into(),
    };
    assert!(prefs.add_to_watchlist(item));
    assert_eq!(prefs.watchlist.len(), 1);
}

#[test]
fn add_to_watchlist_duplicate() {
    let mut prefs = UserPreferences::default();
    let item = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        notes: "".into(),
        added_at: "".into(),
    };
    prefs.add_to_watchlist(item.clone());
    assert!(!prefs.add_to_watchlist(item));
    assert_eq!(prefs.watchlist.len(), 1);
}

#[test]
fn add_to_watchlist_different_market() {
    let mut prefs = UserPreferences::default();
    let item1 = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        ..Default::default()
    };
    let item2 = WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "A股".into(),
        ..Default::default()
    };
    assert!(prefs.add_to_watchlist(item1));
    assert!(prefs.add_to_watchlist(item2));
    assert_eq!(prefs.watchlist.len(), 2);
}

// --- remove_from_watchlist ---

#[test]
fn remove_from_watchlist_existing() {
    let mut prefs = UserPreferences::default();
    prefs.watchlist.push(WatchlistItem {
        symbol: "AAPL".into(),
        name: "Apple".into(),
        market: "美股".into(),
        ..Default::default()
    });
    assert!(prefs.remove_from_watchlist("AAPL", "美股"));
    assert!(prefs.watchlist.is_empty());
}

#[test]
fn remove_from_watchlist_nonexistent() {
    let mut prefs = UserPreferences::default();
    assert!(!prefs.remove_from_watchlist("AAPL", "美股"));
}
