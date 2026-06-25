use sa::data::pipeline::DataPipelineConfig;

#[test]
fn test_default_config() {
    let config = DataPipelineConfig::default();
    assert_eq!(config.quote_timeout_ms, 15000);
    assert_eq!(config.fundamentals_timeout_ms, 15000);
    assert_eq!(config.news_timeout_ms, 25000);
    assert_eq!(config.candles_timeout_ms, 15000);
    assert_eq!(config.max_retries, 2);
    assert_eq!(config.retry_base_delay_ms, 1000);
    assert!(config.cache_enabled);
    assert_eq!(config.cache_ttl_seconds, 300);
    assert_eq!(config.cache_max_size, 1000);
}

#[test]
fn test_timeout_for_data_type() {
    let config = DataPipelineConfig::default();
    assert_eq!(config.timeout_ms("quote"), 15000);
    assert_eq!(config.timeout_ms("fundamentals"), 15000);
    assert_eq!(config.timeout_ms("news"), 25000);
    assert_eq!(config.timeout_ms("candles"), 15000);
    assert_eq!(config.timeout_ms("unknown"), 15000); // default
}
