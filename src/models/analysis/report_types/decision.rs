
/// Localized text placeholder returned in API responses.
/// The frontend uses `key` to look up the i18n template and fills in `params`.
///
/// Deserializes from both `{"key": "...", "params": {...}}` (new format)
/// and plain `"string"` (legacy format → `key` = the string, `params` = empty).
#[derive(Clone, Debug, Default, Serialize)]
pub struct LocalText {
    pub key: String,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, serde_json::Value>,
}

impl<'de> serde::Deserialize<'de> for LocalText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct LocalTextVisitor;

        impl<'de> de::Visitor<'de> for LocalTextVisitor {
            type Value = LocalText;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a LocalText object or a plain string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<LocalText, E> {
                Ok(LocalText::new(v))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<LocalText, E> {
                Ok(LocalText::new(v))
            }

            fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<LocalText, M::Error> {
                let mut key = None::<String>;
                let mut params = None::<serde_json::Map<String, serde_json::Value>>;
                while let Some(k) = map.next_key::<String>()? {
                    match k.as_str() {
                        "key" => key = Some(map.next_value()?),
                        "params" => params = Some(map.next_value()?),
                        _ => { let _ = map.next_value::<serde_json::Value>()?; }
                    }
                }
                Ok(LocalText {
                    key: key.ok_or_else(|| de::Error::missing_field("key"))?,
                    params: params.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(LocalTextVisitor)
    }
}

impl LocalText {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into(), params: serde_json::Map::new() }
    }

    pub fn with_str(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.params.insert(k.into(), serde_json::Value::String(v.into()));
        self
    }

    pub fn with_f64(mut self, k: impl Into<String>, v: f64) -> Self {
        self.params.insert(k.into(), serde_json::json!(v));
        self
    }

    pub fn with_i32(mut self, k: impl Into<String>, v: i32) -> Self {
        self.params.insert(k.into(), serde_json::json!(v));
        self
    }

    pub fn with_bool(mut self, k: impl Into<String>, v: bool) -> Self {
        self.params.insert(k.into(), serde_json::json!(v));
        self
    }
}

impl From<&str> for LocalText {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for LocalText {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for LocalText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key)
    }
}

impl PartialEq for LocalText {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for LocalText {}

impl LocalText {
    pub fn as_str(&self) -> &str {
        &self.key
    }

    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }

    pub fn trim(&self) -> &str {
        self.key.trim()
    }

    pub fn split<'a, 'b>(&'a self, pat: &'b str) -> std::str::Split<'a, &'b str> {
        self.key.split(pat)
    }

    pub fn contains(&self, pat: &str) -> bool {
        self.key.contains(pat)
    }

    pub fn starts_with(&self, pat: &str) -> bool {
        self.key.starts_with(pat)
    }

    pub fn to_ascii_lowercase(&self) -> String {
        self.key.to_ascii_lowercase()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum Rating {
    Buy,
    Overweight,
    #[default]
    Hold,
    Underweight,
    Sell,
}

impl Rating {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "buy" => Self::Buy,
            "overweight" => Self::Overweight,
            "hold" => Self::Hold,
            "underweight" => Self::Underweight,
            "sell" => Self::Sell,
            _ => Self::Hold,
        }
    }

    pub fn is_bullish(&self) -> bool {
        matches!(self, Self::Buy | Self::Overweight)
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, Self::Sell | Self::Underweight)
    }

    pub fn bias(&self, magnitude: i32) -> i32 {
        match self {
            Self::Buy => magnitude,
            Self::Overweight => (magnitude * 3) / 4,
            Self::Hold => 0,
            Self::Underweight => -((magnitude * 3) / 4),
            Self::Sell => -magnitude,
        }
    }

    pub fn to_score(&self) -> i32 {
        match self {
            Self::Buy => 2,
            Self::Overweight => 1,
            Self::Hold => 0,
            Self::Underweight => -1,
            Self::Sell => -2,
        }
    }

    pub fn to_action_group(&self) -> &'static str {
        match self {
            Self::Buy | Self::Overweight => "Buy",
            Self::Hold => "Hold",
            Self::Sell | Self::Underweight => "Sell",
        }
    }
}

