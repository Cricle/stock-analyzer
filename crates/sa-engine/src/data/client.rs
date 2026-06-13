use anyhow::Context;
use tracing::Instrument;

use super::{
    BillboardEntry, CandlePoint, CapitalFlowPoint, DataConfig, DataError, DataErrorKind,
    FundamentalsSnapshot, MarketDataClient, MarketKind, NewsItem, QuoteSnapshot,
};
use crate::types::NewsFetchResult;
use akshare::types::{
    AnnouncementDetail, AnnouncementItem, BillboardSeatDetail, SectorConstituent,
    SectorSnapshot, StockSearchResult, TradeCalendarItem,
};
impl MarketDataClient {
    pub async fn new() -> Self {
        Self::from_config(&DataConfig {}).await
    }

    pub async fn from_config(_config: &DataConfig) -> Self {
        let mut ak_builder = akshare::AkShareClient::builder();
        let outbound_proxy_url = std::env::var("OUTBOUND_PROXY_URL")
            .ok()
            .or_else(|| std::env::var("HTTP_PROXY").ok())
            .or_else(|| std::env::var("http_proxy").ok())
            .or_else(|| std::env::var("HTTPS_PROXY").ok())
            .or_else(|| std::env::var("https_proxy").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(proxy_url) = outbound_proxy_url.as_deref() {
            ak_builder = ak_builder.proxy(proxy_url);
        }
        let ak = ak_builder.build();

        Self { ak }
    }

    pub fn detect_market(&self, symbol: &str) -> MarketKind {
        akshare::detect_market(symbol).into()
    }

    pub fn quote_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) { MarketKind::AShare => "akshare:tencent_quote+eastmoney", MarketKind::HongKong => "akshare:tencent_quote+yahoo_finance_chart", MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq" }
    }

