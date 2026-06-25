use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::traits::MarketDataProvider;
use super::{CandlePoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot};

pub struct MockMarketProvider {
    pub news: Vec<NewsItem>,
    pub global_news: Vec<NewsItem>,
    pub candles: Vec<CandlePoint>,
    pub quote: Option<QuoteSnapshot>,
    pub fundamentals: Option<FundamentalsSnapshot>,
    pub balance_sheet: Value,
    pub cashflow: Value,
    pub income_statement: Value,
    pub indicators: Value,
    pub insider_transactions: Value,
}

impl Default for MockMarketProvider {
    fn default() -> Self {
        Self {
            news: Vec::new(),
            global_news: Vec::new(),
            candles: Vec::new(),
            quote: None,
            fundamentals: None,
            balance_sheet: json!(null),
            cashflow: json!(null),
            income_statement: json!(null),
            indicators: json!(null),
            insider_transactions: json!(null),
        }
    }
}

#[async_trait]
impl MarketDataProvider for MockMarketProvider {
    async fn fetch_news(&self, _symbol: &str, limit: usize) -> Result<Vec<NewsItem>> {
        Ok(self.news.iter().take(limit).cloned().collect())
    }

    async fn fetch_global_news(&self, _market: &str, limit: usize) -> Result<Vec<NewsItem>> {
        Ok(self.global_news.iter().take(limit).cloned().collect())
    }

    async fn fetch_candles(&self, _symbol: &str, _days: usize) -> Result<Vec<CandlePoint>> {
        Ok(self.candles.clone())
    }

    async fn fetch_quote(&self, _symbol: &str) -> Result<QuoteSnapshot> {
        self.quote
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no quote data"))
    }

    async fn fetch_fundamentals(&self, _symbol: &str) -> Result<FundamentalsSnapshot> {
        self.fundamentals
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no fundamentals data"))
    }

    async fn fetch_balance_sheet(&self, _symbol: &str) -> Result<Value> {
        Ok(self.balance_sheet.clone())
    }

    async fn fetch_cashflow(&self, _symbol: &str) -> Result<Value> {
        Ok(self.cashflow.clone())
    }

    async fn fetch_income_statement(&self, _symbol: &str) -> Result<Value> {
        Ok(self.income_statement.clone())
    }

    async fn compute_indicators(&self, _candles: &[CandlePoint], _params: &Value) -> Result<Value> {
        Ok(self.indicators.clone())
    }

    async fn fetch_insider_transactions(&self, _symbol: &str) -> Result<Value> {
        Ok(self.insider_transactions.clone())
    }
}
