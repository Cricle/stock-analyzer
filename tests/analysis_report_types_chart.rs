use stock_analyzer::analysis::{
    ChartOverlay, ReportCandle, ReportMarketChart, TrendLine, TrendLinePoint,
};

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
            open: 149.0,
            close: 150.0,
            high: 151.0,
            low: 148.0,
            volume: 1000000,
            amount: 150000000.0,
            amplitude_pct: 2.0,
            change_pct: 0.67,
            change_amount: 1.0,
            turnover_pct: 0.5,
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
        open: 149.0,
        close: 150.0,
        high: 151.0,
        low: 148.0,
        volume: 1000000,
        amount: 150000000.0,
        amplitude_pct: 2.0,
        change_pct: 0.67,
        change_amount: 1.0,
        turnover_pct: 0.5,
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: ReportCandle = serde_json::from_str(&json).unwrap();
    assert!((restored.close - 150.0).abs() < 0.001);
}

#[test]
fn chart_overlay_serde_roundtrip() {
    let o = ChartOverlay {
        key: "ma10".into(),
        value: 148.5,
        emphasis: "high".into(),
    };
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
            TrendLinePoint {
                date: "2025-01-01".into(),
                value: 155.0,
            },
            TrendLinePoint {
                date: "2025-01-15".into(),
                value: 152.0,
            },
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