impl std::fmt::Display for Rating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Buy => "Buy",
            Self::Overweight => "Overweight",
            Self::Hold => "Hold",
            Self::Underweight => "Underweight",
            Self::Sell => "Sell",
        };
        write!(f, "{value}")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportFlavor {
    #[default]
    Execution,
    CoreResearch,
    TradeNote,
    IcChair,
    AppendixReliability,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoreResearchCall {
    LeanBuy,
    BuyOnConfirmation,
    #[default]
    Neutral,
    LeanSell,
    SellOnBreak,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionMode {
    CoreResearch,
    Execution,
    #[default]
    Blocked,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfidenceProfile {
    #[serde(default)]
    pub direction_confidence: ScoreDimension,
    #[serde(default)]
    pub execution_confidence: ScoreDimension,
    #[serde(default)]
    pub evidence_completeness: ScoreDimension,
    #[serde(default)]
    pub historical_calibration: ScoreDimension,
    #[serde(default)]
    pub total_confidence: i32,
    #[serde(default, skip_serializing)]
    pub methodology: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExecutionReadiness {
    #[serde(default)]
    pub execution_boundary_complete: bool,
    #[serde(default)]
    pub missing_execution_fields: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    #[serde(default)]
    pub forced_hold: bool,
    #[serde(default)]
    pub forced_hold_reason: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionViewDirection {
    Bullish,
    #[default]
    Neutral,
    Bearish,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionExecutionState {
    Ready,
    Conditional,
    #[default]
    Watchlist,
    Blocked,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    BuyNow,
    ProbePosition,
    WaitBreakout,
    WaitRetest,
    #[default]
    Hold,
    Reduce,
    Exit,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionActionBias {
    AddRisk,
    #[default]
    KeepRisk,
    ReduceRisk,
    NoTrade,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionConfidenceBand {
    High,
    #[default]
    Medium,
    Low,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionTimeframe {
    ShortTerm,
    Swing,
    Position,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionTargetType {
    Point,
    Range,
    Conditional,
    Open,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThesisState {
    #[default]
    Intact,
    Improving,
    Weakening,
    Broken,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecisionView {
    #[serde(default)]
    pub view: DecisionViewDirection,
    #[serde(default)]
    pub execution_state: DecisionExecutionState,
    #[serde(default)]
    pub action: DecisionAction,
    #[serde(default)]
    pub action_bias: DecisionActionBias,
    #[serde(default)]
    pub confidence_band: DecisionConfidenceBand,
    #[serde(default)]
    pub timeframe: DecisionTimeframe,
    #[serde(default)]
    pub entry_reference: String,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub invalidation_level: String,
    #[serde(default)]
    pub target_type: DecisionTargetType,
    #[serde(default)]
    pub target_reference: LocalText,
    #[serde(default)]
    pub target_condition: LocalText,
    #[serde(default)]
    pub thesis_state: ThesisState,
    #[serde(default)]
    pub primary_path: String,
    #[serde(default)]
    pub primary_path_key: String,
    #[serde(default)]
    pub primary_path_call: LocalText,
    #[serde(default)]
    pub path_bias_rationale: LocalText,
    #[serde(default)]
    pub advance_probe_opinion: LocalText,
    #[serde(default)]
    pub abort_plan: LocalText,
    #[serde(default)]
    pub next_upgrade_condition: LocalText,
    #[serde(default)]
    pub next_downgrade_condition: LocalText,
    #[serde(default)]
    pub sizing_guidance: LocalText,
    #[serde(default)]
    pub reader_summary: LocalText,
    #[serde(default)]
    pub tilt: CoreResearchCall,
    #[serde(default)]
    pub decision_mode: DecisionMode,
    #[serde(default)]
    pub state_line: LocalText,
    #[serde(default)]
    pub action_line: LocalText,
    #[serde(default)]
    pub risk_line: LocalText,
    #[serde(default)]
    pub current_price: String,
    #[serde(default)]
    pub confirmation_price: String,
    #[serde(default)]
    pub invalidation_price: String,
    #[serde(default)]
    pub distance_to_confirmation_pct: f64,
    #[serde(default)]
    pub distance_to_invalidation_pct: f64,
    #[serde(default)]
    pub early_probe_allowed: bool,
    #[serde(default)]
    pub early_probe_trigger: LocalText,
    #[serde(default)]
    pub early_probe_max_size: LocalText,
    #[serde(default)]
    pub wait_cost: LocalText,
}

#[cfg(test)]
mod decision_tests {
    use super::*;

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
        assert_eq!(Rating::parse("unknown"), Rating::Hold);
    }

    #[test]
    fn rating_is_bullish() {
        assert!(Rating::Buy.is_bullish());
        assert!(Rating::Overweight.is_bullish());
        assert!(!Rating::Hold.is_bullish());
        assert!(!Rating::Sell.is_bullish());
    }

    #[test]
    fn rating_is_bearish() {
        assert!(Rating::Sell.is_bearish());
        assert!(Rating::Underweight.is_bearish());
        assert!(!Rating::Hold.is_bearish());
        assert!(!Rating::Buy.is_bearish());
    }

    #[test]
    fn rating_is_neutral() {
        assert!(Rating::Hold.is_neutral());
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
}
