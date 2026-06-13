use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use super::{CandlePoint, MarketDataClient, MarketKind, QuoteSnapshot};

/// Records a single provider attempt during a data fetch with rotation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataFetchAttempt {
    pub provider: String,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Full diagnosis of a data fetch operation across multiple providers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataFetchDiagnosis {
    pub data_type: String,
    pub symbol: String,
    pub attempts: Vec<DataFetchAttempt>,
    pub final_status: String, // "success" or "failed"
}

impl DataFetchDiagnosis {
    pub fn new(data_type: &str, symbol: &str) -> Self {
        Self {
            data_type: data_type.to_string(),
            symbol: symbol.to_string(),
            attempts: Vec::new(),
            final_status: "failed".to_string(),
        }
    }

    /// Returns a summary string for logging.
    pub fn summary(&self) -> String {
        let providers: Vec<String> = self
            .attempts
            .iter()
            .map(|a| {
                if a.success {
                    format!("{}:ok({}ms)", a.provider, a.duration_ms)
                } else {
                    format!("{}:err({}ms)", a.provider, a.duration_ms)
                }
            })
            .collect();
        format!(
            "[{}:{}:{}] {}",
            self.data_type,
            self.symbol,
            self.final_status,
            providers.join(" -> ")
        )
    }
}

/// A named provider that can attempt to fetch data.
#[allow(clippy::type_complexity)]
pub struct NamedProvider<T> {
    pub name: String,
    pub fetcher: Box<dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>> + Send + Sync>,
}

impl<T> NamedProvider<T> {
    pub fn new<F, Fut>(name: impl Into<String>, fetcher: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let name = name.into();
        Self {
            name,
            fetcher: Box::new(move || Box::pin(fetcher())),
        }
    }
}

impl super::MarketDataClient {
    /// Check if fetched data is effectively empty (empty vec, empty string, etc.)
    fn is_data_empty<T: serde::Serialize>(data: &T) -> bool {
        match serde_json::to_value(data) {
            Ok(serde_json::Value::Array(arr)) => arr.is_empty(),
            Ok(serde_json::Value::String(s)) => s.is_empty(),
            Ok(serde_json::Value::Null) => true,
            _ => false,
        }
    }

    /// Try fetching data from multiple providers in order.
    /// Returns the first successful result, or the last failure.
    /// On success, caches the result. On all-failure, tries stale cache.
    pub async fn fetch_with_rotation<T>(
        &self,
        symbol: &str,
        data_type: &str,
        providers: &[NamedProvider<T>],
        cache_key: &str,
        cache_ttl_secs: u64,
    ) -> (Option<T>, DataFetchDiagnosis)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Clone,
    {
        let mut diagnosis = DataFetchDiagnosis::new(data_type, symbol);

        // First, check fresh cache
        if let Some(cached) = self.cache_get_json::<T>(cache_key).await {
            diagnosis.final_status = "success".to_string();
            diagnosis.attempts.push(DataFetchAttempt {
                provider: "redis_cache".to_string(),
                success: true,
                error: None,
                duration_ms: 0,
            });
            return (Some(cached), diagnosis);
        }

        // Try each provider in order
        for provider in providers {
            let start = std::time::Instant::now();
            let result = (provider.fetcher)().await;
            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(data) => {
                    // Check for empty collections — treat as failure to try next provider
                    if Self::is_data_empty(&data) {
                        diagnosis.attempts.push(DataFetchAttempt {
                            provider: provider.name.clone(),
                            success: false,
                            error: Some("provider returned empty data".to_string()),
                            duration_ms,
                        });
                        tracing::warn!(
                            provider = %provider.name,
                            symbol = %symbol,
                            data_type = %data_type,
                            "provider returned empty data, trying next"
                        );
                        continue;
                    }
                    diagnosis.attempts.push(DataFetchAttempt {
                        provider: provider.name.clone(),
                        success: true,
                        error: None,
                        duration_ms,
                    });
                    diagnosis.final_status = "success".to_string();
                    // Cache the successful result
                    self.cache_set_json(cache_key, cache_ttl_secs, &data).await;
                    return (Some(data), diagnosis);
                }
                Err(e) => {
                    diagnosis.attempts.push(DataFetchAttempt {
                        provider: provider.name.clone(),
                        success: false,
                        error: Some(e.to_string()),
                        duration_ms,
                    });
                    tracing::warn!(
                        provider = %provider.name,
                        symbol = %symbol,
                        data_type = %data_type,
                        error = %e,
                        "data fetch failed, trying next provider"
                    );
                }
            }
        }

        diagnosis.final_status = "failed".to_string();
        (None, diagnosis)
    }
}

// ============================================================
// Provider chain definitions
// ============================================================

