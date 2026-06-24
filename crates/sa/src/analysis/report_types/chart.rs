
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

#[cfg(test)]
mod chart_tests {
    use super::*;

    #[test]
    fn report_market_chart_serde_roundtrip() {
        let c = ReportMarketChart {
            symbol: "AAPL".into(),
            market: "美股".into(),
            adjust: "qfq".into(),
            source: "eastmoney".into(),
            provider_used: "eastmoney".into(),
            start_date: "2025-01-01".into(),
            end_date: "2025-01-15".into(),
            candles: vec![ReportCandle {
                trade_date: "2025-01-15".into(),
                open: 149.0, close: 150.0, high: 151.0, low: 148.0,
                volume: 1000000, amount: 150000000.0,
                amplitude_pct: 2.0, change_pct: 0.67,
                change_amount: 1.0, turnover_pct: 0.5,
            }],
            indicators: vec![],
            overlays: vec![ChartOverlay {
                key: "ma5".into(),
                value: 150.0,
                emphasis: "normal".into(),
            }],
            trend_lines: vec![TrendLine {
                key: "support".into(),
                color: "green".into(),
                points: vec![TrendLinePoint {
                    date: "2025-01-01".into(),
                    value: 145.0,
                }],
            }],
        };
        let json = serde_json::to_string(&c).unwrap();
        let restored: ReportMarketChart = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.symbol, "AAPL");
        assert_eq!(restored.candles.len(), 1);
        assert_eq!(restored.overlays.len(), 1);
        assert_eq!(restored.trend_lines.len(), 1);
    }

    #[test]
    fn report_candle_serde_roundtrip() {
        let c = ReportCandle {
            trade_date: "2025-01-15".into(),
            open: 149.0, close: 150.0, high: 151.0, low: 148.0,
            volume: 1000000, amount: 150000000.0,
            amplitude_pct: 2.0, change_pct: 0.67,
            change_amount: 1.0, turnover_pct: 0.5,
        };
        let json = serde_json::to_string(&c).unwrap();
        let restored: ReportCandle = serde_json::from_str(&json).unwrap();
        assert!((restored.close - 150.0).abs() < 0.001);
    }

    #[test]
    fn chart_overlay_serde_roundtrip() {
        let o = ChartOverlay { key: "ma10".into(), value: 148.5, emphasis: "high".into() };
        let json = serde_json::to_string(&o).unwrap();
        let restored: ChartOverlay = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key, "ma10");
    }

    #[test]
    fn trend_line_serde_roundtrip() {
        let tl = TrendLine {
            key: "resistance".into(),
            color: "red".into(),
            points: vec![
                TrendLinePoint { date: "2025-01-01".into(), value: 155.0 },
                TrendLinePoint { date: "2025-01-15".into(), value: 152.0 },
            ],
        };
        let json = serde_json::to_string(&tl).unwrap();
        let restored: TrendLine = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.points.len(), 2);
    }

    #[test]
    fn all_defaults() {
        assert!(ReportMarketChart::default().symbol.is_empty());
        assert!(ReportMarketChart::default().candles.is_empty());
        assert!(ChartOverlay::default().key.is_empty());
        assert!(TrendLinePoint::default().date.is_empty());
        assert!(TrendLine::default().points.is_empty());
    }
}
