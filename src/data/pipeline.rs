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
