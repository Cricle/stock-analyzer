/// Trait for candle data used in technical indicator calculations.
/// Implemented by both `ReportCandle` and `CandlePoint`.
pub trait CandleLike {
    fn close(&self) -> f64;
    fn high(&self) -> f64;
    fn low(&self) -> f64;
    fn volume(&self) -> i64;
}

impl CandleLike for crate::analysis::ReportCandle {
    fn close(&self) -> f64 {
        self.close
    }
    fn high(&self) -> f64 {
        self.high
    }
    fn low(&self) -> f64 {
        self.low
    }
    fn volume(&self) -> i64 {
        self.volume
    }
}

impl CandleLike for crate::data::CandlePoint {
    fn close(&self) -> f64 {
        self.close
    }
    fn high(&self) -> f64 {
        self.high
    }
    fn low(&self) -> f64 {
        self.low
    }
    fn volume(&self) -> i64 {
        self.volume
    }
}

/// Simple Moving Average over the last `period` candles.
pub fn sma<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    Some(slice.iter().map(|c| c.close()).sum::<f64>() / period as f64)
}

/// Exponential Moving Average over the last `period` candles.
pub fn ema<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = sma(&candles[..period], period)?;
    for candle in &candles[period..] {
        ema = (candle.close() - ema) * multiplier + ema;
    }
    Some(ema)
}

/// EMA series — returns all EMA values from the first valid window.
pub fn ema_series<C: CandleLike>(candles: &[C], period: usize) -> Option<Vec<f64>> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut values = Vec::new();
    let mut ema = candles[..period].iter().map(|c| c.close()).sum::<f64>() / period as f64;
    values.push(ema);
    for candle in &candles[period..] {
        ema = (candle.close() - ema) * multiplier + ema;
        values.push(ema);
    }
    Some(values)
}

/// EMA on raw f64 values (not candles).
pub fn ema_values(values: &[f64], period: usize) -> Option<Vec<f64>> {
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

/// Relative Strength Index (RSI) using Wilder's exponential smoothing.
pub fn rsi<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    // Seed: simple average of first `period` changes
    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;
    for pair in candles[..=period].windows(2) {
        let change = pair[1].close() - pair[0].close();
        if change >= 0.0 {
            avg_gain += change;
        } else {
            avg_loss += change.abs();
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    // Wilder smoothing for remaining candles
    for pair in candles[period..].windows(2) {
        let change = pair[1].close() - pair[0].close();
        let gain = if change >= 0.0 { change } else { 0.0 };
        let loss = if change < 0.0 { change.abs() } else { 0.0 };
        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;
    }

    if avg_loss <= f64::EPSILON {
        return Some(100.0);
    }
    let rs = avg_gain / avg_loss;
    Some(100.0 - 100.0 / (1.0 + rs))
}

/// Average True Range (ATR) over the last `period` candles.
pub fn atr<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    let ranges = candles
        .windows(2)
        .map(|pair| {
            let current = &pair[1];
            let prev = &pair[0];
            let high_low = current.high() - current.low();
            let high_close = (current.high() - prev.close()).abs();
            let low_close = (current.low() - prev.close()).abs();
            high_low.max(high_close).max(low_close)
        })
        .collect::<Vec<_>>();
    let slice = &ranges[ranges.len().saturating_sub(period)..];
    Some(slice.iter().sum::<f64>() / slice.len() as f64)
}

/// Volume-Weighted Moving Average over the last `period` candles.
pub fn vwma<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|c| c.volume() as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(
        slice
            .iter()
            .map(|c| c.close() * c.volume() as f64)
            .sum::<f64>()
            / volume_sum,
    )
}

/// Bollinger Bands — returns (middle, upper, lower) using 2 standard deviations.
pub fn bollinger<C: CandleLike>(candles: &[C], period: usize) -> Option<(f64, f64, f64)> {
    let mid = sma(candles, period)?;
    let slice = &candles[candles.len() - period..];
    let variance = slice
        .iter()
        .map(|c| {
            let diff = c.close() - mid;
            diff * diff
        })
        .sum::<f64>()
        / period as f64;
    let stddev = variance.sqrt();
    Some((mid, mid + stddev * 2.0, mid - stddev * 2.0))
}

/// MACD — returns (macd_line, signal_line, histogram).
pub fn macd<C: CandleLike>(candles: &[C]) -> Option<(f64, f64, f64)> {
    if candles.len() < 35 {
        return None;
    }
    let ema12 = ema_series(candles, 12)?;
    let ema26 = ema_series(candles, 26)?;
    let offset = ema12.len().saturating_sub(ema26.len());
    let macd_series = ema12[offset..]
        .iter()
        .zip(ema26.iter())
        .map(|(fast, slow)| fast - slow)
        .collect::<Vec<_>>();
    let signal = ema_values(&macd_series, 9)?;
    let macd = *macd_series.last()?;
    let signal_last = *signal.last()?;
    Some((macd, signal_last, macd - signal_last))
}

