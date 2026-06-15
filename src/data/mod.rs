use std::fmt;

use crate::types::NewsFetchAttempt;
pub use crate::types::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsItem, QuoteSnapshot,
};

// ---------------------------------------------------------------------------
// Conversion from akshare-rs types to our domain types.
// ---------------------------------------------------------------------------

pub(crate) fn news_item_from_stock_news(n: akshare::stock::feature::StockNews) -> NewsItem {
    NewsItem {
        published_at: n.publish_time,
        title: n.title.clone(),
        summary: n.content.unwrap_or(n.title),
        source: n.source.unwrap_or_else(|| "Eastmoney".to_string()),
        url: n.url,
    }
}

pub(crate) fn news_item_from_news_entry_with_source(
    n: akshare::stock::feature::NewsEntry,
    source: &str,
) -> NewsItem {
    NewsItem {
        published_at: n.time,
        title: n.title.clone(),
        summary: n.summary.unwrap_or(n.title),
        source: source.to_string(),
        url: n.url,
    }
}

pub(crate) fn news_item_from_announcement(a: akshare::AnnouncementItem) -> NewsItem {
    NewsItem {
        published_at: a.published_at,
        title: a.title.clone(),
        summary: a.title,
        source: a.source,
        url: a.url,
    }
}

pub use akshare::stock::feature::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};

mod a_share;
mod client;
pub mod diagnosis;
mod hk;
pub(crate) mod news;
mod us;

#[derive(Clone)]
pub struct MarketDataClient {
    ak: akshare::AkShareClient,
}

#[derive(Debug)]
pub struct DataError {
    message: String,
}

impl DataError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DataError {}

pub(crate) fn news_result_cacheable(items: &[NewsItem], attempts: &[NewsFetchAttempt]) -> bool {
    !items.is_empty() && !attempts.is_empty() && attempts.iter().all(|attempt| attempt.success)
}

pub use akshare::types::BillboardEntry;
