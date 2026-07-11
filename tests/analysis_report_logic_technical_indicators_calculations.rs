use stock_analyzer::analysis::ReportCandle;
use stock_analyzer::analysis::{
    adx_report, atr_report, bollinger_report, ema_report, kdj_report, macd_report, obv_report,
    obv_signal, rsi_report, sma_report,
};

fn make_candle(close: f64, high: f64, low: f64, volume: i64) -> ReportCandle {
    ReportCandle {
        close,
        high,
        low,
        volume,
        open: close,
        ..Default::default()
    }
}

fn sample_candles() -> Vec<ReportCandle> {
    vec![
        make_candle(100.0, 105.0, 95.0, 1000),
        make_candle(102.0, 106.0, 98.0, 1200),
        make_candle(101.0, 104.0, 97.0, 800),
        make_candle(103.0, 107.0, 99.0, 1500),
        make_candle(105.0, 110.0, 100.0, 2000),
    ]
}

// --- sma_report ---

#[test]
fn sma_report_basic() {
    let candles = sample_candles();
    let sma = sma_report(&candles, 3).unwrap();
    // (101 + 103 + 105) / 3 = 103.0
    assert!((sma - 103.0).abs() < 0.01);
}

#[test]
fn sma_report_insufficient_data() {
    let candles = sample_candles();
    assert!(sma_report(&candles, 10).is_none());
}

// --- ema_report ---

#[test]
fn ema_report_basic() {
    let candles = sample_candles();
    let ema = ema_report(&candles, 3).unwrap();
    assert!(ema > 0.0);
}

#[test]
fn ema_report_insufficient_data() {
    let candles = sample_candles();
    assert!(ema_report(&candles, 10).is_none());
}

// --- rsi_report ---

#[test]
fn rsi_report_basic() {
    let candles = sample_candles();
    let rsi = rsi_report(&candles, 3).unwrap();
    assert!(rsi >= 0.0 && rsi <= 100.0);
}

#[test]
fn rsi_report_insufficient_data() {
    let candles = sample_candles();
    assert!(rsi_report(&candles, 10).is_none());
}

// --- atr_report ---

#[test]
fn atr_report_basic() {
    let candles = sample_candles();
    let atr = atr_report(&candles, 3).unwrap();
    assert!(atr > 0.0);
}

#[test]
fn atr_report_insufficient_data() {
    let candles = sample_candles();
    assert!(atr_report(&candles, 10).is_none());
}

// --- bollinger_report ---

#[test]
fn bollinger_report_basic() {
    let candles = sample_candles();
    let (mid, upper, lower) = bollinger_report(&candles, 3).unwrap();
    assert!(upper > mid);
    assert!(lower < mid);
}

#[test]
fn bollinger_report_insufficient_data() {
    let candles = sample_candles();
    assert!(bollinger_report(&candles, 10).is_none());
}

// --- macd_report ---

#[test]
fn macd_report_insufficient_data() {
    let candles = sample_candles();
    assert!(macd_report(&candles).is_none());
}

// --- kdj_report ---

#[test]
fn kdj_report_basic() {
    let candles = sample_candles();
    let (k, d, j) = kdj_report(&candles, 3).unwrap();
    assert!(k >= 0.0 && k <= 100.0);
    assert!(d >= 0.0 && d <= 100.0);
    // j = 3k - 2d, can be outside 0-100
    assert!((j - (3.0 * k - 2.0 * d)).abs() < 0.01);
}

#[test]
fn kdj_report_insufficient_data() {
    let candles = sample_candles();
    assert!(kdj_report(&candles, 10).is_none());
}

// --- adx_report ---

#[test]
fn adx_report_basic() {
    let candles = sample_candles();
    let adx = adx_report(&candles, 2).unwrap();
    assert!(adx >= 0.0 && adx <= 100.0);
}

#[test]
fn adx_report_insufficient_data() {
    let candles = sample_candles();
    assert!(adx_report(&candles, 10).is_none());
}

// --- obv_report ---

#[test]
fn obv_report_basic() {
    let candles = sample_candles();
    let (obv, delta) = obv_report(&candles).unwrap();
    assert!(obv != 0.0);
    assert!(delta != 0.0);
}

#[test]
fn obv_report_insufficient_data() {
    let candles = vec![make_candle(100.0, 105.0, 95.0, 1000)];
    assert!(obv_report(&candles).is_none());
}

// --- obv_signal ---

#[test]
fn obv_signal_positive() {
    assert_eq!(obv_signal(Some(10.0)), "volume_accumulation");
}

#[test]
fn obv_signal_negative() {
    assert_eq!(obv_signal(Some(-10.0)), "volume_distribution");
}

#[test]
fn obv_signal_zero() {
    assert_eq!(obv_signal(Some(0.0)), "volume_neutral");
}

#[test]
fn obv_signal_none() {
    assert_eq!(obv_signal(None), "unavailable");
}
