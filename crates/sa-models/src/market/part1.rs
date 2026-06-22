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
