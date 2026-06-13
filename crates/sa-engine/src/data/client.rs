use anyhow::Context;
use tracing::Instrument;

use super::{
    BillboardEntry, CANDLES_CACHE_TTL_SECS, CANDLES_CACHE_VERSION, CandlePoint,
    CapitalFlowPoint, DataConfig, DataError, DataErrorKind, FUNDAMENTALS_CACHE_TTL_SECS,
    FUNDAMENTALS_CACHE_VERSION, FundamentalsSnapshot, GLOBAL_NEWS_CACHE_VERSION,
    INSIDER_CACHE_TTL_SECS, MARKET_DATA_CACHE_PREFIX, MarketDataClient, MarketKind,
    NEWS_CACHE_TTL_SECS, NEWS_CACHE_VERSION, NewsItem, QUOTE_CACHE_TTL_SECS,
    QUOTE_CACHE_VERSION, Singleflight, SingleflightResult, QuoteSnapshot,
};
use crate::types::{NewsFetchAttempt, NewsFetchResult};
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

        Self {
            ak,
            singleflight: Singleflight::new(),
        }
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
            let market = self.detect_market(symbol);
            let normalized_symbol = self.cache_symbol(symbol, market);
            let cache_key =
                format!("{MARKET_DATA_CACHE_PREFIX}:quote:{QUOTE_CACHE_VERSION}:{normalized_symbol}");
            if let Some(cached) = self.cache_get_json::<QuoteSnapshot>(&cache_key).await {
                return Ok((cached, "redis_cache".to_string()));
            }
            // Singleflight: prevent cache stampede when multiple users request the same stock
            let sf = self.singleflight.clone();
            let _sf_guard = match sf.enter(&cache_key).await {
                SingleflightResult::Leader(g) => Some(g),
                SingleflightResult::Waiting => {
                    if let Some(cached) = self.cache_get_json::<QuoteSnapshot>(&cache_key).await {
                        return Ok((cached, "redis_cache".to_string()));
                    }
                    None
                }
            };
            let result: anyhow::Result<(QuoteSnapshot, String)> = match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_quote_from_eastmoney(symbol).await.map(|q| (q, "akshare".to_string())),
                MarketKind::HongKong => self.fetch_hk_quote(symbol).await,
                MarketKind::UsEquity => self.fetch_us_quote(symbol).await,
            };
            let (snapshot, provider_used) = result?;
            self.cache_set_json(&cache_key, QUOTE_CACHE_TTL_SECS, &snapshot)
                .await;
            Ok((snapshot, provider_used))
        }.instrument(span).await
    }

    pub async fn fetch_fundamentals(&self, symbol: &str) -> anyhow::Result<FundamentalsSnapshot> {
        let span = tracing::info_span!("market_data.fetch", data_type = "fundamentals", symbol);
        async {
            let market = self.detect_market(symbol);
            let normalized_symbol = self.cache_symbol(symbol, market);
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:fundamentals:{FUNDAMENTALS_CACHE_VERSION}:{normalized_symbol}"
            );
            if let Some(cached) = self
                .cache_get_json::<FundamentalsSnapshot>(&cache_key)
                .await
            {
                return Ok(cached);
            }
            // Singleflight: prevent cache stampede
            let sf = self.singleflight.clone();
            let _sf_guard = match sf.enter(&cache_key).await {
                SingleflightResult::Leader(g) => Some(g),
                SingleflightResult::Waiting => {
                    if let Some(cached) = self.cache_get_json::<FundamentalsSnapshot>(&cache_key).await {
                        return Ok(cached);
                    }
                    None
                }
            };
            let result = match self.detect_market(symbol) {
                MarketKind::AShare => {
                    let ts_code = self.normalize_a_share_symbol(symbol)
                        .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
                    self.fetch_a_share_fundamentals(symbol, &ts_code).await
                }
                MarketKind::HongKong => self.fetch_hk_fundamentals(symbol).await,
                MarketKind::UsEquity => self.fetch_us_fundamentals(symbol).await,
            };
            let snapshot = result?;
            self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &snapshot)
                .await;
            Ok(snapshot)
        }.instrument(span).await
    }

    /// Batch fetch quotes for multiple symbols. Returns (symbol, `Option<QuoteSnapshot>`).
    /// Cache hits are returned from Redis MGET; misses are fetched concurrently via akshare.
    pub async fn fetch_quotes_batch(
        &self,
        symbols: &[&str],
    ) -> Vec<(String, Option<QuoteSnapshot>)> {
        if symbols.is_empty() {
            return Vec::new();
        }
        // Build cache keys
        let keys: Vec<(String, String)> = symbols
            .iter()
            .map(|sym| {
                let market = self.detect_market(sym);
                let normalized = self.cache_symbol(sym, market);
                let cache_key =
                    format!("{MARKET_DATA_CACHE_PREFIX}:quote:{QUOTE_CACHE_VERSION}:{normalized}");
                (sym.to_string(), cache_key)
            })
            .collect();
        let cache_keys: Vec<String> = keys.iter().map(|(_, k)| k.clone()).collect();
        // Batch MGET
        let cached: Vec<Option<QuoteSnapshot>> = self.cache_mget_json(&cache_keys).await;
        // Collect misses
        let mut miss_indices = Vec::new();
        for (i, hit) in cached.iter().enumerate() {
            if hit.is_none() {
                miss_indices.push(i);
            }
        }
        // Fetch misses concurrently
        let miss_results: Vec<(usize, Option<QuoteSnapshot>)> = if miss_indices.is_empty() {
            Vec::new()
        } else {
            let futs: Vec<_> = miss_indices
                .iter()
                .map(|&i| {
                    let sym = keys[i].0.clone();
                    let client = self.clone();
                    async move {
                        let result = tokio::time::timeout(
                            std::time::Duration::from_secs(15),
                            async {
                                match client.detect_market(&sym) {
                                    MarketKind::AShare => client.fetch_a_share_quote_from_eastmoney(&sym).await.map(|q| (q, "akshare".to_string())),
                                    MarketKind::HongKong => client.fetch_hk_quote(&sym).await,
                                    MarketKind::UsEquity => client.fetch_us_quote(&sym).await,
                                }
                            },
                        )
                        .await;
                        let quote = match result {
                            Ok(Ok((q, _))) => {
                                // Write to cache
                                let market = client.detect_market(&sym);
                                let normalized = client.cache_symbol(&sym, market);
                                let cache_key = format!(
                                    "{MARKET_DATA_CACHE_PREFIX}:quote:{QUOTE_CACHE_VERSION}:{normalized}"
                                );
                                client
                                    .cache_set_json(&cache_key, QUOTE_CACHE_TTL_SECS, &q)
                                    .await;
                                Some(q)
                            }
                            _ => None,
                        };
                        (i, quote)
                    }
                })
                .collect();
            futures::future::join_all(futs).await
        };
        // Merge results
        let mut results: Vec<Option<QuoteSnapshot>> = cached;
        for (i, quote) in miss_results {
            results[i] = quote;
        }
        keys.into_iter()
            .zip(results)
            .map(|((sym, _), q)| (sym, q))
            .collect()
    }

    /// Batch fetch fundamentals for multiple symbols.
    pub async fn fetch_fundamentals_batch(
        &self,
        symbols: &[&str],
    ) -> Vec<(String, Option<FundamentalsSnapshot>)> {
        if symbols.is_empty() {
            return Vec::new();
        }
        let keys: Vec<(String, String)> = symbols
            .iter()
            .map(|sym| {
                let market = self.detect_market(sym);
                let normalized = self.cache_symbol(sym, market);
                let cache_key = format!(
                    "{MARKET_DATA_CACHE_PREFIX}:fundamentals:{FUNDAMENTALS_CACHE_VERSION}:{normalized}"
                );
                (sym.to_string(), cache_key)
            })
            .collect();
        let cache_keys: Vec<String> = keys.iter().map(|(_, k)| k.clone()).collect();
        let cached: Vec<Option<FundamentalsSnapshot>> = self.cache_mget_json(&cache_keys).await;
        let mut miss_indices = Vec::new();
        for (i, hit) in cached.iter().enumerate() {
            if hit.is_none() {
                miss_indices.push(i);
            }
        }
        let miss_results: Vec<(usize, Option<FundamentalsSnapshot>)> = if miss_indices.is_empty() {
            Vec::new()
        } else {
            let futs: Vec<_> = miss_indices
                .iter()
                .map(|&i| {
                    let sym = keys[i].0.clone();
                    let client = self.clone();
                    async move {
                        let result = tokio::time::timeout(
                            std::time::Duration::from_secs(15),
                            async {
                                match client.detect_market(&sym) {
                                    MarketKind::AShare => {
                                        let ts_code = client.normalize_a_share_symbol(&sym)
                                            .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
                                        client.fetch_a_share_fundamentals(&sym, &ts_code).await
                                    }
                                    MarketKind::HongKong => client.fetch_hk_fundamentals(&sym).await,
                                    MarketKind::UsEquity => client.fetch_us_fundamentals(&sym).await,
                                }
                            },
                        )
                        .await;
                        let fund = match result {
                            Ok(Ok(f)) => {
                                let market = client.detect_market(&sym);
                                let normalized = client.cache_symbol(&sym, market);
                                let cache_key = format!(
                                    "{MARKET_DATA_CACHE_PREFIX}:fundamentals:{FUNDAMENTALS_CACHE_VERSION}:{normalized}"
                                );
                                client
                                    .cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &f)
                                    .await;
                                Some(f)
                            }
                            _ => None,
                        };
                        (i, fund)
                    }
                })
                .collect();
            futures::future::join_all(futs).await
        };
        let mut results: Vec<Option<FundamentalsSnapshot>> = cached;
        for (i, fund) in miss_results {
            results[i] = fund;
        }
        keys.into_iter()
            .zip(results)
            .map(|((sym, _), f)| (sym, f))
            .collect()
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
            let normalized_symbol = self.cache_symbol(symbol, market);
            let query_cache_key = self.news_query_cache_component(query);
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:news:{NEWS_CACHE_VERSION}:{normalized_symbol}:{limit}:{}:{}:{query_cache_key}",
                start_date.unwrap_or("-"),
                end_date.unwrap_or("-")
            );
            if let Some(mut cached) = self.cache_get_json::<NewsFetchResult>(&cache_key).await {
                if cached.attempts.is_empty() {
                    cached.attempts.push(NewsFetchAttempt {
                        source: "redis_cache".to_string(),
                        query: None,
                        success: true,
                        item_count: cached.items.len(),
                        error: None,
                    });
                }
                cached.cacheable = true;
                return Ok(cached);
            }
            let result = self
                .fetch_market_news_diagnostics_query(symbol, market, limit, start_date, end_date, query)
                .await;
            let result = result?;
            if result.cacheable && !result.items.is_empty() {
                self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &result)
                    .await;
            }
            Ok(result)
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
        let normalized_symbol = self.cache_symbol(market_hint_symbol, market);
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:global-news:{GLOBAL_NEWS_CACHE_VERSION}:{normalized_symbol}:{curr_date}:{look_back_days}:{limit}"
        );
        if let Some(mut cached) = self.cache_get_json::<NewsFetchResult>(&cache_key).await {
            if cached.attempts.is_empty() {
                cached.attempts.push(NewsFetchAttempt {
                    source: "redis_cache".to_string(),
                    query: None,
                    success: true,
                    item_count: cached.items.len(),
                    error: None,
                });
            }
            cached.cacheable = true;
            return Ok(cached);
        }
        let result = self
            .fetch_market_global_news_diagnostics(
                market_hint_symbol,
                market,
                curr_date,
                look_back_days,
                limit,
            )
            .await?;
        if result.cacheable && !result.items.is_empty() {
            self.cache_set_json(&cache_key, super::GLOBAL_NEWS_CACHE_TTL_SECS, &result)
                .await;
        }
        Ok(result)
    }
}impl MarketDataClient {

    pub async fn fetch_insider_transactions(&self, symbol: &str) -> anyhow::Result<Vec<NewsItem>> {
        let span = tracing::info_span!("market_data.fetch", data_type = "insider", symbol);
        async {
            let market = self.detect_market(symbol);
            let normalized_symbol = self.cache_symbol(symbol, market);
            let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:insider:{normalized_symbol}");
            if let Some(cached) = self.cache_get_json(&cache_key).await {
                return Ok(cached);
            }
            let result = match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_insider_transactions(symbol).await,
                MarketKind::HongKong => self.fetch_hk_news(symbol, 8, None, None).await,
                MarketKind::UsEquity => self.fetch_us_insider_transactions(symbol).await,
            };
            let items = result?;
            self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items)
                .await;
            Ok(items)
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
            let market = self.detect_market(symbol);
            let normalized_symbol = self.cache_symbol(symbol, market);
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:candles:{CANDLES_CACHE_VERSION}:{normalized_symbol}:{adjust}:{limit}"
            );
            if let Some(cached) = self.cache_get_json(&cache_key).await {
                return Ok((cached, "redis_cache".to_string()));
            }
            // Singleflight: prevent cache stampede
            let sf = self.singleflight.clone();
            let _sf_guard = match sf.enter(&cache_key).await {
                SingleflightResult::Leader(g) => Some(g),
                SingleflightResult::Waiting => {
                    if let Some(cached) = self.cache_get_json(&cache_key).await {
                        return Ok((cached, "redis_cache".to_string()));
                    }
                    None
                }
            };
            let result: anyhow::Result<(Vec<CandlePoint>, String)> = match self.detect_market(symbol) {
                MarketKind::AShare => self.fetch_a_share_tencent_candles(symbol, adjust, limit).await.map(|c| (c, "akshare".to_string())),
                MarketKind::HongKong => self.fetch_hk_candles(symbol, limit).await.map(|c| (c, "akshare".to_string())),
                MarketKind::UsEquity => self.fetch_us_candles(symbol, limit).await,
            };
            let (items, provider_used) = result?;
            if !items.is_empty() {
                self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
                    .await;
            }
            Ok((items, provider_used))
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
                let market = self.detect_market(symbol);
                let normalized_symbol = self.cache_symbol(symbol, market);
                let cache_key =
                    format!("{MARKET_DATA_CACHE_PREFIX}:capital-flow:{normalized_symbol}:{limit}");
                if let Some(cached) = self.cache_get_json(&cache_key).await {
                    return Ok(cached);
                }
                let fetch_result = self.ak.a_share_capital_flow(symbol, limit).await
                    .map(|items| items.into_iter().map(super::capital_flow_from_akshare).collect::<Vec<_>>());
                let items = fetch_result?;
                self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
                    .await;
                Ok(items)
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:a-share-sector-rankings:{}:{}",
            sector_type.trim().to_lowercase(),
            limit
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.a_share_sector_rankings(sector_type, limit).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_a_share_sector_constituents(
        &self,
        sector_code: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SectorConstituent>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:a-share-sector-constituents:{}:{}",
            sector_code.trim().to_uppercase(),
            limit
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.a_share_sector_constituents(sector_code, limit).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_a_share_sector_capital_flow(
        &self,
        sector_code: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CapitalFlowPoint>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:a-share-sector-capital-flow:{}:{}",
            sector_code.trim().to_uppercase(),
            limit
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.a_share_sector_capital_flow(sector_code, limit).await
            .map(|items| items.into_iter().map(super::capital_flow_from_akshare).collect::<Vec<_>>())?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_announcement_detail(
        &self,
        art_code: &str,
    ) -> anyhow::Result<AnnouncementDetail> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:announcement-detail:{}",
            art_code.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let item = self.ak.a_share_announcement_detail(art_code).await?;
        self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &item)
            .await;
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
                let cache_key = format!(
                    "{MARKET_DATA_CACHE_PREFIX}:announcements:{}:{}",
                    ts_code, limit
                );
                if let Some(cached) = self.cache_get_json(&cache_key).await {
                    return Ok(cached);
                }
                let fetch_result = self.ak.a_share_announcements(&ts_code, limit).await.map_err(anyhow::Error::from);
                let items = fetch_result?;
                self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &items)
                    .await;
                Ok(items)
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
            let normalized = self.cache_symbol(symbol, MarketKind::AShare);
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:billboard-entries:{}:{}",
                normalized, limit
            );
            if let Some(cached) = self.cache_get_json(&cache_key).await {
                return Ok(cached);
            }
            let items = self.ak.a_share_billboard(symbol, limit).await?;
            self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &items)
                .await;
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
            let normalized = self.cache_symbol(symbol, MarketKind::AShare);
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:billboard-seats:{}:{}:{}",
                normalized,
                side.trim().to_lowercase(),
                limit
            );
            if let Some(cached) = self.cache_get_json(&cache_key).await {
                return Ok(cached);
            }
            let items = self.ak.a_share_billboard_seats(symbol, side, limit).await?;
            self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &items)
                .await;
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
            let normalized_query = query.trim().to_uppercase();
            let normalized_market = market.unwrap_or("all");
            let cache_key = format!(
                "{MARKET_DATA_CACHE_PREFIX}:search:{NEWS_CACHE_VERSION}:{normalized_market}:{}:{limit}",
                normalized_query
            );
            if let Some(cached) = self.cache_get_json(&cache_key).await {
                return Ok(cached);
            }
            let result = self.ak.a_share_search(query, market, limit)
                .await
                .map(|mut items| { items.truncate(limit); items });
            let items = result?;
            self.cache_set_json(&cache_key, NEWS_CACHE_TTL_SECS, &items)
                .await;
            Ok(items)
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
                    attempts: vec![NewsFetchAttempt {
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

const ESG_CACHE_TTL_SECS: u64 = 24 * 60 * 60;
const INDUSTRY_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

impl MarketDataClient {
    // -----------------------------------------------------------------------
    // Fund Flow (资金流向)
    // -----------------------------------------------------------------------

    pub async fn fetch_fund_flow_individual(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<FundFlowEntry>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:fund-flow-individual:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_fund_flow_individual(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_fund_flow_concept(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:fund-flow-concept:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_fund_flow_concept(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_fund_flow_industry(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:fund-flow-industry:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_fund_flow_industry(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_main_fund_flow(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<MainFundFlow>> {
        let normalized = self.normalize_a_share_symbol(symbol)
            .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol for main fund flow"))?;
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:main-fund-flow:{}",
            normalized
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_main_fund_flow(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-detail:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_detail_em(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_statistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbStockStatistic>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-stock-stat:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_stock_statistic_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_jgmmtj(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<LhbJgmmtj>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-jgmmtj:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_jgmmtj_em(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_jgstatistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbJgstatistic>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-jgstat:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_jgstatistic_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_hyyyb(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<LhbHyyyb>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-hyyyb:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_hyyyb_em(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_yybph(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbYybph>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-yybph:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_yybph_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_trader_statistic(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbTraderStatistic>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-trader:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_traderstatistic_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_detail_date(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbStockDetailDate>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-stock-date:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_stock_detail_date_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_stock_detail(
        &self,
        symbol: &str,
        date: &str,
        flag: &str,
    ) -> anyhow::Result<Vec<LhbStockDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-stock-detail:{}:{}:{}",
            symbol.trim(), date, flag
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_stock_detail_em(symbol, date, flag).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_lhb_yyb_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<LhbYybDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:lhb-yyb-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lhb_yyb_detail_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-account:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_account_info_em(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_margin_sse_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSseDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-sse-detail:{}",
            date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_detail_sse(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_margin_szse_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSzseDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-szse-detail:{}",
            date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_detail_szse(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_margin_ratio_pa(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<MarginRatioPa>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-ratio-pa:{}:{}",
            symbol.trim(), date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_ratio_pa(symbol, date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_margin_sse_summary(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<MarginSseSummary>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-sse-summary:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_sse(start_date, end_date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_margin_szse_summary(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<MarginSzseSummary>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:margin-szse-summary:{}",
            date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_margin_szse(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Limit-Up/Down Pools (涨停/跌停股池)
    // -----------------------------------------------------------------------

    pub async fn fetch_zt_pool(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPool>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_zt_pool_dtgc(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolDtgc>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool-dtgc:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_dtgc_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_zt_pool_previous(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolPrevious>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool-prev:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_previous_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_zt_pool_strong(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolStrong>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool-strong:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_strong_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_zt_pool_sub_new(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolSubNew>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool-subnew:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_sub_new_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_zt_pool_zbgc(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ZtPoolZbgc>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:zt-pool-zbgc:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zt_pool_zbgc_em(date).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Earnings (业绩)
    // -----------------------------------------------------------------------

    pub async fn fetch_earnings_forecast(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsForecast>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:earnings-forecast:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_yjyg_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_earnings_quick_report(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsQuickReport>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:earnings-quick:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_yjkb_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_earnings_report(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<EarningsReport>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:earnings-report:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_yjbb_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Analyst (分析师)
    // -----------------------------------------------------------------------

    pub async fn fetch_analyst_rank(
        &self,
        year: &str,
    ) -> anyhow::Result<Vec<AnalystRank>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:analyst-rank:{}", year);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_analyst_rank_em(year).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_analyst_detail(
        &self,
        analyst_id: &str,
        indicator: &str,
    ) -> anyhow::Result<Vec<AnalystDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:analyst-detail:{}:{}",
            analyst_id, indicator
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_analyst_detail_em(analyst_id, indicator).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Analysis (股东分析)
    // -----------------------------------------------------------------------

    pub async fn fetch_gdfx_free_holding_statistics(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-free-stat:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_holding_statistics_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_statistics(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-stat:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_holding_statistics_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_change(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-free-change:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_holding_change_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_change(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-change:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_holding_change_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_top10(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxTop10>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdfx-free-top10:{}:{}",
            symbol.trim(), date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_top_10_em(symbol, date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_top10(
        &self,
        symbol: &str,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxTop10>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdfx-top10:{}:{}",
            symbol.trim(), date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_top_10_em(symbol, date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-free-detail:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_holding_detail_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_detail(
        &self,
        date: &str,
        indicator: &str,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdfx-detail:{}:{}:{}",
            date, indicator, symbol
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_holding_detail_em(date, indicator, symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_holding_analyse(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-free-analyse:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_holding_analyse_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_holding_analyse(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdfx-analyse:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_holding_analyse_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_free_teamwork(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxTeamwork>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdfx-free-team:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_free_holding_teamwork_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_gdfx_teamwork(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdfxTeamwork>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdfx-team:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdfx_holding_teamwork_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:dzjy-mrtj:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_dzjy_mrtj(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_block_trade_industry(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyHygtj>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:dzjy-hygtj:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_dzjy_hygtj(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_block_trade_industry_daily(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyHyyybtj>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:dzjy-hyyybtj:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_dzjy_hyyybtj(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_block_trade_seat_ranking(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> anyhow::Result<Vec<DzjyYybph>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:dzjy-yybph:{}:{}",
            start_date, end_date
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_dzjy_yybph(start_date, end_date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Hot Stocks (雪球热度)
    // -----------------------------------------------------------------------

    pub async fn fetch_hot_follow_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hot-follow:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hot_follow_xq(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_hot_tweet_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hot-tweet:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hot_tweet_xq(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_hot_deal_xq(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HotStockXq>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hot-deal:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hot_deal_xq(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items).await;
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
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:dividends:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_fhps_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_dividend_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<DividendInfo>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:dividend-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_fhps_detail_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Pledge Data (股权质押)
    // -----------------------------------------------------------------------

    pub async fn fetch_pledge_profile(
        &self,
    ) -> anyhow::Result<Vec<GpzyProfile>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-profile");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_profile_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_ratio(
        &self,
    ) -> anyhow::Result<Vec<GpzyPledgeRatio>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-ratio");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_pledge_ratio_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_detail(
        &self,
    ) -> anyhow::Result<Vec<GpzyPledgeDetail>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-detail");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_pledge_detail_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_ratio_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GpzyPledgeRatioDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:pledge-ratio-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_individual_pledge_ratio_detail_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_distribute_bank(
        &self,
    ) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-dist-bank");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_distribute_statistics_bank_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_distribute_company(
        &self,
    ) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-dist-company");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_distribute_statistics_company_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_pledge_industry(
        &self,
    ) -> anyhow::Result<Vec<GpzyIndustry>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:pledge-industry");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gpzy_industry_data_em().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Institutional Research (机构调研)
    // -----------------------------------------------------------------------

    pub async fn fetch_institutional_research(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<JgdyTj>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:jgdy-tj:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_jgdy_tj_em(date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_institutional_research_detail(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<JgdyDetail>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:jgdy-detail:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_jgdy_detail_em(date).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // ESG Ratings
    // -----------------------------------------------------------------------

    pub async fn fetch_esg_msci(&self) -> anyhow::Result<Vec<EsgRating>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:esg-msci");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_esg_msci_sina().await?;
        self.cache_set_json(&cache_key, ESG_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_esg_rft(&self) -> anyhow::Result<Vec<EsgRating>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:esg-rft");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_esg_rft_sina().await?;
        self.cache_set_json(&cache_key, ESG_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_esg_zd(&self) -> anyhow::Result<Vec<EsgRating>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:esg-zd");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_esg_zd_sina().await?;
        self.cache_set_json(&cache_key, ESG_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_esg_hz(&self) -> anyhow::Result<Vec<EsgRating>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:esg-hz");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_esg_hz_sina().await?;
        self.cache_set_json(&cache_key, ESG_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Financial Reports (三大报表)
    // -----------------------------------------------------------------------

    pub async fn fetch_balance_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<BalanceSheet>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:balance-sheet:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_zcfz_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_profit_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<ProfitSheet>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:profit-sheet:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_lrb_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_cash_flow_sheet(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<CashFlowSheet>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:cash-flow-sheet:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_xjll_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Stock Comments (千股千评)
    // -----------------------------------------------------------------------

    pub async fn fetch_stock_comments(&self) -> anyhow::Result<Vec<StockComment>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:stock-comments");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_comment_em().await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_comment_org_participation(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentOrgParticipation>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:comment-org:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_comment_detail_zlkp_jgcyd_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_comment_hist_score(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentHistScore>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:comment-hist:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_comment_detail_zhpj_lspf_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_comment_focus_index(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentFocusIndex>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:comment-focus:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_comment_detail_scrd_focus_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_comment_desire_index(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<CommentDesireIndex>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:comment-desire:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_comment_detail_scrd_desire_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Changes (高管持股变动)
    // -----------------------------------------------------------------------

    pub async fn fetch_executive_shareholding(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<Ggcg>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:exec-shareholding:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_ggcg_em(symbol).await?;
        self.cache_set_json(&cache_key, INSIDER_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Shareholder Count (股东户数)
    // -----------------------------------------------------------------------

    pub async fn fetch_shareholder_count(
        &self,
        date: &str,
    ) -> anyhow::Result<Vec<Gdhs>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:gdhs:{}", date);
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdhs_em(date).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    pub async fn fetch_shareholder_count_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<GdhsDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:gdhs-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_gdhs_detail_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items).await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Industry Classification (行业分类)
    // -----------------------------------------------------------------------

    pub async fn fetch_industry_category(&self) -> anyhow::Result<Vec<IndustryCategory>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:industry-category");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_industry_category_cninfo().await?;
        self.cache_set_json(&cache_key, INDUSTRY_CACHE_TTL_SECS, &items).await;
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
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:hk-spot");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_spot().await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Famous Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_famous_spot(&self) -> anyhow::Result<Vec<HkFamousStock>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:hk-famous-spot");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_famous_spot_em().await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Hot Rank (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_hot_rank(&self) -> anyhow::Result<Vec<HkHotRank>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:hk-hot-rank");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_hot_rank_em().await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_latest(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-hot-rank-latest:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_hot_rank_latest_em(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-hot-rank-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_hot_rank_detail_em(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_hk_hot_rank_realtime(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkHotRankDetail>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-hot-rank-rt:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_hot_rank_detail_realtime_em(symbol).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Dividends (Eastmoney + THS)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_dividend_payout(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-dividend-payout:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_dividend_payout_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_hk_fhpx_detail(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<HkFhpxDetailThs>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-fhpx-detail:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_fhpx_detail_ths(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    pub async fn fetch_hk_dividend_yield(&self) -> anyhow::Result<Vec<HkGxlLg>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:hk-dividend-yield");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_hk_gxl_lg().await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // HK Financial Indicators (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_hk_financial_indicators(
        &self,
        symbol: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-fin-indicators:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items =
            self.ak.stock_hk_financial_indicator_em(symbol).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:hk-valuation:{}:{}:{}",
            symbol.trim(),
            indicator,
            period
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items =
            self.ak.stock_hk_valuation_baidu(symbol, indicator, period).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Spot (Sina / Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_spot(&self) -> anyhow::Result<Vec<UsSpotSina>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:us-spot");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_us_spot().await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Famous Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_famous_spot(
        &self,
        category: &str,
    ) -> anyhow::Result<Vec<UsFamousStock>> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:us-famous-spot:{}",
            category.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_us_famous_spot_em(category).await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // US Pink Sheet Stocks (Eastmoney)
    // -----------------------------------------------------------------------

    pub async fn fetch_us_pink_spot(&self) -> anyhow::Result<Vec<UsPinkStock>> {
        let cache_key = format!("{MARKET_DATA_CACHE_PREFIX}:us-pink-spot");
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items = self.ak.stock_us_pink_spot_em().await?;
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
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
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:us-valuation:{}:{}:{}",
            symbol.trim(),
            indicator,
            period
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        let items =
            self.ak.stock_us_valuation_baidu(symbol, indicator, period).await?;
        self.cache_set_json(&cache_key, FUNDAMENTALS_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }

    // -----------------------------------------------------------------------
    // Xueqiu Spot (works for HK and US symbols)
    // -----------------------------------------------------------------------

    pub async fn fetch_xq_spot(&self, symbol: &str) -> anyhow::Result<XqStockSpot> {
        let cache_key = format!(
            "{MARKET_DATA_CACHE_PREFIX}:xq-spot:{}",
            symbol.trim()
        );
        if let Some(cached) = self.cache_get_json(&cache_key).await {
            return Ok(cached);
        }
        // Try HK first, then US — both delegate to the same Xueqiu endpoint
        let market = self.detect_market(symbol);
        let items = match market {
            crate::types::MarketKind::HongKong => {
                self.ak.stock_individual_spot_xq(symbol).await?
            }
            _ => self.ak.stock_individual_spot_xq(symbol).await?,
        };
        self.cache_set_json(&cache_key, CANDLES_CACHE_TTL_SECS, &items)
            .await;
        Ok(items)
    }
}
