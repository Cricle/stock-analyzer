use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockSearchItem {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockSearchResponse {
    pub source: String,
    pub status: String,
    pub query: String,
    pub items: Vec<StockSearchItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandleItem {
    pub trade_date: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: i64,
    pub amount: f64,
    pub amplitude_pct: f64,
    pub change_pct: f64,
    pub change_amount: f64,
    pub turnover_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_used: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundamentalsResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cik: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub industry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fiscal_year_end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares_outstanding: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diluted_shares_outstanding: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revenues: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revenues_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_income: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_income_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liabilities: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liabilities_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stockholders_equity_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cash_and_equivalents_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gross_profit_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_income_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_expenses_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_cash_flow_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capital_expenditure_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_cash_flow_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_term_debt_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_debt_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_debt_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewsItemResponse {
    pub published_at: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewsResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    pub items: Vec<NewsItemResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandlesResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_used: Option<String>,
    pub status: String,
    pub adjust: String,
    pub items: Vec<CandleItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalFlowItem {
    pub trade_date: String,
    pub main_net_inflow: f64,
    pub small_net_inflow: f64,
    pub medium_net_inflow: f64,
    pub large_net_inflow: f64,
    pub super_large_net_inflow: f64,
    pub main_net_inflow_ratio_pct: f64,
    pub small_net_inflow_ratio_pct: f64,
    pub medium_net_inflow_ratio_pct: f64,
    pub large_net_inflow_ratio_pct: f64,
    pub super_large_net_inflow_ratio_pct: f64,
    pub close: f64,
    pub change_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalFlowResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    pub items: Vec<CapitalFlowItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorCapitalFlowResponse {
    pub market: String,
    pub source: String,
    pub status: String,
    pub sector_code: String,
    pub items: Vec<CapitalFlowItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorRankingItem {
    pub sector_code: String,
    pub sector_name: String,
    pub latest_index: f64,
    pub change_pct: f64,
    pub main_net_inflow: f64,
    pub main_net_inflow_ratio_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorRankingsResponse {
    pub market: String,
    pub source: String,
    pub status: String,
    pub sector_type: String,
    pub items: Vec<SectorRankingItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorConstituentItem {
    pub symbol: String,
    pub name: String,
    pub latest_price: f64,
    pub change_pct: f64,
    pub main_net_inflow: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorConstituentsResponse {
    pub market: String,
    pub source: String,
    pub status: String,
    pub sector_code: String,
    pub items: Vec<SectorConstituentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnouncementDetailResponse {
    pub source: String,
    pub status: String,
    pub art_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardEntryItem {
    pub trade_date: String,
    pub symbol: String,
    pub name: String,
    pub close_price: f64,
    pub change_rate_pct: f64,
    pub turnover_rate_pct: Option<f64>,
    pub net_amount: Option<f64>,
    pub buy_amount: Option<f64>,
    pub sell_amount: Option<f64>,
    pub explanation: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    pub items: Vec<BillboardEntryItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_search_item_roundtrip() {
        let item = StockSearchItem {
            symbol: "AAPL".to_string(),
            name: "Apple Inc.".to_string(),
            market: "us_equity".to_string(),
            exchange: "NASDAQ".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: StockSearchItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.symbol, "AAPL");
        assert_eq!(parsed.name, "Apple Inc.");
    }

    #[test]
    fn stock_search_response_deserialize() {
        let json = r#"{"source":"test","status":"ok","query":"AAPL","items":[]}"#;
        let resp: StockSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.source, "test");
        assert!(resp.items.is_empty());
        assert!(resp.error_kind.is_none());
    }

    #[test]
    fn candle_item_roundtrip() {
        let item = CandleItem {
            trade_date: "2026-06-20".to_string(),
            open: 100.0,
            close: 105.0,
            high: 106.0,
            low: 99.0,
            volume: 1000000,
            amount: 105000000.0,
            amplitude_pct: 7.0,
            change_pct: 5.0,
            change_amount: 5.0,
            turnover_pct: 1.5,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: CandleItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trade_date, "2026-06-20");
        assert_eq!(parsed.close, 105.0);
    }

    #[test]
    fn quote_response_deserialize() {
        let json = r#"{"symbol":"AAPL","market":"us_equity","source":"test","status":"ok","price":150.5}"#;
        let resp: QuoteResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.symbol, "AAPL");
        assert_eq!(resp.price, Some(150.5));
    }

    #[test]
    fn news_item_roundtrip() {
        let item = NewsItemResponse {
            published_at: "2026-06-20".to_string(),
            title: "Apple beats earnings".to_string(),
            summary: "Strong Q2 results".to_string(),
            source: "Reuters".to_string(),
            url: Some("https://example.com".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: NewsItemResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Apple beats earnings");
        assert_eq!(parsed.url, Some("https://example.com".to_string()));
    }

    #[test]
    fn news_response_deserialize() {
        let json = r#"{"symbol":"AAPL","market":"us_equity","source":"test","status":"ok","items":[]}"#;
        let resp: NewsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.symbol, "AAPL");
        assert!(resp.items.is_empty());
    }

    #[test]
    fn candles_response_deserialize() {
        let json = r#"{"symbol":"AAPL","market":"us_equity","source":"test","status":"ok","adjust":"qfq","items":[]}"#;
        let resp: CandlesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.adjust, "qfq");
    }

    #[test]
    fn fundamentals_response_deserialize() {
        let json = r#"{"symbol":"AAPL","market":"us_equity","source":"test","status":"ok","revenues":50000000000.0}"#;
        let resp: FundamentalsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.revenues, Some(50000000000.0));
    }

    #[test]
    fn capital_flow_item_roundtrip() {
        let item = CapitalFlowItem {
            trade_date: "2026-06-20".to_string(),
            main_net_inflow: 1000000.0,
            small_net_inflow: -500000.0,
            medium_net_inflow: 200000.0,
            large_net_inflow: 300000.0,
            super_large_net_inflow: 1000000.0,
            main_net_inflow_ratio_pct: 5.0,
            small_net_inflow_ratio_pct: -2.5,
            medium_net_inflow_ratio_pct: 1.0,
            large_net_inflow_ratio_pct: 1.5,
            super_large_net_inflow_ratio_pct: 5.0,
            close: 105.0,
            change_pct: 2.0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: CapitalFlowItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.close, 105.0);
    }

    #[test]
    fn sector_ranking_item_roundtrip() {
        let item = SectorRankingItem {
            sector_code: "tech".to_string(),
            sector_name: "Technology".to_string(),
            latest_index: 1500.0,
            change_pct: 1.5,
            main_net_inflow: 5000000.0,
            main_net_inflow_ratio_pct: 3.0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: SectorRankingItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sector_name, "Technology");
    }

    #[test]
    fn billboard_entry_roundtrip() {
        let item = BillboardEntryItem {
            trade_date: "2026-06-20".to_string(),
            symbol: "000001".to_string(),
            name: "平安银行".to_string(),
            close_price: 12.5,
            change_rate_pct: 5.0,
            turnover_rate_pct: Some(2.5),
            net_amount: Some(1000000.0),
            buy_amount: Some(5000000.0),
            sell_amount: Some(4000000.0),
            explanation: Some("涨停".to_string()),
            reason: Some("业绩预增".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: BillboardEntryItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "平安银行");
    }
}
