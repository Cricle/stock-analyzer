//! sa-data — re-exports from akshare-rs for stock-analyzer engine.
//!
//! All data fetching is delegated to akshare-rs. This crate only re-exports
//! the types and client the engine needs.

pub mod cache;
pub mod pipeline;
pub mod validator;

// Re-export MarketDataClient and config types from akshare-rs
pub use akshare::provider::market_client::{GeneralSearchIntent, MarketDataClient};

// Re-export data types from akshare-rs
pub use akshare::types::{
    BillboardEntry, CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsItem,
    QuoteSnapshot,
};

// Re-export stock feature types from akshare-rs
pub use akshare::stock::feature::{
    BillboardStockStatistic, EarningsForecast, FundFlowEntry, HotStockXq, MarginRatioPa, ZtPool,
};

// Re-export DataFetchDiagnosis from akshare-rs
pub use akshare::provider::market_client::DataFetchDiagnosis;

// Re-export news filter utilities
pub use akshare::provider::market_client::normalized_news_date;
