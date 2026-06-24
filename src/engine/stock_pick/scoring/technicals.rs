use crate::data::CandlePoint;
use rust_decimal::prelude::ToPrimitive;

pub(super) fn candle_volume_ratio(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period + 1 {
        return None;
    }
    let last = candles.last()?;
    let slice = &candles[candles.len() - period - 1..candles.len() - 1];
    let avg = slice.iter().map(|row| row.volume as f64).sum::<f64>() / slice.len() as f64;
    (avg > 0.0).then_some(last.volume as f64 / avg)
}

pub(super) fn sma_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    Some(
        slice
            .iter()
            .map(|row| row.close.to_f64().unwrap_or_default())
            .sum::<f64>()
            / period as f64,
    )
}

pub(super) fn ema_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = sma_candles(&candles[..period], period)?;
    for candle in &candles[period..] {
        let close = candle.close.to_f64().unwrap_or_default();
        ema = (close - ema) * multiplier + ema;
    }
    Some(ema)
}

pub(super) fn rsi_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for pair in candles[candles.len() - period - 1..].windows(2) {
        let change =
            pair[1].close.to_f64().unwrap_or_default() - pair[0].close.to_f64().unwrap_or_default();
        if change >= 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    if losses <= f64::EPSILON {
        return Some(100.0);
    }
    let rs = gains / losses;
    Some(100.0 - 100.0 / (1.0 + rs))
}

pub(super) fn atr_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() <= period {
        return None;
    }
    let ranges = candles
        .windows(2)
        .map(|pair| {
            let current = &pair[1];
            let prev = &pair[0];
            let high_low = (current.high - current.low).to_f64().unwrap_or_default();
            let high_close = (current.high - prev.close)
                .abs()
                .to_f64()
                .unwrap_or_default();
            let low_close = (current.low - prev.close)
                .abs()
                .to_f64()
                .unwrap_or_default();
            high_low.max(high_close).max(low_close)
        })
        .collect::<Vec<_>>();
    let slice = &ranges[ranges.len().saturating_sub(period)..];
    Some(slice.iter().sum::<f64>() / slice.len() as f64)
}

pub(super) fn vwma_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|row| row.volume as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(
        slice
            .iter()
            .map(|row| row.close.to_f64().unwrap_or_default() * row.volume as f64)
            .sum::<f64>()
            / volume_sum,
    )
}

pub(super) fn macd_candles(candles: &[CandlePoint]) -> Option<(f64, f64, f64)> {
    if candles.len() < 35 {
        return None;
    }
    let ema12 = ema_series_candles(candles, 12)?;
    let ema26 = ema_series_candles(candles, 26)?;
    let offset = ema12.len().saturating_sub(ema26.len());
    let macd_series = ema12[offset..]
        .iter()
        .zip(ema26.iter())
        .map(|(fast, slow)| fast - slow)
        .collect::<Vec<_>>();
    let signal = ema_values_candles(&macd_series, 9)?;
    let macd = *macd_series.last()?;
    let signal_last = *signal.last()?;
    Some((macd, signal_last, macd - signal_last))
}

fn ema_series_candles(candles: &[CandlePoint], period: usize) -> Option<Vec<f64>> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut values = Vec::new();
    let mut ema = candles[..period]
        .iter()
        .map(|row| row.close.to_f64().unwrap_or_default())
        .sum::<f64>()
        / period as f64;
    values.push(ema);
    for candle in &candles[period..] {
        let close = candle.close.to_f64().unwrap_or_default();
        ema = (close - ema) * multiplier + ema;
        values.push(ema);
    }
    Some(values)
}

