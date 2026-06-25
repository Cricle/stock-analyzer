use crate::data::CandlePoint;

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
    Some(slice.iter().map(|row| row.close).sum::<f64>() / period as f64)
}

pub(super) fn ema_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = sma_candles(&candles[..period], period)?;
    for candle in &candles[period..] {
        let close = candle.close;
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
        let change = pair[1].close - pair[0].close;
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
            let high_low = current.high - current.low;
            let high_close = (current.high - prev.close).abs();
            let low_close = (current.low - prev.close).abs();
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
            .map(|row| row.close * row.volume as f64)
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
    let mut ema = candles[..period].iter().map(|row| row.close).sum::<f64>() / period as f64;
    values.push(ema);
    for candle in &candles[period..] {
        let close = candle.close;
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
            .map(|row| row.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = slice
            .iter()
            .map(|row| row.low)
            .fold(f64::INFINITY, f64::min);
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

pub(super) fn cci_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    if candles.len() < period {
        return None;
    }
    let slice = &candles[candles.len() - period..];
    let typical = slice
        .iter()
        .map(|row| (row.high + row.low + row.close) / 3.0)
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
        .map(|row| row.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let low = slice
        .iter()
        .map(|row| row.low)
        .fold(f64::INFINITY, f64::min);
    let close = slice.last()?.close;
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
            let cur_high = current.high;
            let cur_low = current.low;
            let prev_high = prev.high;
            let prev_low = prev.low;
            let prev_close = prev.close;
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
                let h = row.high;
                let l = row.low;
                let c = row.close;
                ((h + l + c) / 3.0) * row.volume as f64
            })
            .sum::<f64>()
            / volume_sum,
    )
}