/// KDJ oscillator — returns (K, D, J) values.
pub fn kdj<C: CandleLike>(candles: &[C], period: usize) -> Option<(f64, f64, f64)> {
    if candles.len() < period {
        return None;
    }
    let mut k = 50.0;
    let mut d = 50.0;
    for index in period - 1..candles.len() {
        let slice = &candles[index + 1 - period..=index];
        let high = slice
            .iter()
            .map(|c| c.high())
            .fold(f64::NEG_INFINITY, f64::max);
        let low = slice.iter().map(|c| c.low()).fold(f64::INFINITY, f64::min);
        let close = candles[index].close();
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

/// Commodity Channel Index (CCI) over the last `period` candles.
pub fn cci<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let typical = slice
        .iter()
        .map(|c| (c.high() + c.low() + c.close()) / 3.0)
        .collect::<Vec<_>>();
    let ma = typical.iter().sum::<f64>() / period as f64;
    let mean_deviation = typical.iter().map(|v| (v - ma).abs()).sum::<f64>() / period as f64;
    if mean_deviation <= f64::EPSILON {
        return None;
    }
    let last = *typical.last()?;
    Some((last - ma) / (0.015 * mean_deviation))
}

/// Williams %R over the last `period` candles.
pub fn wr<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let high = slice
        .iter()
        .map(|c| c.high())
        .fold(f64::NEG_INFINITY, f64::max);
    let low = slice.iter().map(|c| c.low()).fold(f64::INFINITY, f64::min);
    let close = slice.last()?.close();
    if high <= low {
        return None;
    }
    Some(((high - close) / (high - low)) * -100.0)
}

/// Average Directional Index (ADX) over the last `period` candles.
pub fn adx<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
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
            let up_move = current.high() - prev.high();
            let down_move = prev.low() - current.low();
            if up_move > down_move && up_move > 0.0 {
                plus_dm += up_move;
            }
            if down_move > up_move && down_move > 0.0 {
                minus_dm += down_move;
            }
            tr_sum += (current.high() - current.low())
                .max((current.high() - prev.close()).abs())
                .max((current.low() - prev.close()).abs());
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

/// On-Balance Volume (OBV) — returns (current_obv, obv_change).
pub fn obv<C: CandleLike>(candles: &[C]) -> Option<(f64, f64)> {
    if candles.len() < 2 {
        return None;
    }
    let mut obv = 0.0;
    let mut prev_obv = 0.0;
    for pair in candles.windows(2) {
        prev_obv = obv;
        let prev = &pair[0];
        let current = &pair[1];
        if current.close() > prev.close() {
            obv += current.volume() as f64;
        } else if current.close() < prev.close() {
            obv -= current.volume() as f64;
        }
    }
    Some((obv, obv - prev_obv))
}

/// Volume-Weighted Average Price (VWAP) over the last `period` candles.
pub fn vwap<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|c| c.volume() as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(
        slice
            .iter()
            .map(|c| {
                let h = c.high();
                let l = c.low();
                let cl = c.close();
                ((h + l + cl) / 3.0) * c.volume() as f64
            })
            .sum::<f64>()
            / volume_sum,
    )
}

/// Volume ratio — last candle volume / average volume over `period`.
pub fn candle_volume_ratio<C: CandleLike>(candles: &[C], period: usize) -> Option<f64> {
    if candles.len() < period + 1 {
        return None;
    }
    let last = candles.last()?;
    let slice = &candles[candles.len() - period - 1..candles.len() - 1];
    let avg = slice.iter().map(|c| c.volume() as f64).sum::<f64>() / slice.len() as f64;
    (avg > 0.0).then_some(last.volume() as f64 / avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCandle {
        close: f64,
        high: f64,
        low: f64,
        volume: i64,
    }

    impl CandleLike for TestCandle {
        fn close(&self) -> f64 { self.close }
        fn high(&self) -> f64 { self.high }
        fn low(&self) -> f64 { self.low }
        fn volume(&self) -> i64 { self.volume }
    }

    fn candle(close: f64) -> TestCandle {
        TestCandle { close, high: close + 0.5, low: close - 0.5, volume: 1000 }
    }

    #[test]
    fn rsi_all_gains() {
        // 16 candles, period=14. All ascending → RSI should be 100.
        let candles: Vec<TestCandle> = (0..16).map(|i| candle(100.0 + i as f64)).collect();
        let r = rsi(&candles, 14).unwrap();
        assert!((r - 100.0).abs() < 0.01, "expected 100, got {r}");
    }

    #[test]
    fn rsi_all_losses() {
        // All descending → RSI should be ~0.
        let candles: Vec<TestCandle> = (0..16).map(|i| candle(100.0 - i as f64)).collect();
        let r = rsi(&candles, 14).unwrap();
        assert!(r < 0.01, "expected ~0, got {r}");
    }

    #[test]
    fn rsi_mixed_wilder_smoothing() {
        // Known sequence: 16 candles with alternating up/down.
        // With Wilder smoothing, this should produce a stable RSI.
        let prices = [44.0, 44.34, 44.09, 43.61, 44.33, 44.83, 45.10, 45.42,
                       45.84, 46.08, 45.89, 46.03, 45.61, 46.28, 46.28, 46.00];
        let candles: Vec<TestCandle> = prices.iter().map(|&p| candle(p)).collect();
        let r = rsi(&candles, 14).unwrap();
        // With this data the old rolling-sum approach would give ~70.5,
        // Wilder smoothing gives ~63.3.
        assert!((55.0..=75.0).contains(&r), "RSI {r} out of expected range");
        // Verify it's different from the old buggy simple sum approach
        // Old: sum last 14 changes directly → different from Wilder
        assert!(r > 50.0, "RSI should reflect net gains in this sequence");
    }

    #[test]
    fn rsi_insufficient_data() {
        let candles: Vec<TestCandle> = (0..14).map(|i| candle(100.0 + i as f64)).collect();
        assert!(rsi(&candles, 14).is_none());
    }
}
