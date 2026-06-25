use sa::data::cache::DataCacheLayer;
use sa::data::pipeline::{DataPipelineConfig, ParallelExecutor};
use sa::data::validator::{DataQualityReport, DataValidator};
use sa::data::FundamentalsSnapshot;
use std::time::Duration;

#[test]
fn test_cache_set_and_get() {
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let data = serde_json::json!({"price": 100.0});
    cache.set("AAPL_quote".to_string(), data.clone(), Duration::from_secs(60));

    let cached = cache.get("AAPL_quote");
    assert!(cached.is_some());
    assert_eq!(cached.unwrap(), data);
}

#[test]
fn test_cache_miss() {
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let cached = cache.get("NONEXISTENT");
    assert!(cached.is_none());
}

#[test]
fn test_cache_expiry() {
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let data = serde_json::json!({"price": 100.0});
    cache.set("AAPL_quote".to_string(), data.clone(), Duration::from_millis(1));

    // Wait for expiry
    std::thread::sleep(Duration::from_millis(10));

    let cached = cache.get("AAPL_quote");
    assert!(cached.is_none());
}

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

#[test]
fn test_validate_with_complete_data() {
    let validator = DataValidator;
    let fundamentals = FundamentalsSnapshot {
        market_cap: Some(1_000_000.0),
        revenues_usd: Some(500_000.0),
        net_income_usd: Some(100_000.0),
        gross_profit_usd: Some(200_000.0),
        operating_income_usd: Some(150_000.0),
        ..Default::default()
    };

    let report = validator.validate_fundamentals(&fundamentals);
    assert!(report.score > 0.0);
    assert!(report.missing_fields.is_empty());
}

#[test]
fn test_validate_with_missing_data() {
    let validator = DataValidator;
    let fundamentals = FundamentalsSnapshot::default();

    let report = validator.validate_fundamentals(&fundamentals);
    assert_eq!(report.score, 0.0);
    assert!(!report.missing_fields.is_empty());
}

#[test]
fn test_overall_score_weighted_average() {
    let validator = DataValidator;
    let score = validator.overall_score(100.0, 100.0, 100.0, 100.0);
    assert!((score - 100.0).abs() < f64::EPSILON);

    let score_zero = validator.overall_score(0.0, 0.0, 0.0, 0.0);
    assert!((score_zero - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_overall_score_clamped() {
    let validator = DataValidator;
    // Even with huge inputs, score should clamp to 100
    let score = validator.overall_score(200.0, 200.0, 200.0, 200.0);
    assert!((score - 100.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_parallel_executor_creation() {
    let config = DataPipelineConfig::default();
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let validator = DataValidator;

    let executor = ParallelExecutor::new(config, cache, validator);
    assert_eq!(executor.config().max_retries, 2);
}