fn ema_values_candles(values: &[f64], period: usize) -> Option<Vec<f64>> {
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

pub(super) fn kdj_candles(candles: &[CandlePoint], period: usize) -> Option<(f64, f64, f64)> {
    if candles.len() < period {
        return None;
    }
    let mut k = 50.0;
    let mut d = 50.0;
    for index in period - 1..candles.len() {
        let slice = &candles[index + 1 - period..=index];
        let high = slice
            .iter()
            .map(|row| row.high.to_f64().unwrap_or_default())
            .fold(f64::NEG_INFINITY, f64::max);
        let low = slice
            .iter()
            .map(|row| row.low.to_f64().unwrap_or_default())
            .fold(f64::INFINITY, f64::min);
        let close = candles[index].close.to_f64().unwrap_or_default();
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

pub(super) fn cci_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let typical = slice
        .iter()
        .map(|row| {
            (row.high.to_f64().unwrap_or_default()
                + row.low.to_f64().unwrap_or_default()
                + row.close.to_f64().unwrap_or_default())
                / 3.0
        })
        .collect::<Vec<_>>();
    let ma = typical.iter().sum::<f64>() / period as f64;
    let mean_deviation =
        typical.iter().map(|value| (value - ma).abs()).sum::<f64>() / period as f64;
    if mean_deviation <= f64::EPSILON {
        return None;
    }
    let last = *typical.last()?;
    Some((last - ma) / (0.015 * mean_deviation))
}

pub(super) fn wr_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let high = slice
        .iter()
        .map(|row| row.high.to_f64().unwrap_or_default())
        .fold(f64::NEG_INFINITY, f64::max);
    let low = slice
        .iter()
        .map(|row| row.low.to_f64().unwrap_or_default())
        .fold(f64::INFINITY, f64::min);
    let close = slice.last()?.close.to_f64().unwrap_or_default();
    if high <= low {
        return None;
    }
    Some(((high - close) / (high - low)) * -100.0)
}

pub(super) fn adx_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
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
            let cur_high = current.high.to_f64().unwrap_or_default();
            let cur_low = current.low.to_f64().unwrap_or_default();
            let prev_high = prev.high.to_f64().unwrap_or_default();
            let prev_low = prev.low.to_f64().unwrap_or_default();
            let prev_close = prev.close.to_f64().unwrap_or_default();
            let up_move = cur_high - prev_high;
            let down_move = prev_low - cur_low;
            if up_move > down_move && up_move > 0.0 {
                plus_dm += up_move;
            }
            if down_move > up_move && down_move > 0.0 {
                minus_dm += down_move;
            }
            tr_sum += (cur_high - cur_low)
                .max((cur_high - prev_close).abs())
                .max((cur_low - prev_close).abs());
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

pub(super) fn obv_candles(candles: &[CandlePoint]) -> Option<(f64, f64)> {
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

pub(super) fn vwap_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let volume_sum = slice.iter().map(|row| row.volume as f64).sum::<f64>();
    if volume_sum <= 0.0 {
        return None;
    }
    Some(
        slice
            .iter()
            .map(|row| {
                let h = row.high.to_f64().unwrap_or_default();
                let l = row.low.to_f64().unwrap_or_default();
                let c = row.close.to_f64().unwrap_or_default();
                ((h + l + c) / 3.0) * row.volume as f64
            })
            .sum::<f64>()
            / volume_sum,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_candle(close: f64, high: f64, low: f64, volume: i64) -> CandlePoint {
        CandlePoint {
            trade_date: "2024-01-15".to_string(),
            open: Decimal::from_f64_retain(close).unwrap(),
            close: Decimal::from_f64_retain(close).unwrap(),
            high: Decimal::from_f64_retain(high).unwrap(),
            low: Decimal::from_f64_retain(low).unwrap(),
            volume,
            amount: Decimal::ZERO,
            amplitude_pct: 0.0,
            change_pct: 0.0,
            change_amount: Decimal::ZERO,
            turnover_pct: 0.0,
        }
    }

    fn make_candles(prices: &[(f64, f64, f64, i64)]) -> Vec<CandlePoint> {
        prices
            .iter()
            .map(|(c, h, l, v)| make_candle(*c, *h, *l, *v))
            .collect()
    }

    #[test]
    fn test_candle_volume_ratio_basic() {
        let candles = make_candles(&[
            (100.0, 105.0, 95.0, 1000),
            (101.0, 106.0, 96.0, 1000),
            (102.0, 107.0, 97.0, 1000),
            (103.0, 108.0, 98.0, 2000),
        ]);
        let ratio = candle_volume_ratio(&candles, 3);
        assert!(ratio.is_some());
        let r = ratio.unwrap();
        assert!((r - 2.0).abs() < 0.01); // 2000 / 1000
    }

    #[test]
    fn test_candle_volume_ratio_too_few() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000)]);
        assert!(candle_volume_ratio(&candles, 3).is_none());
    }

    #[test]
    fn test_sma_basic() {
        let candles = make_candles(&[
            (100.0, 105.0, 95.0, 1000),
            (110.0, 115.0, 105.0, 1000),
            (120.0, 125.0, 115.0, 1000),
        ]);
        let sma = sma_candles(&candles, 3);
        assert!(sma.is_some());
        assert!((sma.unwrap() - 110.0).abs() < 0.01);
    }

    #[test]
    fn test_sma_too_few() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000)]);
        assert!(sma_candles(&candles, 3).is_none());
    }

    #[test]
    fn test_ema_basic() {
        let candles = make_candles(&[
            (100.0, 105.0, 95.0, 1000),
            (110.0, 115.0, 105.0, 1000),
            (120.0, 125.0, 115.0, 1000),
            (115.0, 120.0, 110.0, 1000),
            (125.0, 130.0, 120.0, 1000),
        ]);
        let ema = ema_candles(&candles, 3);
        assert!(ema.is_some());
        // EMA should be between min and max
        let val = ema.unwrap();
        assert!(val >= 100.0 && val <= 130.0);
    }

    #[test]
    fn test_ema_too_few() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000)]);
        assert!(ema_candles(&candles, 3).is_none());
    }

    #[test]
    fn test_rsi_all_gains() {
        let candles = make_candles(&[
            (100.0, 105.0, 95.0, 1000),
            (101.0, 106.0, 96.0, 1000),
            (102.0, 107.0, 97.0, 1000),
            (103.0, 108.0, 98.0, 1000),
            (104.0, 109.0, 99.0, 1000),
        ]);
        let rsi = rsi_candles(&candles, 4);
        assert!(rsi.is_some());
        assert!((rsi.unwrap() - 100.0).abs() < 0.01); // All gains → RSI = 100
    }

    #[test]
    fn test_rsi_too_few() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000)]);
        assert!(rsi_candles(&candles, 3).is_none());
    }

    #[test]
    fn test_atr_basic() {
        let candles = make_candles(&[
            (100.0, 110.0, 90.0, 1000),
            (105.0, 115.0, 95.0, 1000),
            (110.0, 120.0, 100.0, 1000),
            (108.0, 118.0, 98.0, 1000),
        ]);
        let atr = atr_candles(&candles, 3);
        assert!(atr.is_some());
        assert!(atr.unwrap() > 0.0);
    }

    #[test]
    fn test_atr_too_few() {
        let candles = make_candles(&[(100.0, 110.0, 90.0, 1000)]);
        assert!(atr_candles(&candles, 3).is_none());
    }

    #[test]
    fn test_vwma_basic() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000), (110.0, 115.0, 105.0, 2000)]);
        let vwma = vwma_candles(&candles, 2);
        assert!(vwma.is_some());
        // VWMA = (100*1000 + 110*2000) / (1000+2000) = 320000/3000 ≈ 106.67
        assert!((vwma.unwrap() - 106.67).abs() < 0.1);
    }

    #[test]
    fn test_vwma_zero_volume() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 0), (110.0, 115.0, 105.0, 0)]);
        assert!(vwma_candles(&candles, 2).is_none());
    }

    #[test]
    fn test_kdj_basic() {
        let candles = make_candles(&[
            (100.0, 110.0, 90.0, 1000),
            (105.0, 115.0, 95.0, 1000),
            (110.0, 120.0, 100.0, 1000),
            (108.0, 118.0, 98.0, 1000),
            (112.0, 122.0, 102.0, 1000),
        ]);
        let kdj = kdj_candles(&candles, 3);
        assert!(kdj.is_some());
        let (k, d, j) = kdj.unwrap();
        assert!(k >= 0.0 && k <= 100.0);
        assert!(d >= 0.0 && d <= 100.0);
        // J = 3K - 2D
        assert!((j - (3.0 * k - 2.0 * d)).abs() < 0.01);
    }

    #[test]
    fn test_cci_basic() {
        let candles = make_candles(&[
            (100.0, 110.0, 90.0, 1000),
            (105.0, 115.0, 95.0, 1000),
            (110.0, 120.0, 100.0, 1000),
        ]);
        let cci = cci_candles(&candles, 3);
        assert!(cci.is_some());
    }

    #[test]
    fn test_wr_basic() {
        let candles = make_candles(&[
            (100.0, 110.0, 90.0, 1000),
            (105.0, 115.0, 95.0, 1000),
            (110.0, 120.0, 100.0, 1000),
        ]);
        let wr = wr_candles(&candles, 3);
        assert!(wr.is_some());
        let val = wr.unwrap();
        // Williams %R is between -100 and 0
        assert!(val >= -100.0 && val <= 0.0);
    }

    #[test]
    fn test_obv_basic() {
        let candles = make_candles(&[
            (100.0, 105.0, 95.0, 1000),
            (110.0, 115.0, 105.0, 2000),
            (105.0, 110.0, 100.0, 1500),
        ]);
        let obv = obv_candles(&candles);
        assert!(obv.is_some());
        let (obv_val, delta) = obv.unwrap();
        // Day 2: close up → +2000, Day 3: close down → -1500
        assert!((obv_val - 500.0).abs() < 0.01);
        // Delta = 500 - 2000 = -1500
        assert!((delta - (-1500.0)).abs() < 0.01);
    }

    #[test]
    fn test_obv_too_few() {
        let candles = make_candles(&[(100.0, 105.0, 95.0, 1000)]);
        assert!(obv_candles(&candles).is_none());
    }

    #[test]
    fn test_vwap_basic() {
        let candles = make_candles(&[(100.0, 110.0, 90.0, 1000), (110.0, 120.0, 100.0, 2000)]);
        let vwap = vwap_candles(&candles, 2);
        assert!(vwap.is_some());
        assert!(vwap.unwrap() > 0.0);
    }

    #[test]
    fn test_adx_basic() {
        let candles = make_candles(&[
            (100.0, 110.0, 90.0, 1000),
            (105.0, 115.0, 95.0, 1000),
            (110.0, 120.0, 100.0, 1000),
            (108.0, 118.0, 98.0, 1000),
            (112.0, 122.0, 102.0, 1000),
        ]);
        let adx = adx_candles(&candles, 3);
        assert!(adx.is_some());
        assert!(adx.unwrap() >= 0.0);
    }
}
