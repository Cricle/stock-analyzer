use super::cache::DataCacheLayer;
use super::validator::DataValidator;
use std::future::Future;
use std::time::Duration;

/// Configuration for the data pipeline.
#[derive(Debug, Clone)]
pub struct DataPipelineConfig {
    /// Timeout for quote fetching in milliseconds
    pub quote_timeout_ms: u64,
    /// Timeout for fundamentals fetching in milliseconds
    pub fundamentals_timeout_ms: u64,
    /// Timeout for news fetching in milliseconds
    pub news_timeout_ms: u64,
    /// Timeout for candles fetching in milliseconds
    pub candles_timeout_ms: u64,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay for exponential backoff in milliseconds
    pub retry_base_delay_ms: u64,
    /// Whether caching is enabled
    pub cache_enabled: bool,
    /// Cache time-to-live in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum number of entries in cache
    pub cache_max_size: usize,
}

impl Default for DataPipelineConfig {
    fn default() -> Self {
        Self {
            quote_timeout_ms: 15000,
            fundamentals_timeout_ms: 15000,
            news_timeout_ms: 25000,
            candles_timeout_ms: 15000,
            max_retries: 2,
            retry_base_delay_ms: 1000,
            cache_enabled: true,
            cache_ttl_seconds: 300,
            cache_max_size: 1000,
        }
    }
}

impl DataPipelineConfig {
    /// Get timeout for a specific data type
    pub fn timeout_ms(&self, data_type: &str) -> u64 {
        match data_type {
            "quote" => self.quote_timeout_ms,
            "fundamentals" => self.fundamentals_timeout_ms,
            "news" => self.news_timeout_ms,
            "candles" => self.candles_timeout_ms,
            _ => self.quote_timeout_ms, // default
        }
    }
}

/// Parallel data fetcher with retry and caching.
pub struct ParallelExecutor {
    config: DataPipelineConfig,
    cache: DataCacheLayer,
    validator: DataValidator,
}

impl ParallelExecutor {
    /// Create a new ParallelExecutor.
    pub fn new(
        config: DataPipelineConfig,
        cache: DataCacheLayer,
        validator: DataValidator,
    ) -> Self {
        Self {
            config,
            cache,
            validator,
        }
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &DataPipelineConfig {
        &self.config
    }

    /// Get a reference to the validator.
    pub fn validator(&self) -> &DataValidator {
        &self.validator
    }

    /// Fetch data with retry logic.
    pub async fn fetch_with_retry<T, F, Fut>(&self, data_type: &str, fetcher: F) -> Option<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let timeout_ms = self.config.timeout_ms(data_type);
        for attempt in 0..=self.config.max_retries {
            match tokio::time::timeout(Duration::from_millis(timeout_ms), fetcher()).await {
                Ok(Ok(data)) => return Some(data),
                Ok(Err(e)) => tracing::warn!("{} attempt {} failed: {}", data_type, attempt, e),
                Err(_) => tracing::warn!("{} attempt {} timed out", data_type, attempt),
            }
            if attempt < self.config.max_retries {
                let delay =
                    Duration::from_millis(self.config.retry_base_delay_ms * 2u64.pow(attempt));
                tokio::time::sleep(delay).await;
            }
        }
        None
    }

    /// Get cached data or fetch fresh data.
    pub async fn get_or_fetch<T, F, Fut>(
        &self,
        cache_key: &str,
        data_type: &str,
        fetcher: F,
    ) -> Option<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get(cache_key) {
                if let Ok(data) = serde_json::from_value(cached) {
                    return Some(data);
                }
            }
        }
        let data = self.fetch_with_retry(data_type, fetcher).await;
        if let Some(ref data) = data {
            if self.config.cache_enabled {
                if let Ok(json) = serde_json::to_value(data) {
                    self.cache.set_default(cache_key.to_string(), json);
                }
            }
        }
        data
    }
}
