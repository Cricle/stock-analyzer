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
