//! Market data primitive types for stock analysis.
//!
//! Re-exports types from akshare-rs so that stock-analyzer crates share
//! a single canonical type definition.

// Re-export all market data types from akshare-rs
pub use akshare::types::{
    AnnouncementDetail, AnnouncementItem, BillboardEntry, BillboardSeatDetail, CandlePoint,
    CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsFetchAttempt, NewsFetchResult,
    NewsItem, QuoteSnapshot, SectorConstituent, SectorSnapshot, StockSearchResult,
    TradeCalendarItem,
};

// Re-export tool types from akshare-rs
pub use akshare::provider::market_client::tools::{PendingToolCall, ToolObservation};
