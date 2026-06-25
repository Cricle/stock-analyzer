# Data Pipeline & LLM Reasoning Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix data fetching failures and optimize LLM reasoning to produce actionable Buy/Sell/Hold recommendations instead of always Hold.

**Architecture:** Refactor data pipeline with parallel fetching, caching, and retry mechanisms. Optimize LLM reasoning with a data-driven decision framework and confidence calibration.

**Tech Stack:** Rust, tokio (async), lru crate, exponential backoff, prompt engineering

---

## File Structure

### New Files
- `src/data/pipeline.rs` — ParallelExecutor, DataCacheLayer, DataValidator, DataPipelineConfig
- `src/data/cache.rs` — LRU cache implementation for data caching
- `src/data/validator.rs` — Data quality validation and scoring
- `tests/data_pipeline_test.rs` — Unit tests for data pipeline
- `tests/llm_decision_test.rs` — Unit tests for LLM decision framework

### Modified Files
- `src/data/mod.rs` — Add module declarations and re-exports
- `src/report/lifecycle/fetch.rs` — Refactor to use new data pipeline
- `src/llm/prompt/prompts.rs` — Update prompt templates with decision framework
- `src/llm/prompt/calibration.rs` — Add confidence calibration logic
- `Cargo.toml` — Add lru dependency

---

## Task 1: Add LRU Cache Dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add lru crate to dependencies**

```toml
# In Cargo.toml [dependencies] section, add:
lru = "0.12"
```

- [ ] **Step 2: Verify dependency resolves**

Run: `cargo check -p sa`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "deps: add lru crate for data caching"
```

---

## Task 2: Create DataPipelineConfig

**Files:**
- Create: `src/data/pipeline.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// tests/data_pipeline_test.rs
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test data_pipeline_test test_default_config`
Expected: FAIL with "module `pipeline` not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// src/data/pipeline.rs

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
```

- [ ] **Step 4: Add module declaration to data/mod.rs**

```rust
// In src/data/mod.rs, add:
pub mod pipeline;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_default_config`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/data/pipeline.rs src/data/mod.rs tests/data_pipeline_test.rs
git commit -m "feat: add DataPipelineConfig with timeout and retry settings"
```

---

## Task 3: Create DataCacheLayer

**Files:**
- Create: `src/data/cache.rs`
- Modify: `src/data/mod.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/data_pipeline_test.rs, add:
use sa::data::cache::DataCacheLayer;
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test data_pipeline_test test_cache_set_and_get`
Expected: FAIL with "module `cache` not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// src/data/cache.rs

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// LRU cache for market data with TTL support.
pub struct DataCacheLayer {
    cache: Mutex<lru::LruCache<String, CacheEntry>>,
    default_ttl: Duration,
}

struct CacheEntry {
    data: serde_json::Value,
    cached_at: Instant,
    ttl: Duration,
}

impl DataCacheLayer {
    /// Create a new cache with the given max size and default TTL.
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(max_size.max(1)).unwrap(),
            )),
            default_ttl,
        }
    }

    /// Get a value from the cache. Returns None if missing or expired.
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut cache = self.cache.lock().unwrap();
        cache.get(key).and_then(|entry| {
            if entry.cached_at.elapsed() < entry.ttl {
                Some(entry.data.clone())
            } else {
                cache.pop(key);
                None
            }
        })
    }

    /// Set a value in the cache with a specific TTL.
    pub fn set(&self, key: String, data: serde_json::Value, ttl: Duration) {
        let mut cache = self.cache.lock().unwrap();
        cache.put(
            key,
            CacheEntry {
                data,
                cached_at: Instant::now(),
                ttl,
            },
        );
    }

    /// Set a value in the cache with the default TTL.
    pub fn set_default(&self, key: String, data: serde_json::Value) {
        self.set(key, data, self.default_ttl);
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.lock().unwrap();
        cache.is_empty()
    }
}
```

- [ ] **Step 4: Add module declaration to data/mod.rs**

```rust
// In src/data/mod.rs, add:
pub mod cache;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_cache`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/data/cache.rs src/data/mod.rs
git commit -m "feat: add DataCacheLayer with LRU cache and TTL support"
```

---

## Task 4: Create DataValidator

**Files:**
- Create: `src/data/validator.rs`
- Modify: `src/data/mod.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/data_pipeline_test.rs, add:
use sa::data::validator::{DataValidator, DataQualityReport};
use sa::data::FundamentalsSnapshot;

