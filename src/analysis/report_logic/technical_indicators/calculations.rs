
pub fn obv_signal(delta: Option<f64>) -> &'static str {
    match delta {
        Some(value) if value > 0.0 => "volume_accumulation",
        Some(value) if value < 0.0 => "volume_distribution",
        Some(_) => "volume_neutral",
        None => "unavailable",
    }
}

pub fn sma_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::sma(candles, period)
}

pub fn ema_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::ema(candles, period)
}

pub fn rsi_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::rsi(candles, period)
}

pub fn atr_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::atr(candles, period)
}

fn vwma_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::vwma(candles, period)
}

pub fn bollinger_report(candles: &[ReportCandle], period: usize) -> Option<(f64, f64, f64)> {
    crate::indicators::bollinger(candles, period)
}

pub fn macd_report(candles: &[ReportCandle]) -> Option<(f64, f64, f64)> {
    crate::indicators::macd(candles)
}

pub fn kdj_report(candles: &[ReportCandle], period: usize) -> Option<(f64, f64, f64)> {
    crate::indicators::kdj(candles, period)
}

fn cci_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::cci(candles, period)
}

fn wr_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::wr(candles, period)
}

pub fn adx_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::adx(candles, period)
}

pub fn obv_report(candles: &[ReportCandle]) -> Option<(f64, f64)> {
    crate::indicators::obv(candles)
}

fn vwap_report(candles: &[ReportCandle], period: usize) -> Option<f64> {
    crate::indicators::vwap(candles, period)
}
