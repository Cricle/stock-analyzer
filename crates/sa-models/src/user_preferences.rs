//! Enhanced user preferences with watchlist, risk profile, and investment settings.

use serde::{Deserialize, Serialize};

/// Structured user preferences stored in preferences_json.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserPreferences {
    // === UI Settings ===
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub ui_theme: String,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default)]
    pub email_notifications: bool,
    #[serde(default = "default_true")]
    pub desktop_notifications: bool,
    #[serde(default = "default_true")]
    pub analysis_complete_notification: bool,
    #[serde(default = "default_true")]
    pub system_maintenance_notification: bool,
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,

    // === Analysis Defaults ===
    #[serde(default = "default_market")]
    pub default_market: String,
    #[serde(default = "default_analysts")]
    pub default_analysts: Vec<String>,
    #[serde(default = "default_depth")]
    pub default_depth: String,

    // === Investment Profile ===
    #[serde(default)]
    pub risk_preference: String, // low, medium, high
    #[serde(default)]
    pub investment_horizon: String, // short_term, swing, position, long_term
    #[serde(default)]
    pub preferred_markets: Vec<String>, // a_share, hong_kong, us_equity

    // === Watchlist ===
    #[serde(default)]
    pub watchlist: Vec<WatchlistItem>,

    // === Guidance Preferences ===
    #[serde(default = "default_true")]
    pub guidance_auto_refresh: bool,
    #[serde(default)]
    pub guidance_profile: String, // conservative, balanced, aggressive
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub symbol: String,
    pub name: String,
    pub market: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub added_at: String,
}

fn default_true() -> bool {
    true
}
fn default_refresh_interval() -> u64 {
    60
}
fn default_sidebar_width() -> u32 {
    240
}
fn default_market() -> String {
    "A股".to_string()
}
fn default_analysts() -> Vec<String> {
    vec![
        "market".to_string(),
        "fundamentals".to_string(),
        "news".to_string(),
    ]
}
fn default_depth() -> String {
    "3".to_string()
}

impl UserPreferences {
    pub fn from_json(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn add_to_watchlist(&mut self, item: WatchlistItem) -> bool {
        if self
            .watchlist
            .iter()
            .any(|w| w.symbol == item.symbol && w.market == item.market)
        {
            return false; // Already exists
        }
        self.watchlist.push(item);
        true
    }

    pub fn remove_from_watchlist(&mut self, symbol: &str, market: &str) -> bool {
        let len = self.watchlist.len();
        self.watchlist
            .retain(|w| !(w.symbol == symbol && w.market == market));
        self.watchlist.len() < len
    }
}

#[cfg(test)]
mod user_prefs_tests {
    use super::*;

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
        // Default::default() uses Rust defaults, not serde defaults
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
        assert_eq!(prefs.default_analysts, vec!["market", "fundamentals", "news"]);
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
}