    pub fn fundamentals_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) { MarketKind::AShare => "akshare:eastmoney", MarketKind::HongKong => "akshare:tencent_quote+eastmoney_search", MarketKind::UsEquity => "akshare:sec_edgar" }
    }

    pub fn news_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) { MarketKind::AShare => "akshare:eastmoney+searxng_news", MarketKind::HongKong => "akshare:searxng_news", MarketKind::UsEquity => "akshare:sec_edgar+searxng_news" }
    }

    pub fn candles_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) { MarketKind::AShare => "akshare:tencent_kline+eastmoney_kline", MarketKind::HongKong => "akshare:tencent_kline+yahoo_finance_chart", MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq" }
    }

    pub fn capital_flow_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) {
            MarketKind::AShare => "akshare_compatible:eastmoney",
            MarketKind::HongKong => "unsupported",
            MarketKind::UsEquity => "unsupported",
        }
    }

    pub fn error_kind(&self, error: &anyhow::Error) -> &'static str {
        for cause in error.chain() {
            if let Some(data_error) = cause.downcast_ref::<DataError>() {
                return data_error.kind.as_str();
            }
            if let Some(reqwest_error) = cause.downcast_ref::<reqwest::Error>()
                && let Some(status) = reqwest_error.status()
            {
                return match status {
                    reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::UNAUTHORIZED => {
                        DataErrorKind::Restricted.as_str()
                    }
                    reqwest::StatusCode::NOT_FOUND => DataErrorKind::NotFound.as_str(),
                    _ => DataErrorKind::Upstream.as_str(),
                };
            }
        }
        DataErrorKind::Upstream.as_str()
    }

    pub async fn fetch_quote(&self, symbol: &str) -> anyhow::Result<QuoteSnapshot> {
        self.fetch_quote_with_provider(symbol)
            .await
            .map(|(quote, _)| quote)
    }

    pub async fn fetch_quote_with_provider(
        &self,
        symbol: &str,
    ) -> anyhow::Result<(QuoteSnapshot, String)> {
        let span = tracing::info_span!("market_data.fetch", data_type = "quote", symbol);
        async {
            match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_quote_from_eastmoney(symbol).await.map(|q| (q, "akshare".to_string())),
                MarketKind::HongKong => self.fetch_hk_quote(symbol).await,
                MarketKind::UsEquity => self.fetch_us_quote(symbol).await,
            }
        }.instrument(span).await
    }

    pub async fn fetch_fundamentals(&self, symbol: &str) -> anyhow::Result<FundamentalsSnapshot> {
        let span = tracing::info_span!("market_data.fetch", data_type = "fundamentals", symbol);
        async {
            match self.detect_market(symbol) {
                MarketKind::AShare => {
                    let ts_code = self.normalize_a_share_symbol(symbol)
                        .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
                    self.fetch_a_share_fundamentals(symbol, &ts_code).await
                }
                MarketKind::HongKong => self.fetch_hk_fundamentals(symbol).await,
                MarketKind::UsEquity => self.fetch_us_fundamentals(symbol).await,
            }
        }.instrument(span).await
    }

    /// Batch fetch quotes for multiple symbols. Returns (symbol, `Option<QuoteSnapshot>`).
    pub async fn fetch_quotes_batch(
        &self,
        symbols: &[&str],
    ) -> Vec<(String, Option<QuoteSnapshot>)> {
        if symbols.is_empty() {
            return Vec::new();
        }
        let futs: Vec<_> = symbols
            .iter()
            .map(|&sym| {
                let sym_owned = sym.to_string();
                let client = self.clone();
                async move {
                    let result = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        async {
                            match client.detect_market(&sym_owned) {
                                MarketKind::AShare => client.fetch_a_share_quote_from_eastmoney(&sym_owned).await.map(|q| (q, "akshare".to_string())),
                                MarketKind::HongKong => client.fetch_hk_quote(&sym_owned).await,
                                MarketKind::UsEquity => client.fetch_us_quote(&sym_owned).await,
                            }
                        },
                    )
                    .await;
                    let quote = match result {
                        Ok(Ok((q, _))) => Some(q),
                        _ => None,
                    };
                    (sym_owned, quote)
                }
            })
            .collect();
        futures::future::join_all(futs).await
    }

    /// Batch fetch fundamentals for multiple symbols.
    pub async fn fetch_fundamentals_batch(
        &self,
        symbols: &[&str],
    ) -> Vec<(String, Option<FundamentalsSnapshot>)> {
        if symbols.is_empty() {
            return Vec::new();
        }
        let futs: Vec<_> = symbols
            .iter()
            .map(|&sym| {
                let sym_owned = sym.to_string();
                let client = self.clone();
                async move {
                    let result = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        async {
                            match client.detect_market(&sym_owned) {
                                MarketKind::AShare => {
                                    let ts_code = client.normalize_a_share_symbol(&sym_owned)
                                        .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
                                    client.fetch_a_share_fundamentals(&sym_owned, &ts_code).await
                                }
                                MarketKind::HongKong => client.fetch_hk_fundamentals(&sym_owned).await,
                                MarketKind::UsEquity => client.fetch_us_fundamentals(&sym_owned).await,
                            }
                        },
                    )
                    .await;
                    let fund = match result {
                        Ok(Ok(f)) => Some(f),
                        _ => None,
                    };
                    (sym_owned, fund)
                }
            })
            .collect();
        futures::future::join_all(futs).await
    }

    pub async fn fetch_news(
        &self,
        symbol: &str,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "news", symbol);
        async {
            let result = self
                .fetch_news_with_diagnostics(symbol, limit, start_date, end_date)
                .await;
            Ok(result?.items)
        }.instrument(span).await
    }

    pub async fn fetch_news_with_diagnostics(
        &self,
        symbol: &str,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> anyhow::Result<NewsFetchResult> {
        self.fetch_news_with_diagnostics_query(symbol, limit, start_date, end_date, None)
            .await
    }

    pub async fn fetch_news_with_diagnostics_query(
        &self,
        symbol: &str,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
        query: Option<&str>,
    ) -> anyhow::Result<NewsFetchResult> {
        let span = tracing::info_span!("market_data.fetch", data_type = "news_detailed", symbol);
        async {
            let market = self.detect_market(symbol);
            self.fetch_market_news_diagnostics_query(symbol, market, limit, start_date, end_date, query)
                .await
        }.instrument(span).await
    }

    pub async fn fetch_global_news(
        &self,
        market_hint_symbol: &str,
        curr_date: &str,
        look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "global_news", symbol = market_hint_symbol);
        async {
            let result = self
                .fetch_global_news_with_diagnostics(
                    market_hint_symbol,
                    curr_date,
                    look_back_days,
                    limit,
                )
                .await;
            Ok(result?.items)
        }.instrument(span).await
    }

    pub async fn fetch_global_news_with_diagnostics(
        &self,
        market_hint_symbol: &str,
        curr_date: &str,
        look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<NewsFetchResult> {
        let market = self.detect_market(market_hint_symbol);
        let result = self
            .fetch_market_global_news_diagnostics(
                market_hint_symbol,
                market,
                curr_date,
                look_back_days,
                limit,
            )
            .await?;
        Ok(result)
    }
}impl MarketDataClient {

    pub async fn fetch_insider_transactions(&self, symbol: &str) -> anyhow::Result<Vec<NewsItem>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "insider", symbol);
        async {
            match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_insider_transactions(symbol).await,
                MarketKind::HongKong => self.fetch_hk_news(symbol, 8, None, None).await,
                MarketKind::UsEquity => self.fetch_us_insider_transactions(symbol).await,
            }
        }.instrument(span).await
    }

    pub async fn fetch_candles(
        &self,
        symbol: &str,
        adjust: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CandlePoint>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "candles", symbol);
        async {

            self.fetch_candles_with_provider(symbol, adjust, limit)
                .await
                .map(|(items, _)| items)
        }.instrument(span).await
    }

    pub async fn fetch_candles_with_provider(
        &self,
        symbol: &str,
        adjust: &str,
        limit: usize,
    ) -> anyhow::Result<(Vec<CandlePoint>, String)> {
        let span = tracing::info_span!("market_data.fetch", data_type = "candles", symbol);
        async {
            match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_tencent_candles(symbol, adjust, limit).await.map(|c| (c, "akshare".to_string())),
                MarketKind::HongKong => self.fetch_hk_candles(symbol, limit).await.map(|c| (c, "akshare".to_string())),
                MarketKind::UsEquity => self.fetch_us_candles(symbol, limit).await,
            }
        }.instrument(span).await
    }

    pub async fn fetch_capital_flow(
        &self,
        symbol: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CapitalFlowPoint>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "capital_flow", symbol);
        async {
            if self.normalize_a_share_symbol(symbol).is_some() {
                self.ak.a_share_capital_flow(symbol, limit).await
                    .map(|items| items.into_iter().map(super::capital_flow_from_akshare).collect::<Vec<_>>())
                    .map_err(anyhow::Error::from)
            } else {
                Err(DataError::new(
                    DataErrorKind::UnsupportedMarket,
                    format!("capital flow is unsupported for symbol {symbol}"),
                )
                .into())
            }
        }.instrument(span).await
    }

    pub async fn fetch_a_share_sector_rankings(
        &self,
        sector_type: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SectorSnapshot>> {
        let items = self.ak.a_share_sector_rankings(sector_type, limit).await?;
        Ok(items)
    }

    pub async fn fetch_a_share_sector_constituents(
        &self,
        sector_code: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SectorConstituent>> {
        let items = self.ak.a_share_sector_constituents(sector_code, limit).await?;
        Ok(items)
    }

    pub async fn fetch_a_share_sector_capital_flow(
        &self,
        sector_code: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CapitalFlowPoint>> {
        let items = self.ak.a_share_sector_capital_flow(sector_code, limit).await
            .map(|items| items.into_iter().map(super::capital_flow_from_akshare).collect::<Vec<_>>())?;
        Ok(items)
    }

    pub async fn fetch_announcement_detail(
        &self,
        art_code: &str,
    ) -> anyhow::Result<AnnouncementDetail> {
        let item = self.ak.a_share_announcement_detail(art_code).await?;
        Ok(item)
    }

    pub async fn fetch_announcements(
        &self,
        symbol: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<AnnouncementItem>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "announcements", symbol);
        async {
            if let Some(ts_code) = self.normalize_a_share_symbol(symbol) {
                self.ak.a_share_announcements(&ts_code, limit).await.map_err(anyhow::Error::from)
            } else {
                Err(DataError::new(
                    DataErrorKind::UnsupportedMarket,
                    format!("announcements are unsupported for symbol {symbol}"),
                )
                .into())
            }
        }.instrument(span).await
    }

    pub async fn fetch_billboard_entries(
        &self,
        symbol: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<BillboardEntry>> {
        if self.normalize_a_share_symbol(symbol).is_some() {
            let items = self.ak.a_share_billboard(symbol, limit).await?;
            return Ok(items);
        }

        Err(DataError::new(
            DataErrorKind::UnsupportedMarket,
            format!("billboard is unsupported for symbol {symbol}"),
        )
        .into())
    }

    pub async fn fetch_billboard_seats(
        &self,
        symbol: &str,
        side: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<BillboardSeatDetail>> {
        if self.normalize_a_share_symbol(symbol).is_some() {
            let items = self.ak.a_share_billboard_seats(symbol, side, limit).await?;
            return Ok(items);
        }

        Err(DataError::new(
            DataErrorKind::UnsupportedMarket,
            format!("billboard seats are unsupported for symbol {symbol}"),
        )
        .into())
    }

    pub async fn search_stocks(
        &self,
        query: &str,
        market: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<StockSearchResult>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "stock_search", symbol = query);
        async {
            self.ak.a_share_search(query, market, limit)
                .await
                .map(|mut items| { items.truncate(limit); items })
                .map_err(anyhow::Error::from)
        }.instrument(span).await
    }

    pub async fn fetch_trade_calendar(
        &self,
        exchange: &str,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<TradeCalendarItem>> {
        let items = self
            .ak
            .a_share_trade_calendar(exchange, start_date, end_date)
            .await
            .context("akshare a_share_trade_calendar failed")?;
        Ok(items
            .into_iter()
            .map(|item| TradeCalendarItem {
                exchange: item.exchange,
                calendar_date: item.calendar_date,
                is_open: item.is_open,
                previous_trade_date: item.previous_trade_date,
            })
            .collect())
    }
}
impl MarketDataClient {

    pub async fn fetch_return_since(
        &self,
        symbol: &str,
        start_date: &str,
        holding_days: usize,
    ) -> anyhow::Result<Option<f64>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "return_since", symbol);
        async {

            match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_return_since(symbol, start_date, holding_days).await,
                MarketKind::HongKong => self.fetch_hk_return_since(symbol, start_date, holding_days).await,
                MarketKind::UsEquity => self.fetch_us_return_since(symbol, start_date, holding_days).await,
            }
        }.instrument(span).await
    }

    pub(super) async fn fetch_market_news_diagnostics_query(
        &self,
        symbol: &str,
        market: MarketKind,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
        _query: Option<&str>,
    ) -> anyhow::Result<NewsFetchResult> {
        match market {
            MarketKind::AShare => {
                let ts_code = self
                    .normalize_a_share_symbol(symbol)
                    .context("invalid A-share symbol for news")?;
                self.fetch_a_share_news_diagnostics(&ts_code, limit).await
            }
            MarketKind::HongKong => {
                let items = self.fetch_hk_news(symbol, limit, start_date, end_date).await?;
                Ok(NewsFetchResult {
                    items,
                    attempts: vec![crate::types::NewsFetchAttempt {
                        source: "eastmoney_em_hk".to_string(),
                        query: None,
                        success: true,
                        item_count: 0,
                        error: None,
                    }],
                    cacheable: true,
                })
            }
            MarketKind::UsEquity => {
                self.fetch_us_news(symbol, limit, start_date, end_date)
                    .await
            }
        }
    }

    pub(super) async fn fetch_market_global_news_diagnostics(
        &self,
        _market_hint_symbol: &str,
        market: MarketKind,
        curr_date: &str,
        look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<NewsFetchResult> {
        match market {
            MarketKind::AShare => {
                self.fetch_a_share_global_news_diagnostics(curr_date, look_back_days, limit)
                    .await
            }
            MarketKind::HongKong => {
                anyhow::bail!("HK global news diagnostics not supported after akshare migration")
            }
            MarketKind::UsEquity => {
                self.fetch_us_global_news(curr_date, look_back_days, limit)
                    .await
            }
        }
    }
}
use akshare::stock::feature::{
    AnalystDetail, AnalystRank, BalanceSheet, CashFlowSheet, CommentDesireIndex,
    CommentFocusIndex, CommentHistScore, CommentOrgParticipation, DividendInfo, DzjyHygtj,
    DzjyHyyybtj, DzjyMrtj, DzjyYybph, EarningsForecast, EarningsQuickReport, EarningsReport,
    EsgRating, FundFlowEntry, GdfxHoldingAnalyse, GdfxHoldingChange, GdfxHoldingDetail,
    GdfxHoldingStatistic, GdfxTeamwork, GdfxTop10, Gdhs, GdhsDetail, Ggcg, GpzyDistributeEntry,
    GpzyIndustry, GpzyPledgeDetail, GpzyPledgeRatio, GpzyPledgeRatioDetail, GpzyProfile,
    HotStockXq, IndustryCategory, JgdyDetail, JgdyTj, LhbDetail, LhbHyyyb, LhbJgmmtj,
    LhbJgstatistic, LhbStockDetail, LhbStockDetailDate, LhbStockStatistic, LhbTraderStatistic,
    LhbYybDetail, LhbYybph, MainFundFlow, MarginAccountInfo, MarginRatioPa, MarginSseDetail,
    MarginSseSummary, MarginSzseDetail, MarginSzseSummary, PankouChange, ProfitSheet,
    SectorFundFlowRank, StockComment, ZtPool, ZtPoolDtgc, ZtPoolPrevious, ZtPoolStrong,
    ZtPoolSubNew, ZtPoolZbgc,
};

impl MarketDataClient {
    // -----------------------------------------------------------------------
    // Fund Flow (资金流向)
    // -----------------------------------------------------------------------

    pub async fn fetch_fund_flow_individual(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<FundFlowEntry>> {
        let items = self.ak.stock_fund_flow_individual(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_fund_flow_concept(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        let items = self.ak.stock_fund_flow_concept(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_fund_flow_industry(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        let items = self.ak.stock_fund_flow_industry(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_main_fund_flow(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<MainFundFlow>> {
        self.normalize_a_share_symbol(symbol)
            .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
        let items = self.ak.stock_main_fund_flow(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Billboard / Dragon Tiger List (龙虎榜)
    // -----------------------------------------------------------------------

    pub async fn fetch_lhb_detail(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<LhbDetail>> {
        let items = self.ak.stock_lhb_detail_em(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_statistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbStockStatistic>> {
        let items = self.ak.stock_lhb_stock_statistic_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_jgmmtj(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<LhbJgmmtj>> {
        let items = self.ak.stock_lhb_jgmmtj_em(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_jgstatistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbJgstatistic>> {
        let items = self.ak.stock_lhb_jgstatistic_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_hyyyb(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<LhbHyyyb>> {
        let items = self.ak.stock_lhb_hyyyb_em(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_yybph(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbYybph>> {
        let items = self.ak.stock_lhb_yybph_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_trader_statistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbTraderStatistic>> {
        let items = self.ak.stock_lhb_traderstatistic_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_detail_date(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbStockDetailDate>> {
        let items = self.ak.stock_lhb_stock_detail_date_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_detail(
        &self,
        symbol: &str,
        date: &str,
        flag: &str,
    ) -> anyhow::Result<Vec<LhbStockDetail>> {
        let items = self.ak.stock_lhb_stock_detail_em(symbol, date, flag).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_yyb_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbYybDetail>> {
        let items = self.ak.stock_lhb_yyb_detail_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Margin Trading (融资融券)
    // -----------------------------------------------------------------------

    pub async fn fetch_margin_account_info(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<MarginAccountInfo>> {
        let items = self.ak.stock_margin_account_info_em(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_margin_sse_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSseDetail>> {
        let items = self.ak.stock_margin_detail_sse(date).await?;
        Ok(items)
    }

    pub async fn fetch_margin_szse_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSzseDetail>> {
        let items = self.ak.stock_margin_detail_szse(date).await?;
        Ok(items)
    }

    pub async fn fetch_margin_ratio_pa(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<MarginRatioPa>> {
        let items = self.ak.stock_margin_ratio_pa(symbol, date).await?;
        Ok(items)
    }

    pub async fn fetch_margin_sse_summary(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<MarginSseSummary>> {
        let items = self.ak.stock_margin_sse(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_margin_szse_summary(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSzseSummary>> {
        let items = self.ak.stock_margin_szse(date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Limit-Up/Down Pools (涨停/跌停股池)
    // -----------------------------------------------------------------------

    pub async fn fetch_zt_pool(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPool>> {
        let items = self.ak.stock_zt_pool_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool_dtgc(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolDtgc>> {
        let items = self.ak.stock_zt_pool_dtgc_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool_previous(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolPrevious>> {
        let items = self.ak.stock_zt_pool_previous_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool_strong(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolStrong>> {
        let items = self.ak.stock_zt_pool_strong_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool_sub_new(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolSubNew>> {
        let items = self.ak.stock_zt_pool_sub_new_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool_zbgc(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolZbgc>> {
        let items = self.ak.stock_zt_pool_zbgc_em(date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Earnings (业绩)
    // -----------------------------------------------------------------------

    pub async fn fetch_earnings_forecast(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsForecast>> {
        let items = self.ak.stock_yjyg_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_earnings_quick_report(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsQuickReport>> {
        let items = self.ak.stock_yjkb_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_earnings_report(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsReport>> {
        let items = self.ak.stock_yjbb_em(date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Analyst (分析师)
    // -----------------------------------------------------------------------

    pub async fn fetch_analyst_rank(
        &self,
        year: &str,
    ) -> anyhow::Result<Vec<AnalystRank>> {
        let items = self.ak.stock_analyst_rank_em(year).await?;
        Ok(items)
    }

    pub async fn fetch_analyst_detail(
        &self,
        analyst_id: &str,
        indicator: &str,
    ) -> anyhow::Result<Vec<AnalystDetail>> {
        let items = self.ak.stock_analyst_detail_em(analyst_id, indicator).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Analysis (股东分析)
    // -----------------------------------------------------------------------

    pub async fn fetch_gdfx_free_holding_statistics(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        let items = self.ak.stock_gdfx_free_holding_statistics_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_statistics(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        let items = self.ak.stock_gdfx_holding_statistics_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_change(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        let items = self.ak.stock_gdfx_free_holding_change_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_change(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        let items = self.ak.stock_gdfx_holding_change_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_top10(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxTop10>> {
        let items = self.ak.stock_gdfx_free_top_10_em(symbol, date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_top10(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxTop10>> {
        let items = self.ak.stock_gdfx_top_10_em(symbol, date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        let items = self.ak.stock_gdfx_free_holding_detail_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_detail(
        &self,
        date: &str,
        indicator: &str,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        let items = self.ak.stock_gdfx_holding_detail_em(date, indicator, symbol).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_analyse(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        let items = self.ak.stock_gdfx_free_holding_analyse_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_analyse(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        let items = self.ak.stock_gdfx_holding_analyse_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_teamwork(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxTeamwork>> {
        let items = self.ak.stock_gdfx_free_holding_teamwork_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_gdfx_teamwork(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxTeamwork>> {
        let items = self.ak.stock_gdfx_holding_teamwork_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Block Trades (大宗交易)
    // -----------------------------------------------------------------------

    pub async fn fetch_block_trade_daily(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyMrtj>> {
        let items = self.ak.stock_dzjy_mrtj(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_block_trade_industry(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyHygtj>> {
        let items = self.ak.stock_dzjy_hygtj(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_block_trade_industry_daily(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyHyyybtj>> {
        let items = self.ak.stock_dzjy_hyyybtj(start_date, end_date).await?;
        Ok(items)
    }

    pub async fn fetch_block_trade_seat_ranking(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyYybph>> {
        let items = self.ak.stock_dzjy_yybph(start_date, end_date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Hot Stocks (雪球热度)
    // -----------------------------------------------------------------------

    pub async fn fetch_hot_follow_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let items = self.ak.stock_hot_follow_xq(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hot_tweet_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let items = self.ak.stock_hot_tweet_xq(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hot_deal_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let items = self.ak.stock_hot_deal_xq(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Order Book Changes (盘口异动)
    // -----------------------------------------------------------------------

    pub async fn fetch_pankou_changes(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<PankouChange>> {
        self.ak.stock_changes_em(symbol).await.map_err(Into::into)
    }

    // -----------------------------------------------------------------------
    // Dividends (分红送配)
    // -----------------------------------------------------------------------

    pub async fn fetch_dividends(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<DividendInfo>> {
        let items = self.ak.stock_fhps_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_dividend_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<DividendInfo>> {
        let items = self.ak.stock_fhps_detail_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Pledge Data (股权质押)
    // -----------------------------------------------------------------------

    pub async fn fetch_pledge_profile(
        &self,
    ) -> anyhow::Result<Vec<GpzyProfile>> {
        let items = self.ak.stock_gpzy_profile_em().await?;
        Ok(items)
    }

    pub async fn fetch_pledge_ratio(
        &self,
    ) -> anyhow::Result<Vec<GpzyPledgeRatio>> {
        let items = self.ak.stock_gpzy_pledge_ratio_em().await?;
        Ok(items)
    }

    pub async fn fetch_pledge_detail(
        &self,
    ) -> anyhow::Result<Vec<GpzyPledgeDetail>> {
        let items = self.ak.stock_gpzy_pledge_detail_em().await?;
        Ok(items)
    }

    pub async fn fetch_pledge_ratio_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GpzyPledgeRatioDetail>> {
        let items = self.ak.stock_gpzy_individual_pledge_ratio_detail_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_pledge_distribute_bank(
        &self,
    ) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        let items = self.ak.stock_gpzy_distribute_statistics_bank_em().await?;
        Ok(items)
    }

    pub async fn fetch_pledge_distribute_company(
        &self,
    ) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        let items = self.ak.stock_gpzy_distribute_statistics_company_em().await?;
        Ok(items)
    }

    pub async fn fetch_pledge_industry(
        &self,
    ) -> anyhow::Result<Vec<GpzyIndustry>> {
        let items = self.ak.stock_gpzy_industry_data_em().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Institutional Research (机构调研)
    // -----------------------------------------------------------------------

    pub async fn fetch_institutional_research(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<JgdyTj>> {
        let items = self.ak.stock_jgdy_tj_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_institutional_research_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<JgdyDetail>> {
        let items = self.ak.stock_jgdy_detail_em(date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // ESG Ratings
    // -----------------------------------------------------------------------

    pub async fn fetch_esg_msci(&self) -> anyhow::Result<Vec<EsgRating>> {
        let items = self.ak.stock_esg_msci_sina().await?;
        Ok(items)
    }

    pub async fn fetch_esg_rft(&self) -> anyhow::Result<Vec<EsgRating>> {
        let items = self.ak.stock_esg_rft_sina().await?;
        Ok(items)
    }

    pub async fn fetch_esg_zd(&self) -> anyhow::Result<Vec<EsgRating>> {
        let items = self.ak.stock_esg_zd_sina().await?;
        Ok(items)
    }

    pub async fn fetch_esg_hz(&self) -> anyhow::Result<Vec<EsgRating>> {
        let items = self.ak.stock_esg_hz_sina().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Financial Reports (三大报表)
    // -----------------------------------------------------------------------

    pub async fn fetch_balance_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<BalanceSheet>> {
        let items = self.ak.stock_zcfz_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_profit_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ProfitSheet>> {
        let items = self.ak.stock_lrb_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_cash_flow_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<CashFlowSheet>> {
        let items = self.ak.stock_xjll_em(date).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Stock Comments (千股千评)
    // -----------------------------------------------------------------------

    pub async fn fetch_stock_comments(&self) -> anyhow::Result<Vec<StockComment>> {
        let items = self.ak.stock_comment_em().await?;
        Ok(items)
    }

    pub async fn fetch_comment_org_participation(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentOrgParticipation>> {
        let items = self.ak.stock_comment_detail_zlkp_jgcyd_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_comment_hist_score(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentHistScore>> {
        let items = self.ak.stock_comment_detail_zhpj_lspf_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_comment_focus_index(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentFocusIndex>> {
        let items = self.ak.stock_comment_detail_scrd_focus_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_comment_desire_index(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentDesireIndex>> {
        let items = self.ak.stock_comment_detail_scrd_desire_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Changes (高管持股变动)
    // -----------------------------------------------------------------------

    pub async fn fetch_executive_shareholding(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<Ggcg>> {
        let items = self.ak.stock_ggcg_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Count (股东户数)
    // -----------------------------------------------------------------------

    pub async fn fetch_shareholder_count(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<Gdhs>> {
        let items = self.ak.stock_gdhs_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_shareholder_count_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdhsDetail>> {
        let items = self.ak.stock_gdhs_detail_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Industry Classification (行业分类)
    // -----------------------------------------------------------------------

    pub async fn fetch_industry_category(&self) -> anyhow::Result<Vec<IndustryCategory>> {
        let items = self.ak.stock_industry_category_cninfo().await?;
        Ok(items)
    }
}
use akshare::stock::hk_extra::{
    HkFamousStock, HkFhpxDetailThs, HkGxlLg, HkHotRank, HkHotRankDetail, HkSpotQuote,
    HkValuationBaidu,
};
use akshare::stock::us_extra::{UsFamousStock, UsPinkStock, UsSpotSina, UsValuationBaidu};
use akshare::stock::xueqiu::XqStockSpot;

impl MarketDataClient {
    // -----------------------------------------------------------------------
    // HK Spot (Sina)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_spot(&self) -> anyhow::Result<Vec<HkSpotQuote>> {
        let items = self.ak.stock_hk_spot().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Famous Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_famous_spot(&self) -> anyhow::Result<Vec<HkFamousStock>> {
        let items = self.ak.stock_hk_famous_spot_em().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Hot Rank (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_hot_rank(&self) -> anyhow::Result<Vec<HkHotRank>> {
        let items = self.ak.stock_hk_hot_rank_em().await?;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_latest(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let items = self.ak.stock_hk_hot_rank_latest_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let items = self.ak.stock_hk_hot_rank_detail_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_realtime(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let items = self.ak.stock_hk_hot_rank_detail_realtime_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Dividends (Eastmoney + THS)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_dividend_payout(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let items = self.ak.stock_hk_dividend_payout_em(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hk_fhpx_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkFhpxDetailThs>> {
        let items = self.ak.stock_hk_fhpx_detail_ths(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_hk_dividend_yield(&self) -> anyhow::Result<Vec<HkGxlLg>> {
        let items = self.ak.stock_hk_gxl_lg().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Financial Indicators (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_financial_indicators(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let items =
            self.ak.stock_hk_financial_indicator_em(symbol).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Valuation (Baidu)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_valuation(
        &self,
        symbol: &str,
        indicator: &str,
        period: &str,
    ) -> anyhow::Result<Vec<HkValuationBaidu>> {
        let items =
            self.ak.stock_hk_valuation_baidu(symbol, indicator, period).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Spot (Sina / Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_spot(&self) -> anyhow::Result<Vec<UsSpotSina>> {
        let items = self.ak.stock_us_spot().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Famous Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_famous_spot(
        &self,
        category: &str,
    ) -> anyhow::Result<Vec<UsFamousStock>> {
        let items = self.ak.stock_us_famous_spot_em(category).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Pink Sheet Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_pink_spot(&self) -> anyhow::Result<Vec<UsPinkStock>> {
        let items = self.ak.stock_us_pink_spot_em().await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Valuation (Baidu)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_valuation(
        &self,
        symbol: &str,
        indicator: &str,
        period: &str,
    ) -> anyhow::Result<Vec<UsValuationBaidu>> {
        let items =
            self.ak.stock_us_valuation_baidu(symbol, indicator, period).await?;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Xueqiu Spot (works for HK and US symbols)
    // -----------------------------------------------------------------------

    pub async fn fetch_xq_spot(&self, symbol: &str) -> anyhow::Result<XqStockSpot> {
        // Try HK first, then US -- both delegate to the same Xueqiu endpoint
        let market = self.detect_market(symbol);
        let items = match market {
            crate::types::MarketKind::HongKong => {
                self.ak.stock_individual_spot_xq(symbol).await?
            }
            _ => self.ak.stock_individual_spot_xq(symbol).await?,
        };
        Ok(items)
    }
}