#[test]
fn test_validate_with_complete_data() {
    let validator = DataValidator;
    let fundamentals = FundamentalsSnapshot {
        pe_like: Some(25.0),
        pb: Some(5.0),
        roe: Some(0.15),
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test data_pipeline_test test_validate_with_complete_data`
Expected: FAIL with "module `validator` not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// src/data/validator.rs

use super::FundamentalsSnapshot;

/// Data quality report for a single data source.
#[derive(Debug, Clone)]
pub struct DataQualityReport {
    /// Quality score (0.0 - 100.0)
    pub score: f64,
    /// List of missing fields
    pub missing_fields: Vec<String>,
    /// List of warnings
    pub warnings: Vec<String>,
}

impl DataQualityReport {
    pub fn new() -> Self {
        Self {
            score: 0.0,
            missing_fields: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Validates data quality and completeness.
pub struct DataValidator;

impl DataValidator {
    /// Validate fundamentals data quality.
    pub fn validate_fundamentals(&self, fundamentals: &FundamentalsSnapshot) -> DataQualityReport {
        let mut report = DataQualityReport::new();
        let mut score = 0.0;
        let mut count = 0;

        if fundamentals.pe_like.is_some() {
            score += 20.0;
            count += 1;
        } else {
            report.missing_fields.push("pe_like".to_string());
        }

        if fundamentals.pb.is_some() {
            score += 20.0;
            count += 1;
        } else {
            report.missing_fields.push("pb".to_string());
        }

        if fundamentals.roe.is_some() {
            score += 20.0;
            count += 1;
        } else {
            report.missing_fields.push("roe".to_string());
        }

        if fundamentals.revenues_usd.is_some() {
            score += 20.0;
            count += 1;
        } else {
            report.missing_fields.push("revenues_usd".to_string());
        }

        if fundamentals.net_income_usd.is_some() {
            score += 20.0;
            count += 1;
        } else {
            report.missing_fields.push("net_income_usd".to_string());
        }

        report.score = score;
        report
    }

    /// Calculate overall data quality score.
    pub fn overall_score(
        &self,
        quote_score: f64,
        fundamentals_score: f64,
        news_score: f64,
        candles_score: f64,
    ) -> f64 {
        (quote_score * 0.3 + fundamentals_score * 0.3 + news_score * 0.2 + candles_score * 0.2)
            .clamp(0.0, 100.0)
    }
}
```

- [ ] **Step 4: Add module declaration to data/mod.rs**

```rust
// In src/data/mod.rs, add:
pub mod validator;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_validate`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/data/validator.rs src/data/mod.rs
git commit -m "feat: add DataValidator for data quality scoring"
```

---

## Task 5: Create ParallelExecutor

**Files:**
- Modify: `src/data/pipeline.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/data_pipeline_test.rs, add:
use sa::data::pipeline::ParallelExecutor;
use sa::data::cache::DataCacheLayer;
use sa::data::validator::DataValidator;
use std::time::Duration;

#[tokio::test]
async fn test_parallel_executor_creation() {
    let config = sa::data::pipeline::DataPipelineConfig::default();
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let validator = DataValidator;
    
    let executor = ParallelExecutor::new(config, cache, validator);
    assert_eq!(executor.config().max_retries, 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test data_pipeline_test test_parallel_executor_creation`
Expected: FAIL with "ParallelExecutor not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// In src/data/pipeline.rs, add:

use super::cache::DataCacheLayer;
use super::validator::DataValidator;
use std::time::Duration;
use std::future::Future;

/// Parallel data fetcher with retry and caching.
pub struct ParallelExecutor {
    config: DataPipelineConfig,
    cache: DataCacheLayer,
    validator: DataValidator,
}

impl ParallelExecutor {
    /// Create a new ParallelExecutor.
    pub fn new(config: DataPipelineConfig, cache: DataCacheLayer, validator: DataValidator) -> Self {
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

    /// Fetch data with retry logic.
    pub async fn fetch_with_retry<T, F, Fut>(
        &self,
        data_type: &str,
        fetcher: F,
    ) -> Option<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let timeout_ms = self.config.timeout_ms(data_type);

        for attempt in 0..=self.config.max_retries {
            match tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                fetcher(),
            )
            .await
            {
                Ok(Ok(data)) => return Some(data),
                Ok(Err(e)) => {
                    tracing::warn!("{} attempt {} failed: {}", data_type, attempt, e);
                }
                Err(_) => {
                    tracing::warn!("{} attempt {} timed out", data_type, attempt);
                }
            }

            if attempt < self.config.max_retries {
                let delay = Duration::from_millis(
                    self.config.retry_base_delay_ms * 2u64.pow(attempt),
                );
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
        // Try cache first
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get(cache_key) {
                if let Ok(data) = serde_json::from_value(cached) {
                    return Some(data);
                }
            }
        }

        // Fetch fresh data
        let data = self.fetch_with_retry(data_type, fetcher).await;

        // Cache the result
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_parallel_executor_creation`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/data/pipeline.rs
git commit -m "feat: add ParallelExecutor with retry and caching"
```

---

## Task 6: Update LLM Prompt Templates

**Files:**
- Modify: `src/llm/prompt/prompts.rs`
- Test: `tests/llm_decision_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// tests/llm_decision_test.rs
use sa::llm::prompt::build_decision_framework_prompt;

#[test]
fn test_decision_framework_prompt_with_high_completeness() {
    let prompt = build_decision_framework_prompt(85.0, 90.0, 70.0, 60.0);
    assert!(prompt.contains("Data Completeness"));
    assert!(prompt.contains("85.0%"));
    assert!(prompt.contains("must give clear directional judgment"));
}

#[test]
fn test_decision_framework_prompt_with_low_completeness() {
    let prompt = build_decision_framework_prompt(30.0, 40.0, 20.0, 10.0);
    assert!(prompt.contains("30.0%"));
    assert!(prompt.contains("give Hold and explain missing data"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test llm_decision_test test_decision_framework_prompt`
Expected: FAIL with "build_decision_framework_prompt not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// In src/llm/prompt/prompts.rs, add:

/// Build a decision framework prompt based on data completeness.
pub fn build_decision_framework_prompt(
    technical_completeness: f64,
    fundamental_completeness: f64,
    news_completeness: f64,
    sentiment_completeness: f64,
) -> String {
    let overall = (technical_completeness * 0.3
        + fundamental_completeness * 0.3
        + news_completeness * 0.2
        + sentiment_completeness * 0.2)
        .clamp(0.0, 100.0);

    let decision_rule = if overall < 60.0 {
        "If data completeness < 60%, give Hold and explain missing data"
    } else {
        "If data completeness >= 60%, must give clear directional judgment"
    };

    format!(
        r#"## Decision Framework

You are a professional stock analyst. Make investment decisions based on the following data:

### Data Completeness
- Technical data: {technical:.1}%
- Fundamental data: {fundamental:.1}%
- News data: {news:.1}%
- Sentiment data: {sentiment:.1}%
- Overall: {overall:.1}%

### Decision Rules
1. {decision_rule}
2. Use this decision matrix:
   - Technical bullish + Fundamentals healthy → Buy
   - Technical bearish + Fundamentals deteriorating → Sell
   - Contradictory signals or insufficient data → Hold

### Output Requirements
1. Must give clear Buy/Sell/Hold recommendation
2. Must give confidence score (0-100)
3. Must list supporting and opposing evidence
4. Must explain impact of missing data on decision"#,
        technical = technical_completeness,
        fundamental = fundamental_completeness,
        news = news_completeness,
        sentiment = sentiment_completeness,
        overall = overall,
        decision_rule = decision_rule,
    )
}
```

- [ ] **Step 4: Add function to prompt module exports**

```rust
// In src/llm/prompt/mod.rs, ensure the function is accessible
// The include! macro should handle this automatically
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p sa --test llm_decision_test test_decision_framework_prompt`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/llm/prompt/prompts.rs tests/llm_decision_test.rs
git commit -m "feat: add decision framework prompt for data-driven decisions"
```

---

## Task 7: Add Confidence Calibration

**Files:**
- Modify: `src/llm/prompt/calibration.rs`
- Test: `tests/llm_decision_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/llm_decision_test.rs, add:
use sa::llm::prompt::ConfidenceCalibration;

#[test]
fn test_confidence_calibration_high_data() {
    let cal = ConfidenceCalibration::new(70.0);
    let result = cal.calibrate(0.9, 0.8, 0.7);
    assert!(result > 50.0);
    assert!(result <= 100.0);
}

#[test]
fn test_confidence_calibration_low_data() {
    let cal = ConfidenceCalibration::new(70.0);
    let result = cal.calibrate(0.3, 0.4, 0.5);
    assert!(result < 50.0);
}

#[test]
fn test_confidence_calibration_clamping() {
    let cal = ConfidenceCalibration::new(90.0);
    let result = cal.calibrate(1.5, 1.5, 1.5);
    assert!(result <= 100.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test llm_decision_test test_confidence_calibration`
Expected: FAIL with "ConfidenceCalibration not found"

- [ ] **Step 3: Write minimal implementation**

```rust
// In src/llm/prompt/calibration.rs, add:

/// Confidence calibration based on data completeness and signal consistency.
#[derive(Debug, Clone)]
pub struct ConfidenceCalibration {
    /// Base confidence score
    pub base_confidence: f64,
    /// Factor for data completeness (multiplier)
    pub data_completeness_factor: f64,
    /// Factor for signal consistency (multiplier)
    pub signal_consistency_factor: f64,
    /// Factor for historical accuracy (multiplier)
    pub historical_accuracy_factor: f64,
}

impl ConfidenceCalibration {
    /// Create a new calibration with the given base confidence.
    pub fn new(base_confidence: f64) -> Self {
        Self {
            base_confidence: base_confidence.clamp(0.0, 100.0),
            data_completeness_factor: 1.0,
            signal_consistency_factor: 1.0,
            historical_accuracy_factor: 1.0,
        }
    }

    /// Calibrate confidence based on input factors.
    ///
    /// - `data_completeness`: 0.0 to 1.0 (percentage of available data)
    /// - `signal_consistency`: 0.0 to 1.0 (how consistent are the signals)
    /// - `historical_accuracy`: 0.0 to 1.0 (historical accuracy of similar setups)
    ///
    /// Returns a confidence score clamped to 0.0-100.0.
    pub fn calibrate(
        &self,
        data_completeness: f64,
        signal_consistency: f64,
        historical_accuracy: f64,
    ) -> f64 {
        let adjusted = self.base_confidence
            * (1.0 + (data_completeness - 0.5) * self.data_completeness_factor)
            * (1.0 + (signal_consistency - 0.5) * self.signal_consistency_factor)
            * (1.0 + (historical_accuracy - 0.5) * self.historical_accuracy_factor);

        adjusted.clamp(0.0, 100.0)
    }
}

impl Default for ConfidenceCalibration {
    fn default() -> Self {
        Self::new(50.0)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sa --test llm_decision_test test_confidence_calibration`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/llm/prompt/calibration.rs tests/llm_decision_test.rs
git commit -m "feat: add ConfidenceCalibration for data-driven confidence scoring"
```

---

## Task 8: Integrate Data Pipeline into Report Lifecycle

**Files:**
- Modify: `src/report/lifecycle/fetch.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/data_pipeline_test.rs, add:
use sa::data::pipeline::{DataPipelineConfig, ParallelExecutor};
use sa::data::cache::DataCacheLayer;
use sa::data::validator::DataValidator;
use std::time::Duration;

#[tokio::test]
async fn test_fetch_with_retry_success() {
    let config = DataPipelineConfig::default();
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let validator = DataValidator;
    let executor = ParallelExecutor::new(config, cache, validator);

    let result = executor
        .fetch_with_retry("test", || async { Ok(42) })
        .await;
    assert_eq!(result, Some(42));
}

#[tokio::test]
async fn test_fetch_with_retry_failure() {
    let config = DataPipelineConfig {
        max_retries: 1,
        retry_base_delay_ms: 10,
        ..Default::default()
    };
    let cache = DataCacheLayer::new(100, Duration::from_secs(300));
    let validator = DataValidator;
    let executor = ParallelExecutor::new(config, cache, validator);

    let result = executor
        .fetch_with_retry("test", || async {
            Err(anyhow::anyhow!("test error"))
        })
        .await;
    assert!(result.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sa --test data_pipeline_test test_fetch_with_retry_success`
Expected: FAIL (compilation error due to missing integration)

- [ ] **Step 3: Integrate pipeline into fetch.rs**

```rust
// In src/report/lifecycle/fetch.rs, add helper method:

impl TaskManager {
    /// Create a ParallelExecutor for data fetching.
    pub(super) fn create_data_executor(&self) -> crate::data::pipeline::ParallelExecutor {
        let config = crate::data::pipeline::DataPipelineConfig::default();
        let cache = crate::data::cache::DataCacheLayer::new(
            config.cache_max_size,
            std::time::Duration::from_secs(config.cache_ttl_seconds),
        );
        let validator = crate::data::validator::DataValidator;
        crate::data::pipeline::ParallelExecutor::new(config, cache, validator)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_fetch_with_retry`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/report/lifecycle/fetch.rs
git commit -m "feat: integrate ParallelExecutor into report lifecycle"
```

---

## Task 9: Refactor fetch_core_market_data to Use Pipeline

**Files:**
- Modify: `src/report/lifecycle/fetch.rs`

- [ ] **Step 1: Refactor fetch_core_market_data**

```rust
// In src/report/lifecycle/fetch.rs, update fetch_core_market_data:

impl TaskManager {
    pub(super) async fn fetch_core_market_data(
        &self,
        task: &PersistedTask,
        news_start: Option<String>,
    ) -> CoreMarketData {
        let executor = self.create_data_executor();
        let candle_limit = std::env::var("REPORT_KLINE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(3000)
            .clamp(1, 5000);

        let symbol = task.symbol.clone();
        let analysis_date = task.analysis_date.clone();
        let market_data = self.market_data.clone();

        // Parallel fetch with retry
        let (quote_result, fundamentals, news_items, candles_result) = tokio::join!(
            executor.fetch_with_retry("quote", {
                let md = market_data.clone();
                let sym = symbol.clone();
                move || {
                    let md = md.clone();
                    let sym = sym.clone();
                    async move { md.fetch_quote_with_rotation(&sym).await }
                }
            }),
            executor.fetch_with_retry("fundamentals", {
                let md = market_data.clone();
                let sym = symbol.clone();
                move || {
                    let md = md.clone();
                    let sym = sym.clone();
                    async move { md.fetch_fundamentals(&sym).await }
                }
            }),
            executor.fetch_with_retry("news", {
                let md = market_data.clone();
                let sym = symbol.clone();
                let ns = news_start.clone();
                let ad = analysis_date.clone();
                move || {
                    let md = md.clone();
                    let sym = sym.clone();
                    let ns = ns.clone();
                    let ad = ad.clone();
                    async move { md.fetch_news(&sym, 15, ns.as_deref(), Some(&ad)).await }
                }
            }),
            executor.fetch_with_retry("candles", {
                let md = market_data.clone();
                let sym = symbol.clone();
                move || {
                    let md = md.clone();
                    let sym = sym.clone();
                    async move { md.fetch_candles_with_rotation(&sym, "qfq", candle_limit).await }
                }
            }),
        );

        // Process results (same as before, but now with retry)
        let (quote, quote_diagnosis) = match quote_result {
            Some(result) => result,
            None => {
                tracing::warn!("quote fetch failed for {} after retries", symbol);
                (None, crate::data::DataFetchDiagnosis::new("quote", &symbol))
            }
        };

        let fundamentals = fundamentals.unwrap_or(None);

        let news_items = news_items.unwrap_or_default();

        let (candles_data, candles_diagnosis) = match candles_result {
            Some(result) => result,
            None => {
                tracing::warn!("candles fetch failed for {} after retries", symbol);
                (None, crate::data::DataFetchDiagnosis::new("candles", &symbol))
            }
        };

        // Build market chart (same as before)
        let market_chart = match candles_data {
            Some(items) => {
                let provider_used = candles_diagnosis
                    .attempts
                    .last()
                    .filter(|a| a.success)
                    .map(|a| a.provider.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                crate::ReportMarketChart {
                    symbol: symbol.clone(),
                    market: task.market_type.clone(),
                    adjust: "qfq".to_string(),
                    source: self.market_data.candles_source(&symbol).to_string(),
                    provider_used,
                    start_date: items.first().map(|item| item.trade_date.clone()).unwrap_or_default(),
                    end_date: items.last().map(|item| item.trade_date.clone()).unwrap_or_default(),
                    candles: items
                        .into_iter()
                        .map(|item| crate::ReportCandle {
                            trade_date: item.trade_date,
                            open: item.open,
                            close: item.close,
                            high: item.high,
                            low: item.low,
                            volume: item.volume,
                            amount: item.amount,
                            amplitude_pct: item.amplitude_pct,
                            change_pct: item.change_pct,
                            change_amount: item.change_amount,
                            turnover_pct: item.turnover_pct,
                        })
                        .collect(),
                    indicators: Vec::new(),
                    overlays: Vec::new(),
                    trend_lines: Vec::new(),
                }
            }
            None => {
                tracing::warn!("candles fetch failed for {}: all providers exhausted", symbol);
                crate::ReportMarketChart::default()
            }
        };

        let mut fetch_diagnosis = Vec::new();
        if !quote_diagnosis.attempts.is_empty() {
            fetch_diagnosis.push(serde_json::to_value(&quote_diagnosis).unwrap_or_default());
        }
        if !candles_diagnosis.attempts.is_empty() {
            fetch_diagnosis.push(serde_json::to_value(&candles_diagnosis).unwrap_or_default());
        }

        CoreMarketData {
            quote,
            fundamentals,
            news_items,
            market_chart,
            fetch_diagnosis,
        }
    }
}
```

- [ ] **Step 2: Run existing tests**

Run: `cargo test -p sa`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/report/lifecycle/fetch.rs
git commit -m "refactor: use ParallelExecutor in fetch_core_market_data"
```

---

## Task 10: Add Data Quality Reporting to Results

**Files:**
- Modify: `src/report/result/stages.rs`
- Test: `tests/data_pipeline_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In tests/data_pipeline_test.rs, add:
use sa::data::validator::{DataValidator, DataQualityReport};

#[test]
fn test_overall_score_calculation() {
    let validator = DataValidator;
    let score = validator.overall_score(80.0, 60.0, 90.0, 70.0);
    // 80*0.3 + 60*0.3 + 90*0.2 + 70*0.2 = 24 + 18 + 18 + 14 = 74
    assert!((score - 74.0).abs() < 0.1);
}

#[test]
fn test_overall_score_clamping() {
    let validator = DataValidator;
    let score = validator.overall_score(100.0, 100.0, 100.0, 100.0);
    assert_eq!(score, 100.0);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p sa --test data_pipeline_test test_overall_score`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/data_pipeline_test.rs
git commit -m "test: add data quality scoring tests"
```

---

## Task 11: Update Report to Include Data Quality Metrics

**Files:**
- Modify: `src/report/lifecycle/task_run.rs`

- [ ] **Step 1: Add data quality tracking**

```rust
// In src/report/lifecycle/task_run.rs, after fetch_core_market_data:

// Calculate data quality scores
let validator = crate::data::validator::DataValidator;
let fundamentals_score = data.fundamentals.as_ref()
    .map(|f| validator.validate_fundamentals(f).score)
    .unwrap_or(0.0);

let quote_score = if data.quote.is_some() { 100.0 } else { 0.0 };
let news_score = if data.news_items.is_empty() { 0.0 } else { 80.0 };
let candles_score = if data.market_chart.candles.is_empty() { 0.0 } else { 90.0 };

let overall_quality = validator.overall_score(
    quote_score,
    fundamentals_score,
    news_score,
    candles_score,
);

tracing::info!(
    task_id = %task.task_id,
    symbol = %task.symbol,
    data_quality = overall_quality,
    fundamentals_score = fundamentals_score,
    "data quality assessment"
);
```

- [ ] **Step 2: Run existing tests**

Run: `cargo test -p sa`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/report/lifecycle/task_run.rs
git commit -m "feat: add data quality tracking to report lifecycle"
```

---

## Task 12: Run Full Test Suite

- [ ] **Step 1: Run all tests**

Run: `cargo test -p sa`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p sa`
Expected: No warnings

- [ ] **Step 3: Run fmt**

Run: `cargo fmt -p sa --check`
Expected: All files formatted

- [ ] **Step 4: Run coverage**

Run: `cargo llvm-cov -p sa --fail-under-lines 40`
Expected: Coverage >= 40%

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: final cleanup and test verification"
```

---

## Spec Coverage Checklist

- [x] Data Pipeline Refactoring — Tasks 1-5, 8-9
- [x] ParallelExecutor with retry — Task 5, 8-9
- [x] DataCacheLayer with LRU — Task 3
- [x] DataValidator with quality scoring — Task 4, 10-11
- [x] DataPipelineConfig — Task 2
- [x] LLM Decision Framework — Task 6
- [x] Confidence Calibration — Task 7
- [x] Prompt optimization — Task 6
- [x] Testing strategy — Tasks 2-7, 10, 12
- [x] Success criteria verification — Task 12
