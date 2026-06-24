use super::TradingToolbox;
use crate::types::CandlePoint;
use rust_decimal::prelude::ToPrimitive;

/// Adapter bridging stock-analyzer's `CandlePoint` (Decimal fields) to
/// akshare's `Ohlcv` trait (f64 interface).
struct OhlcvAdapter<'a>(&'a CandlePoint);

impl akshare::types::Ohlcv for OhlcvAdapter<'_> {
    fn trade_date(&self) -> &str {
        &self.0.trade_date
    }
    fn open(&self) -> f64 {
        self.0.open.to_f64().unwrap_or_default()
    }
    fn close(&self) -> f64 {
        self.0.close.to_f64().unwrap_or_default()
    }
    fn high(&self) -> f64 {
        self.0.high.to_f64().unwrap_or_default()
    }
    fn low(&self) -> f64 {
        self.0.low.to_f64().unwrap_or_default()
    }
    fn volume(&self) -> f64 {
        self.0.volume as f64
    }
}

impl TradingToolbox {
    pub(super) fn compute_indicator(name: &str, candles: &[CandlePoint]) -> Option<f64> {
        let adapted: Vec<_> = candles.iter().map(OhlcvAdapter).collect();
        akshare::ta::compute_indicator(name, &adapted)
    }
}


