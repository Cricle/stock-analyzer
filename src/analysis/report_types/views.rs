/// Price context — current price, lookback high/low, and distance metrics.
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

/// Probability view — upside/downside/sideways probabilities with targets.
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
    /// Direction-agnostic labels for clarity:
    /// profit_target = the price level where profit is taken
    /// stop_loss = the price level where loss is cut
    /// These are always correct regardless of trade direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profit_target: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
}

/// A driver contributing to probability assessment.
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

/// Profit/risk view — upside/downside percentages and reward-risk ratio.
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
    // Transparency fields: show calculation basis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calc_entry: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calc_target: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calc_stop: Option<f64>,
    /// Trade direction: "long" for bullish/neutral, "short" for bearish.
    /// For short trades, calc_target is below entry (profit from falling)
    /// and calc_stop is above entry (stop loss if rises).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trade_direction: String,
    /// Explicit trade structure summary for evaluator clarity.
    /// For bearish: explains that entry < stop is correct (short sells high, stops if rises).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trade_summary: String,
}

/// IC (Investment Committee) navigator view with verdict, path, and conditions.
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

/// Weighted signal resolution combining volume, momentum, and overbought signals.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SignalResolution {
    #[serde(default)]
    pub weighted_score: f64,
    #[serde(default)]
    pub volume_weight: f64,
    #[serde(default)]
    pub momentum_weight: f64,
    #[serde(default)]
    pub overbought_weight: f64,
    #[serde(default)]
    pub dominant_signal: String,
}

/// IC discipline view with state, signals, and price levels.
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
    #[serde(default)]
    pub signal_resolution: SignalResolution,
}

/// Organized view of technical indicators by category with conclusions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorView {
    #[serde(default)]
    pub categories: Vec<TechnicalIndicatorCategory>,
    #[serde(default)]
    pub conclusions: Vec<TechnicalIndicatorConclusion>,
}

/// A category of technical indicators (e.g., trend, momentum, volume).
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

/// A single technical indicator with value, signal, and interpretation.
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

/// A conclusion drawn from technical indicator analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TechnicalIndicatorConclusion {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub evidence_keys: Vec<String>,
}

/// A single evidence card in the report (metric, direction, strength).
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

/// A structured news insight with impact analysis and interpretation.
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

/// A named risk control with probability, impact, and defense action.
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
