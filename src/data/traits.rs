use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::{CandlePoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot};

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    // News
    async fn fetch_news(&self, symbol: &str, limit: usize) -> Result<Vec<NewsItem>>;
    async fn fetch_global_news(&self, market: &str, limit: usize) -> Result<Vec<NewsItem>>;

    // Market data
    async fn fetch_candles(&self, symbol: &str, days: usize) -> Result<Vec<CandlePoint>>;
    async fn fetch_quote(&self, symbol: &str) -> Result<QuoteSnapshot>;
    async fn fetch_fundamentals(&self, symbol: &str) -> Result<FundamentalsSnapshot>;

    // Financial statements
    async fn fetch_balance_sheet(&self, symbol: &str) -> Result<Value>;
    async fn fetch_cashflow(&self, symbol: &str) -> Result<Value>;
    async fn fetch_income_statement(&self, symbol: &str) -> Result<Value>;

    // Indicators
    async fn compute_indicators(&self, candles: &[CandlePoint], params: &Value) -> Result<Value>;

    // Insider transactions
    async fn fetch_insider_transactions(&self, symbol: &str) -> Result<Value>;
}
