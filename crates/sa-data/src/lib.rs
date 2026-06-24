//! sa-data — re-exports from akshare-rs for stock-analyzer engine.
//!
//! All data fetching is delegated to akshare-rs. This crate only re-exports
//! the types and client the engine needs.

// Re-export MarketDataClient and config types from akshare-rs
pub use akshare::provider::market_client::{
    DataConfig, DataError, DataErrorKind, GeneralSearchIntent, MarketDataClient,
    SearchProviderConfig,
};

// Re-export data types from akshare-rs
pub use akshare::types::{
    AnnouncementDetail, AnnouncementItem, BillboardEntry, BillboardSeatDetail, CandlePoint,
    CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsFetchAttempt, NewsFetchResult,
    NewsItem, QuoteSnapshot, SectorConstituent, SectorSnapshot, StockSearchResult,
    TradeCalendarItem,
};

// Re-export stock feature types from akshare-rs
pub use akshare::stock::feature::{
    AnalystDetail, AnalystRank, BalanceSheet, CashFlowSheet, CommentDesireIndex, CommentFocusIndex,
    CommentHistScore, CommentOrgParticipation, DividendInfo, DzjyHygtj, DzjyHyyybtj, DzjyMrtj,
    DzjyYybph, EarningsForecast, EarningsQuickReport, EarningsReport, EsgRating, FundFlowEntry,
    GdfxHoldingAnalyse, GdfxHoldingChange, GdfxHoldingDetail, GdfxHoldingStatistic, GdfxTeamwork,
    GdfxTop10, Gdhs, GdhsDetail, Ggcg, GpzyDistributeEntry, GpzyIndustry, GpzyPledgeDetail,
    GpzyPledgeRatio, GpzyPledgeRatioDetail, GpzyProfile, HotStockXq, IndustryCategory, JgdyDetail,
    JgdyTj, LhbDetail, LhbHyyyb, LhbJgmmtj, LhbJgstatistic, LhbStockDetail, LhbStockDetailDate,
    LhbStockStatistic, LhbTraderStatistic, LhbYybDetail, LhbYybph, MainFundFlow, MarginAccountInfo,
    MarginRatioPa, MarginSseDetail, MarginSseSummary, MarginSzseDetail, MarginSzseSummary,
    PankouChange, ProfitSheet, SectorFundFlowRank, StockComment, ZtPool, ZtPoolDtgc,
    ZtPoolPrevious, ZtPoolStrong, ZtPoolSubNew, ZtPoolZbgc,
};

// Re-export HK types
pub use akshare::stock::hk_extra::{
    HkFamousStock, HkFhpxDetailThs, HkGxlLg, HkHotRank, HkHotRankDetail, HkSpotQuote,
    HkValuationBaidu,
};

// Re-export US types
pub use akshare::stock::us_extra::{UsFamousStock, UsPinkStock, UsSpotSina, UsValuationBaidu};

// Re-export Xueqiu types
pub use akshare::stock::xueqiu::XqStockSpot;

// Re-export DataFetchDiagnosis from akshare-rs
pub use akshare::provider::market_client::DataFetchDiagnosis;

// Re-export news filter utilities
pub use akshare::provider::market_client::normalized_news_date;
