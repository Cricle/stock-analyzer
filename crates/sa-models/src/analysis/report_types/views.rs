
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PriceContext {
    #[serde(default)]
    pub current_price: Option<f64>,
    #[serde(default)]
    pub lookback_days: usize,
    #[serde(default)]
    pub high_price: Option<f64>,
    #[serde(default)]
    pub high_date: String,
    #[serde(default)]
    pub low_price: Option<f64>,
    #[serde(default)]
    pub low_date: String,
    #[serde(default)]
    pub distance_to_high_pct: Option<f64>,
    #[serde(default)]
    pub distance_to_low_pct: Option<f64>,
    #[serde(default)]
    pub range_pct: Option<f64>,
    #[serde(default)]
    pub latest_volume: Option<i64>,
    #[serde(default)]
    pub volume_change_pct: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbabilityView {
    #[serde(default)]
    pub upside_probability_pct: f64,
    #[serde(default)]
    pub upside_target: Option<f64>,
    #[serde(default)]
    pub upside_pct: Option<f64>,
    #[serde(default)]
    pub downside_probability_pct: f64,
    #[serde(default)]
    pub downside_target: Option<f64>,
    #[serde(default)]
    pub downside_pct: Option<f64>,
    #[serde(default)]
    pub sideways_probability_pct: f64,
    #[serde(default)]
    pub risk_probability_pct: f64,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence_band: LocalText,
    #[serde(default)]
    pub drivers: Vec<ProbabilityDriver>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbabilityDriver {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub evidence_keys: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfitRiskView {
    #[serde(default)]
    pub upside_pct: Option<f64>,
    #[serde(default)]
    pub downside_pct: Option<f64>,
    #[serde(default)]
    pub reward_risk_ratio: Option<f64>,
    #[serde(default)]
    pub current_position_reward_risk_ratio: Option<f64>,
    #[serde(default)]
    pub max_loss_reference: Option<f64>,
    #[serde(default)]
    pub risk_budget: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub actionability: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IcNavigatorView {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub verdict: LocalText,
    #[serde(default)]
    pub primary_path_key: String,
    #[serde(default)]
    pub path_probability_pct: f64,
    #[serde(default)]
    pub confidence_band: String,
    #[serde(default)]
    pub can_act_now: bool,
    #[serde(default)]
    pub early_probe_allowed: bool,
    #[serde(default)]
    pub upgrade_condition: LocalText,
    #[serde(default)]
    pub abort_condition: LocalText,
    #[serde(default)]
    pub responsibility: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IcDisciplineView {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub state: LocalText,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub next_action_code: LocalText,
    #[serde(default)]
    pub reward_risk_ratio: Option<f64>,
    #[serde(default)]
    pub current_position_reward_risk_ratio: Option<f64>,
    #[serde(default)]
    pub rsi: Option<f64>,
    #[serde(default)]
    pub macd: Option<f64>,
    #[serde(default)]
    pub upside_probability_pct: f64,
    #[serde(default)]
    pub downside_probability_pct: f64,
    #[serde(default)]
    pub risk_probability_pct: f64,
    #[serde(default)]
    pub current_price: Option<f64>,
    #[serde(default)]
    pub confirmation_price: Option<f64>,
    #[serde(default)]
    pub invalidation_price: Option<f64>,
    #[serde(default)]
    pub upside_pct: Option<f64>,
    #[serde(default)]
    pub downside_pct: Option<f64>,
    #[serde(default)]
    pub technical_signal_codes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorView {
    #[serde(default)]
    pub categories: Vec<TechnicalIndicatorCategory>,
    #[serde(default)]
    pub conclusions: Vec<TechnicalIndicatorConclusion>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorCategory {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub display_mode: String,
    #[serde(default)]
    pub signal_attribute: String,
    #[serde(default)]
    pub indicators: Vec<TechnicalIndicatorItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorItem {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub signal_code: String,
    #[serde(default)]
    pub interpretation_code: String,
    #[serde(default)]
    pub display_mode: String,
    #[serde(default)]
    pub signal_attribute: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorConclusion {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub evidence_keys: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportEvidenceCard {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub strength: String,
    #[serde(default)]
    pub source: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub claim: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewsInsight {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub fact_summary: LocalText,
    #[serde(default)]
    pub interpretation: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub impact_direction: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub impact_strength: LocalText,
    #[serde(default)]
    pub what_it_confirms: LocalText,
    #[serde(default)]
    pub what_to_watch_next: LocalText,
    /// True when the news item's date is on or before the analysis date,
    /// meaning the market has already had a chance to react to this catalyst.
    #[serde(default)]
    pub published_before_analysis: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskControl {
    #[serde(default)]
    pub risk_name: LocalText,
    #[serde(default)]
    pub probability_pct: f64,
    #[serde(default)]
    pub impact: LocalText,
    #[serde(default)]
    pub trigger: LocalText,
    #[serde(default)]
    pub defense_action: LocalText,
    #[serde(default)]
    pub invalidation_level: String,
    #[serde(default)]
    pub monitoring_signal: LocalText,
}
