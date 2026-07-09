
/// New report output structure: separates Analysis from TradePlan.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewStructuredReport {
    /// Analysis section (always present)
    pub analysis: NewAnalysis,

    /// Trade plan (only for active trades)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_plan: Option<NewTradePlan>,

    /// Metadata
    pub meta: ReportMeta,
}

/// Analysis section: what we think about the stock.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewAnalysis {
    /// One-paragraph overview
    #[serde(default)]
    pub summary: LocalText,

    /// -100 to +100 (bearish to bullish)
    #[serde(default)]
    pub direction_score: i32,

    /// Breakdown of direction score components
    #[serde(default)]
    pub direction_breakdown: DirectionBreakdown,

    /// 0-100 confidence score
    #[serde(default)]
    pub confidence_score: i32,

    /// Current price, range, volume
    #[serde(default)]
    pub price_context: PriceContext,

    /// Probability and risk/reward (always present)
    #[serde(default)]
    pub probability: NewProbabilityAnalysis,

    /// Technical indicators (RSI, MACD, etc.)
    #[serde(default)]
    pub technical_indicators: TechnicalIndicatorView,

    /// Recent news insights
    #[serde(default)]
    pub news_insights: Vec<NewsInsight>,

    /// Key risk factors
    #[serde(default)]
    pub risk_factors: Vec<NewRiskFactor>,

    /// Arguments for bull and bear cases
    #[serde(default)]
    pub bull_bear_case: NewBullBearCase,

    /// Calibration and setup quality
    #[serde(default)]
    pub calibration: CalibrationSummary,

    /// Research reliability
    #[serde(default)]
    pub reliability: ResearchReliability,
}

/// Probability analysis: always present in analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewProbabilityAnalysis {
    /// Upside probability (0-100%)
    #[serde(default)]
    pub upside_probability_pct: f64,

    /// Downside probability (0-100%)
    #[serde(default)]
    pub downside_probability_pct: f64,

    /// Sideways probability (0-100%)
    #[serde(default)]
    pub sideways_probability_pct: f64,

    /// Risk probability (0-100%)
    #[serde(default)]
    pub risk_probability_pct: f64,

    /// Upside target price (HIGHER price, always)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upside_target: Option<f64>,

    /// Downside target price (LOWER price, always)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downside_target: Option<f64>,

    /// Upside percentage (always positive = profit %)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upside_pct: Option<f64>,

    /// Downside percentage (always positive = loss %)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downside_pct: Option<f64>,

    /// Reward/risk ratio (upside_pct / downside_pct)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_risk_ratio: Option<f64>,

    /// Confidence band (high/medium/low)
    #[serde(default)]
    pub confidence_band: LocalText,

    /// Key drivers of the probability
    #[serde(default)]
    pub drivers: Vec<ProbabilityDriver>,
}

/// Trade plan: only present for active trades.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewTradePlan {
    /// Action to take: buy, sell, short, cover
    #[serde(default)]
    pub action: String,

    /// Trade direction: long, short
    #[serde(default)]
    pub direction: String,

    /// Entry price
    #[serde(default)]
    pub entry: f64,

    /// Where to take profit (HIGHER for long, LOWER for short)
    #[serde(default)]
    pub profit_target: f64,

    /// Where to cut loss (LOWER for long, HIGHER for short)
    #[serde(default)]
    pub stop_loss: f64,

    /// Reward/risk ratio
    #[serde(default)]
    pub reward_risk_ratio: f64,

    /// Suggested position size
    #[serde(default)]
    pub position_size: LocalText,

    /// Price to confirm thesis
    #[serde(default)]
    pub confirmation_level: f64,

    /// Price that invalidates thesis
    #[serde(default)]
    pub invalidation_level: f64,

    /// Expected holding period
    #[serde(default)]
    pub time_horizon: LocalText,

    /// Explicit trade structure summary
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trade_summary: String,
}

/// Risk factor in analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewRiskFactor {
    #[serde(default)]
    pub name: LocalText,
    #[serde(default)]
    pub probability_pct: f64,
    #[serde(default)]
    pub impact: LocalText,
    #[serde(default)]
    pub trigger: LocalText,
    #[serde(default)]
    pub defense_action: LocalText,
}

/// Bull/bear case arguments.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewBullBearCase {
    #[serde(default)]
    pub bull_arguments: Vec<LocalText>,
    #[serde(default)]
    pub bear_arguments: Vec<LocalText>,
}

/// Report metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportMeta {
    #[serde(default)]
    pub analysis_date: String,
    #[serde(default)]
    pub stock_code: String,
    #[serde(default)]
    pub stock_name: String,
    #[serde(default)]
    pub market: String,
}
