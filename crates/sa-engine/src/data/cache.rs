use super::MarketDataClient;

impl MarketDataClient {
    pub(super) fn normalize_a_share_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_a_share_symbol(symbol)
    }

    pub(super) fn normalize_hk_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_hk_symbol(symbol).map(|code| format!("{code}.HK"))
    }
}
