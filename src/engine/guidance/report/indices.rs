//! Market index fetching for guidance reports.
//!
//! Uses ETF proxies for indices that the quote provider cannot resolve directly
//! (e.g. HSI → 2800.HK, S&P 500 → SPY). Display names remain as index names.

use rust_decimal::prelude::ToPrimitive;

use super::*;

/// (fetch_symbol, display_name, market)
fn index_definitions(market: &GuidanceMarket) -> Vec<(&'static str, &'static str, &'static str)> {
    match market {
        GuidanceMarket::AShare => vec![
            ("000001", "SSE Composite", "a_share"),
            ("399001", "SZSE Component", "a_share"),
            ("399006", "ChiNext", "a_share"),
        ],
        GuidanceMarket::HongKong => vec![
            ("2800.HK", "Hang Seng Index", "hong_kong"),
            ("2823.HK", "Hang Seng China Enterprises", "hong_kong"),
        ],
        GuidanceMarket::UsEquity => vec![
            ("SPY", "S&P 500", "us_equity"),
            ("QQQ", "NASDAQ 100", "us_equity"),
            ("DIA", "Dow Jones", "us_equity"),
        ],
        GuidanceMarket::All => vec![
            ("000001", "SSE Composite", "a_share"),
            ("2800.HK", "Hang Seng Index", "hong_kong"),
            ("SPY", "S&P 500", "us_equity"),
        ],
    }
}

impl DailyGuidanceGenerator {
    pub(super) async fn fetch_market_indices(&self, market: &GuidanceMarket) -> Vec<MarketIndex> {
        let index_defs = index_definitions(market);

        let symbols: Vec<&str> = index_defs.iter().map(|(s, _, _)| *s).collect();
        let quotes = self.market_data.fetch_quotes_batch(&symbols).await;
        let quote_map: std::collections::HashMap<&str, &crate::data::QuoteSnapshot> = quotes
            .iter()
            .filter_map(|(s, q)| q.as_ref().map(|q| (s.as_str(), q)))
            .collect();

        index_defs
            .iter()
            .filter_map(|(symbol, name, mkt)| {
                let quote = quote_map.get(symbol)?;
                let open = quote.open.to_f64().unwrap_or_default();
                let close = quote.close.to_f64().unwrap_or_default();
                let change_pct = if open > 0.0 {
                    ((close - open) / open) * 100.0
                } else {
                    0.0
                };
                Some(MarketIndex {
                    symbol: symbol.to_string(),
                    name: name.to_string(),
                    price: close,
                    change_pct,
                    market: mkt.to_string(),
                })
            })
            .collect()
    }
}
