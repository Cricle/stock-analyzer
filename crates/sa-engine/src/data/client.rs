use anyhow::Context;
use tracing::Instrument;

use super::{
    BillboardEntry, CandlePoint, CapitalFlowPoint, DataError, FundamentalsSnapshot,
    MarketDataClient, MarketKind, NewsItem, QuoteSnapshot,
};
use crate::types::NewsFetchResult;
use akshare::types::{SectorConstituent, SectorSnapshot, StockSearchResult};
use akshare::stock::feature::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};

impl MarketDataClient {
    pub async fn new() -> Self {
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
        Self { ak: ak_builder.build() }
    }

    pub fn detect_market(&self, symbol: &str) -> MarketKind {
        akshare::detect_market(symbol).into()
    }

    pub(super) fn normalize_a_share_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_a_share_symbol(symbol)
    }

    pub(super) fn normalize_hk_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_hk_symbol(symbol).map(|code| format!("{code}.HK"))
    }

    pub fn candles_source(&self, symbol: &str) -> &'static str {
        match self.detect_market(symbol) {
            MarketKind::AShare => "akshare:tencent_kline+eastmoney_kline",
            MarketKind::HongKong => "akshare:tencent_kline+yahoo_finance_chart",
            MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
        }
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

    pub(crate) async fn fetch_enrichment(
        &self,
        symbol: &str,
    ) -> anyhow::Result<super::a_share::AShareEnrichmentData> {
        let span = tracing::info_span!("market_data.fetch", data_type = "enrichment", symbol);
        async {
            match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_enrichment(symbol).await,
                _ => Ok(super::a_share::AShareEnrichmentData::default()),
            }
        }.instrument(span).await
    }

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
                    .map_err(anyhow::Error::from)
            } else {
                Err(DataError::new(
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
            format!("billboard is unsupported for symbol {symbol}"),
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
                self.fetch_hk_global_news(curr_date, look_back_days, limit)
                    .await
            }
            MarketKind::UsEquity => {
                self.fetch_us_global_news(curr_date, look_back_days, limit)
                    .await
            }
        }
    }

    // -----------------------------------------------------------------------
    // Enrichment data used by task_run
    // -----------------------------------------------------------------------

    pub async fn fetch_fund_flow_individual(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<FundFlowEntry>> {
        let items = self.ak.stock_fund_flow_individual(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_statistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbStockStatistic>> {
        let items = self.ak.stock_lhb_stock_statistic_em(symbol).await?;
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

    pub async fn fetch_hot_follow_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let items = self.ak.stock_hot_follow_xq(symbol).await?;
        Ok(items)
    }

    pub async fn fetch_earnings_forecast(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsForecast>> {
        let items = self.ak.stock_yjyg_em(date).await?;
        Ok(items)
    }

    pub async fn fetch_zt_pool(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPool>> {
        let items = self.ak.stock_zt_pool_em(date).await?;
        Ok(items)
    }
}
