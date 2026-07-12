
/// Market chart data with candles, indicators, and overlays.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportMarketChart {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub market: String,
    #[serde(default)]
    pub adjust: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub provider_used: String,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub candles: Vec<ReportCandle>,
    #[serde(default)]
    pub indicators: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub overlays: Vec<ChartOverlay>,
    #[serde(default)]
    pub trend_lines: Vec<TrendLine>,
}

/// A single candle in the report chart.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportCandle {
    #[serde(default)]
    pub trade_date: String,
    #[serde(default)]
    pub open: f64,
    #[serde(default)]
    pub close: f64,
    #[serde(default)]
    pub high: f64,
    #[serde(default)]
    pub low: f64,
    #[serde(default)]
    pub volume: i64,
    #[serde(default)]
    pub amount: f64,
    #[serde(default)]
    pub amplitude_pct: f64,
    #[serde(default)]
    pub change_pct: f64,
    #[serde(default)]
    pub change_amount: f64,
    #[serde(default)]
    pub turnover_pct: f64,
}

/// A chart overlay (e.g., support/resistance level).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChartOverlay {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: f64,
    #[serde(default)]
    pub emphasis: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrendLinePoint {
    pub date: String,
    pub value: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrendLine {
    pub key: String,
    pub color: String,
    pub points: Vec<TrendLinePoint>,
}
