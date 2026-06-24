
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardSeatItem {
    pub trade_date: String,
    pub symbol: String,
    pub department_name: String,
    pub buy_amount: Option<f64>,
    pub sell_amount: Option<f64>,
    pub net_amount: Option<f64>,
    pub explanation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardSeatsResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    pub side: String,
    pub items: Vec<BillboardSeatItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod market_tests {
    use super::super::*;

    #[test]
    fn stock_search_item_serde_roundtrip() {
        let item = StockSearchItem {
            symbol: "AAPL".to_string(),
            name: "Apple".to_string(),
            market: "美股".to_string(),
            exchange: "NASDAQ".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let restored: StockSearchItem = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.symbol, "AAPL");
        assert_eq!(restored.name, "Apple");
    }

    #[test]
    fn quote_response_serde_roundtrip() {
        let resp = QuoteResponse {
            symbol: "AAPL".to_string(),
            market: "美股".to_string(),
            source: "eastmoney".to_string(),
            provider_used: Some("eastmoney".to_string()),
            status: "ok".to_string(),
            trade_date: Some("2025-01-15".to_string()),
            price: Some(150.0),
            open: Some(149.0),
            high: Some(151.0),
            low: Some(148.0),
            volume: Some(1000000),
            error_kind: None,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: QuoteResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.symbol, "AAPL");
        assert_eq!(restored.price, Some(150.0));
    }

    #[test]
    fn candle_item_serde_roundtrip() {
        let item = CandleItem {
            trade_date: "2025-01-15".to_string(),
            open: 149.0,
            close: 150.0,
            high: 151.0,
            low: 148.0,
            volume: 1000000,
            amount: 150000000.0,
            amplitude_pct: 2.0,
            change_pct: 0.67,
            change_amount: 1.0,
            turnover_pct: 0.5,
        };
        let json = serde_json::to_string(&item).unwrap();
        let restored: CandleItem = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.trade_date, "2025-01-15");
        assert!((restored.close - 150.0).abs() < 0.001);
    }

    #[test]
    fn billboard_seat_item_serde_roundtrip() {
        let item = BillboardSeatItem {
            trade_date: "2025-01-15".to_string(),
            symbol: "AAPL".to_string(),
            department_name: "Trading Desk".to_string(),
            buy_amount: Some(1000000.0),
            sell_amount: Some(500000.0),
            net_amount: Some(500000.0),
            explanation: Some("Strong buying".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let restored: BillboardSeatItem = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.department_name, "Trading Desk");
    }

    #[test]
    fn billboard_seats_response_serde_roundtrip() {
        let resp = BillboardSeatsResponse {
            symbol: "AAPL".to_string(),
            market: "美股".to_string(),
            source: "eastmoney".to_string(),
            status: "ok".to_string(),
            side: "buy".to_string(),
            items: vec![],
            error_kind: None,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: BillboardSeatsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.side, "buy");
        assert!(restored.items.is_empty());
    }

    #[test]
    fn news_response_serde_roundtrip() {
        let resp = NewsResponse {
            symbol: "AAPL".to_string(),
            market: "美股".to_string(),
            source: "searxng".to_string(),
            status: "ok".to_string(),
            items: vec![NewsItemResponse {
                title: "Apple Earnings".to_string(),
                url: Some("http://test.com".to_string()),
                source: "Reuters".to_string(),
                published_at: "2025-01-15".to_string(),
                summary: "Record revenue".to_string(),
            }],
            upstream_source: None,
            upstream_sources: None,
            error_kind: None,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: NewsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.items.len(), 1);
        assert_eq!(restored.items[0].title, "Apple Earnings");
    }
}
