use sa::analysis::{ReportCandle, ReportMarketChart};
use sa::analysis::{add_overlay, compute_trend_lines, derive_price_context};

fn make_candle(date: &str, close: f64, high: f64, low: f64, volume: i64) -> ReportCandle {
    ReportCandle {
        trade_date: date.into(),
        open: close,
        close,
        high,
        low,
        volume,
        ..Default::default()
    }
}

fn sample_candles(n: usize) -> Vec<ReportCandle> {
    (0..n)
        .map(|i| {
            let date = format!("2026-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1);
            let close = 100.0 + (i as f64) * 0.5;
            make_candle(&date, close, close + 2.0, close - 2.0, 1000 + i as i64 * 10)
        })
        .collect()
}

#[test]
fn compute_trend_lines_insufficient_data() {
    let candles = sample_candles(30);
    let lines = compute_trend_lines(&candles);
    assert!(lines.is_empty());
}

#[test]
fn compute_trend_lines_with_sma50() {
    let candles = sample_candles(60);
    let lines = compute_trend_lines(&candles);
    let sma50 = lines.iter().find(|l| l.key == "sma_50");
    assert!(sma50.is_some(), "expected sma_50 trend line");
}

#[test]
fn compute_trend_lines_with_regression() {
    let candles = sample_candles(60);
    let lines = compute_trend_lines(&candles);
    let reg = lines.iter().find(|l| l.key == "regression");
    assert!(reg.is_some(), "expected regression trend line");
}

#[test]
fn compute_trend_lines_regression_points_count() {
    let candles = sample_candles(100);
    let lines = compute_trend_lines(&candles);
    let reg = lines.iter().find(|l| l.key == "regression").unwrap();
    assert_eq!(reg.points.len(), 100);
}

#[test]
fn add_overlay_skips_non_positive() {
    let mut overlays = Vec::new();
    add_overlay(&mut overlays, "test", Some(-1.0), "info");
    assert!(overlays.is_empty());
}

#[test]
fn add_overlay_skips_nan() {
    let mut overlays = Vec::new();
    add_overlay(&mut overlays, "test", Some(f64::NAN), "info");
    assert!(overlays.is_empty());
}

#[test]
fn add_overlay_skips_none() {
    let mut overlays = Vec::new();
    add_overlay(&mut overlays, "test", None, "info");
    assert!(overlays.is_empty());
}

#[test]
fn add_overlay_adds_valid() {
    let mut overlays = Vec::new();
    add_overlay(&mut overlays, "price", Some(150.0), "primary");
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].key, "price");
    assert_eq!(overlays[0].value, 150.0);
}

#[test]
fn add_overlay_deduplicates() {
    let mut overlays = Vec::new();
    add_overlay(&mut overlays, "price", Some(150.0), "primary");
    add_overlay(&mut overlays, "price", Some(160.0), "info");
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].value, 150.0);
}

#[test]
fn derive_price_context_empty_chart() {
    let chart = ReportMarketChart::default();
    let ctx = derive_price_context(&chart, None);
    assert!(ctx.current_price.is_none());
    assert!(ctx.high_price.is_none());
}

#[test]
fn derive_price_context_with_candles() {
    let candles = vec![
        make_candle("2026-01-01", 100.0, 105.0, 95.0, 1000),
        make_candle("2026-01-02", 110.0, 115.0, 105.0, 1200),
        make_candle("2026-01-03", 105.0, 112.0, 98.0, 800),
    ];
    let chart = ReportMarketChart {
        candles,
        ..Default::default()
    };
    let ctx = derive_price_context(&chart, None);
    assert_eq!(ctx.current_price, Some(105.0));
    assert_eq!(ctx.high_price, Some(115.0));
    assert_eq!(ctx.low_price, Some(95.0));
}

#[test]
fn derive_price_context_distance_calculations() {
    let candles = vec![
        make_candle("2026-01-01", 100.0, 100.0, 100.0, 1000),
        make_candle("2026-01-02", 110.0, 120.0, 90.0, 1000),
    ];
    let chart = ReportMarketChart {
        candles,
        ..Default::default()
    };
    let ctx = derive_price_context(&chart, None);
    // current=110, high=120, low=90
    // distance_to_high = (120-110)/110 * 100 = 9.09%
    assert!(ctx.distance_to_high_pct.unwrap() > 8.0);
    assert!(ctx.distance_to_low_pct.unwrap() > 18.0);
}

#[test]
fn derive_price_context_override_current_price() {
    let candles = vec![make_candle("2026-01-01", 100.0, 105.0, 95.0, 1000)];
    let chart = ReportMarketChart {
        candles,
        ..Default::default()
    };
    let ctx = derive_price_context(&chart, Some(200.0));
    assert_eq!(ctx.current_price, Some(200.0));
}
