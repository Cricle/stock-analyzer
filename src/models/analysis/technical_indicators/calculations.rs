
fn obv_signal(delta: Option<f64>) -> &'static str {
    match delta {
        Some(value) if value > 0.0 => "volume_accumulation",
        Some(value) if value < 0.0 => "volume_distribution",
        Some(_) => "volume_neutral",
        None => "unavailable",
    }
}

pub fn sma_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    Some(slice.iter().map(|item| item.close).sum::<f64>() / period as f64)
}

pub fn ema_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = sma_report(&candles[..period], period)?;
    for candle in &candles[period..] {
        ema = (candle.close - ema) * multiplier + ema;
    }
    Some(ema)
}

pub fn rsi_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for pair in candles[candles.len() - period - 1..].windows(2) {
        let change = pair[1].close - pair[0].close;
        if change >= 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    if losses == 0.0 {
        return Some(100.0);
    }
    let rs = gains / losses;
    Some(100.0 - 100.0 / (1.0 + rs))
}

pub fn atr_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    let ranges = candles
        .windows(2)
        .map(|pair| {
            let current = &pair[1];
            let prev = &pair[0];
            let high_low = current.high - current.low;
            let high_close = (current.high - prev.close).abs();
            let low_close = (current.low - prev.close).abs();
            high_low.max(high_close).max(low_close)
        })
        .collect::<Vec<_>>();
    let slice = &ranges[ranges.len().saturating_sub(period)..];
    Some(slice.iter().sum::<f64>() / slice.len() as f64)
}

fn vwma_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|item| item.volume as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(slice.iter().map(|item| item.close * item.volume as f64).sum::<f64>() / volume_sum)
}

pub fn bollinger_report(candles: &[ReportCandle], period: usize) -> Option<(f64, f64, f64)> {
    let mid = sma_report(candles, period)?;
    let slice = &candles[candles.len() - period..];
    let variance = slice
        .iter()
        .map(|item| {
            let diff = item.close - mid;
            diff * diff
        })
        .sum::<f64>()
        / period as f64;
    let stddev = variance.sqrt();
    Some((mid, mid + stddev * 2.0, mid - stddev * 2.0))
}

pub fn macd_report(candles: &[ReportCandle]) -> Option<(f64, f64, f64)> {
    if candles.len() < 35 {
        return None;
    }
    let ema12 = ema_series_report(candles, 12)?;
    let ema26 = ema_series_report(candles, 26)?;
    let offset = ema12.len().saturating_sub(ema26.len());
    let macd_series = ema12[offset..]
        .iter()
        .zip(ema26.iter())
        .map(|(fast, slow)| fast - slow)
        .collect::<Vec<_>>();
    let signal = ema_values_report(&macd_series, 9)?;
    let macd = *macd_series.last()?;
    let signal_last = *signal.last()?;
    Some((macd, signal_last, macd - signal_last))
}

fn ema_series_report(candles: &[ReportCandle], period: usize) -> Option<Vec<f64>> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut values = Vec::new();
    let mut ema = candles[..period].iter().map(|item| item.close).sum::<f64>() / period as f64;
    values.push(ema);
    for candle in &candles[period..] {
        ema = (candle.close - ema) * multiplier + ema;
        values.push(ema);
    }
    Some(values)
}

fn ema_values_report(values: &[f64], period: usize) -> Option<Vec<f64>> {
    if values.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut out = Vec::new();
    let mut ema = values[..period].iter().sum::<f64>() / period as f64;
    out.push(ema);
    for value in &values[period..] {
        ema = (value - ema) * multiplier + ema;
        out.push(ema);
    }
    Some(out)
}

pub fn kdj_report(candles: &[ReportCandle], period: usize) -> Option<(f64, f64, f64)> {
    if candles.len() < period {
        return None;
    }
    let mut k = 50.0;
    let mut d = 50.0;
    for index in period - 1..candles.len() {
        let slice = &candles[index + 1 - period..=index];
        let high = slice.iter().map(|item| item.high).fold(f64::NEG_INFINITY, f64::max);
        let low = slice.iter().map(|item| item.low).fold(f64::INFINITY, f64::min);
        let close = candles[index].close;
        let rsv = if high > low {
            ((close - low) / (high - low)) * 100.0
        } else {
            50.0
        };
        k = (2.0 / 3.0) * k + (1.0 / 3.0) * rsv;
        d = (2.0 / 3.0) * d + (1.0 / 3.0) * k;
    }
    let j = 3.0 * k - 2.0 * d;
    Some((k, d, j))
}

fn cci_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let typical = slice.iter().map(|item| (item.high + item.low + item.close) / 3.0).collect::<Vec<_>>();
    let ma = typical.iter().sum::<f64>() / period as f64;
    let mean_deviation = typical.iter().map(|value| (value - ma).abs()).sum::<f64>() / period as f64;
    if mean_deviation <= f64::EPSILON {
        return None;
    }
    let last = *typical.last()?;
    Some((last - ma) / (0.015 * mean_deviation))
}

fn wr_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let high = slice.iter().map(|item| item.high).fold(f64::NEG_INFINITY, f64::max);
    let low = slice.iter().map(|item| item.low).fold(f64::INFINITY, f64::min);
    let close = slice.last()?.close;
    if high <= low {
        return None;
    }
    Some(((high - close) / (high - low)) * -100.0)
}

pub fn adx_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() <= period + 1 {
        return None;
    }
    let mut dx_values = Vec::new();
    for window in candles.windows(period + 1) {
        let mut plus_dm = 0.0;
        let mut minus_dm = 0.0;
        let mut tr_sum = 0.0;
        for pair in window.windows(2) {
            let prev = &pair[0];
            let current = &pair[1];
            let up_move = current.high - prev.high;
            let down_move = prev.low - current.low;
            if up_move > down_move && up_move > 0.0 {
                plus_dm += up_move;
            }
            if down_move > up_move && down_move > 0.0 {
                minus_dm += down_move;
            }
            tr_sum += (current.high - current.low)
                .max((current.high - prev.close).abs())
                .max((current.low - prev.close).abs());
        }
        if tr_sum <= f64::EPSILON {
            continue;
        }
        let plus_di = 100.0 * plus_dm / tr_sum;
        let minus_di = 100.0 * minus_dm / tr_sum;
        let denom = plus_di + minus_di;
        if denom > f64::EPSILON {
            dx_values.push(((plus_di - minus_di).abs() / denom) * 100.0);
        }
    }
    let slice = &dx_values[dx_values.len().saturating_sub(period)..];
    (!slice.is_empty()).then_some(slice.iter().sum::<f64>() / slice.len() as f64)
}

pub fn obv_report(candles: &[ReportCandle]) -> Option<(f64, f64)> {
    if candles.len() < 2 {
        return None;
    }
    let mut obv = 0.0;
    let mut prev_obv = 0.0;
    for pair in candles.windows(2) {
        prev_obv = obv;
        let prev = &pair[0];
        let current = &pair[1];
        if current.close > prev.close {
            obv += current.volume as f64;
        } else if current.close < prev.close {
            obv -= current.volume as f64;
        }
    }
    Some((obv, obv - prev_obv))
}

fn vwap_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|item| item.volume as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(
        slice
            .iter()
            .map(|item| ((item.high + item.low + item.close) / 3.0) * item.volume as f64)
            .sum::<f64>()
            / volume_sum,
    )
}

#[cfg(test)]
mod technical_calc_tests {
    use super::super::*;

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
}