impl MarketDataClient {
    /// Fetch quote with provider rotation and diagnosis.
    /// A-share: tencent -> eastmoney | HK: tencent -> yahoo | US: sina -> eastmoney -> yahoo -> stooq
    pub async fn fetch_quote_with_rotation(
        &self,
        symbol: &str,
    ) -> (Option<QuoteSnapshot>, DataFetchDiagnosis) {
        let market = self.detect_market(symbol);
        let normalized = self.cache_symbol(symbol, market);
        let cache_key = format!(
            "{}:quote:{}:{}",
            super::MARKET_DATA_CACHE_PREFIX,
            super::QUOTE_CACHE_VERSION,
            normalized
        );
        match market {
            MarketKind::AShare => {
                let symbol_owned = symbol.to_string();
                let providers: Vec<NamedProvider<QuoteSnapshot>> = vec![
                    NamedProvider::new("tencent_quote", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            async move { client.fetch_a_share_quote_from_eastmoney(&sym).await }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "quote",
                    &providers,
                    &cache_key,
                    super::QUOTE_CACHE_TTL_SECS,
                )
                .await
            }
            MarketKind::HongKong => {
                let symbol_owned = symbol.to_string();
                let providers: Vec<NamedProvider<QuoteSnapshot>> = vec![
                    NamedProvider::new("akshare_hk_quote", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            async move {
                                let (quote, _source) = client.fetch_hk_quote(&sym).await?;
                                Ok(quote)
                            }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "quote",
                    &providers,
                    &cache_key,
                    super::QUOTE_CACHE_TTL_SECS,
                )
                .await
            }
            MarketKind::UsEquity => {
                let symbol_owned = symbol.to_string();
                let providers: Vec<NamedProvider<QuoteSnapshot>> = vec![
                    NamedProvider::new("akshare_us", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            async move {
                                let (quote, _source) = client.fetch_us_quote(&sym).await?;
                                Ok(quote)
                            }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "quote",
                    &providers,
                    &cache_key,
                    super::QUOTE_CACHE_TTL_SECS,
                )
                .await
            }
        }
    }

    /// Fetch candles with provider rotation and diagnosis.
    /// A-share: tencent -> eastmoney | HK: tencent -> yahoo | US: sina -> eastmoney -> yahoo -> stooq
    pub async fn fetch_candles_with_rotation(
        &self,
        symbol: &str,
        adjust: &str,
        limit: usize,
    ) -> (Option<Vec<CandlePoint>>, DataFetchDiagnosis) {
        let market = self.detect_market(symbol);
        let normalized = self.cache_symbol(symbol, market);
        let cache_key = format!(
            "{}:candles:{}:{}:{}:{}",
            super::MARKET_DATA_CACHE_PREFIX,
            super::CANDLES_CACHE_VERSION,
            normalized,
            adjust,
            limit
        );
        match market {
            MarketKind::AShare => {
                let symbol_owned = symbol.to_string();
                let adjust_owned = adjust.to_string();
                let providers: Vec<NamedProvider<Vec<CandlePoint>>> = vec![
                    NamedProvider::new("tencent_kline", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        let adj = adjust_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            let adj = adj.clone();
                            async move {
                                client
                                    .fetch_a_share_tencent_candles(&sym, &adj, limit)
                                    .await
                            }
                        }
                    }),
                    NamedProvider::new("eastmoney_kline", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        let adj = adjust_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            let adj = adj.clone();
                            async move {
                                client
                                    .fetch_a_share_eastmoney_candles(&sym, &adj, limit)
                                    .await
                            }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "candles",
                    &providers,
                    &cache_key,
                    super::CANDLES_CACHE_TTL_SECS,
                )
                .await
            }
            MarketKind::HongKong => {
                let symbol_owned = symbol.to_string();
                let providers: Vec<NamedProvider<Vec<CandlePoint>>> = vec![
                    NamedProvider::new("akshare_hk_candles", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            async move {
                                let candles = client.fetch_hk_candles(&sym, limit).await?;
                                Ok(candles)
                            }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "candles",
                    &providers,
                    &cache_key,
                    super::CANDLES_CACHE_TTL_SECS,
                )
                .await
            }
            MarketKind::UsEquity => {
                let symbol_owned = symbol.to_string();
                let providers: Vec<NamedProvider<Vec<CandlePoint>>> = vec![
                    NamedProvider::new("akshare_us", {
                        let client = self.clone();
                        let sym = symbol_owned.clone();
                        move || {
                            let client = client.clone();
                            let sym = sym.clone();
                            async move {
                                let (candles, _source) = client.fetch_us_candles(&sym, limit).await?;
                                Ok(candles)
                            }
                        }
                    }),
                ];
                self.fetch_with_rotation(
                    symbol,
                    "candles",
                    &providers,
                    &cache_key,
                    super::CANDLES_CACHE_TTL_SECS,
                )
                .await
            }
        }
    }
}
