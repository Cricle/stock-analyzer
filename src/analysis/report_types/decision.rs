
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

    pub fn with_param(mut self, k: impl Into<String>, v: serde_json::Value) -> Self {
        self.params.insert(k.into(), v);
        self
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

    pub fn value_str(&self) -> &str {
        self.params
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or(self.key.as_str())
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
    /// LLM did not extract a clear recommendation — use analyst signals
    /// instead of treating as Hold.
    Unknown,
}

impl Rating {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "buy" => Self::Buy,
            "overweight" => Self::Overweight,
            "hold" => Self::Hold,
            "underweight" => Self::Underweight,
            "sell" => Self::Sell,
            _ => Self::Unknown,
        }
    }

    pub fn is_bullish(&self) -> bool {
        matches!(self, Self::Buy | Self::Overweight)
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, Self::Sell | Self::Underweight)
    }

    pub fn is_neutral(&self) -> bool {
        matches!(self, Self::Hold | Self::Unknown)
    }

    pub fn bias(&self, magnitude: i32) -> i32 {
        match self {
            Self::Buy => magnitude,
            Self::Overweight => (magnitude * 3) / 4,
            Self::Hold | Self::Unknown => 0,
            Self::Underweight => -((magnitude * 3) / 4),
            Self::Sell => -magnitude,
        }
    }

    pub fn to_score(&self) -> i32 {
        match self {
            Self::Buy => 2,
            Self::Overweight => 1,
            Self::Hold | Self::Unknown => 0,
            Self::Underweight => -1,
            Self::Sell => -2,
        }
    }

    pub fn to_action_group(&self) -> &'static str {
        match self {
            Self::Buy | Self::Overweight => "Buy",
            Self::Hold | Self::Unknown => "Hold",
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
            Self::Unknown => "Unknown",
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
    pub entry_derivation: LocalText,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub invalidation_level: String,
    #[serde(default)]
    pub target_type: DecisionTargetType,
    #[serde(default)]
    pub target_reference: LocalText,
    #[serde(default)]
    pub first_target: String,
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

