use crate::data::CandlePoint;

pub(super) fn candle_volume_ratio(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::candle_volume_ratio(candles, period)
}

pub(super) fn sma_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::sma(candles, period)
}

pub(super) fn ema_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::ema(candles, period)
}

pub(super) fn rsi_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::rsi(candles, period)
}

pub(super) fn atr_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::atr(candles, period)
}

pub(super) fn vwma_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::vwma(candles, period)
}

pub(super) fn macd_candles(candles: &[CandlePoint]) -> Option<(f64, f64, f64)> {
    crate::indicators::macd(candles)
}

pub(super) fn kdj_candles(candles: &[CandlePoint], period: usize) -> Option<(f64, f64, f64)> {
    crate::indicators::kdj(candles, period)
}

pub(super) fn cci_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::cci(candles, period)
}

pub(super) fn wr_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::wr(candles, period)
}

pub(super) fn adx_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::adx(candles, period)
}

pub(super) fn obv_candles(candles: &[CandlePoint]) -> Option<(f64, f64)> {
    crate::indicators::obv(candles)
}

pub(super) fn vwap_candles(candles: &[CandlePoint], period: usize) -> Option<f64> {
    crate::indicators::vwap(candles, period)
}
