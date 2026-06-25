# Data Pipeline & LLM Reasoning Optimization Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix data fetching failures and optimize LLM reasoning to produce actionable Buy/Sell/Hold recommendations instead of always Hold.

**Architecture:** Refactor data pipeline with parallel fetching, caching, and retry mechanisms. Optimize LLM reasoning with a data-driven decision framework and confidence calibration.

**Tech Stack:** Rust, tokio (async), LRU cache, exponential backoff, prompt engineering

---

## Problem Statement

Current system issues:
1. **All reports give Hold** — 3 market reports (A股, 港股, 美股) all returned Hold with 38-41/100 execution confidence
2. **Missing fundamental data** — PE/PB/ROE/revenue data marked as "missing" despite API availability
3. **Low execution confidence** — 38-41/100 due to data gaps
4. **Slow execution** — ~18 minutes per report in debug_quick_only mode

Root causes:
- 8-second timeout for data fetching is too short
- No retry mechanism for failed requests
- LLM defaults to Hold when data is incomplete
- No data quality scoring or completeness tracking

---

## Design: Data Pipeline Refactoring

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    DataPipeline                         │
├─────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │  Quote   │  │Fundament.│  │   News   │  │  Candles │ │
│  │ Fetcher  │  │ Fetcher  │  │ Fetcher  │  │ Fetcher  │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘ │
│       │             │             │             │       │
│       ▼             ▼             ▼             ▼       │
│  ┌─────────────────────────────────────────────────────┐ │
│  │              ParallelExecutor                       │ │
│  │  - Parallel fetching for all data sources           │ │
│  │  - Timeout control + retry mechanism                │ │
│  │  - Data source fallback strategy                    │ │
│  └─────────────────────────────────────────────────────┘ │
│       │                                                  │
│       ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐ │
│  │              DataCacheLayer                         │ │
│  │  - In-memory cache (LRU)                           │ │
│  │  - Optional file cache (persistent)                │ │
│  │  - Cache invalidation strategy                     │ │
│  └─────────────────────────────────────────────────────┘ │
│       │                                                  │
│       ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐ │
│  │              DataValidator                          │ │
│  │  - Data completeness check                         │ │
│  │  - Missing data marking                            │ │
│  │  - Quality scoring (0-100)                         │ │
│  └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. ParallelExecutor

```rust
struct ParallelExecutor {
    config: DataPipelineConfig,
    cache: DataCacheLayer,
    validator: DataValidator,
}

impl ParallelExecutor {
    async fn fetch_all(&self, symbol: &str, market: &str) -> DataBundle {
        let (quote, fundamentals, news, candles, enrichment) = tokio::join!(
            self.fetch_with_retry("quote", || self.fetch_quote(symbol)),
            self.fetch_with_retry("fundamentals", || self.fetch_fundamentals(symbol)),
            self.fetch_with_retry("news", || self.fetch_news(symbol)),
            self.fetch_with_retry("candles", || self.fetch_candles(symbol)),
            self.fetch_with_retry("enrichment", || self.fetch_enrichment(symbol)),
        );
        
        DataBundle { quote, fundamentals, news, candles, enrichment }
    }
    
    async fn fetch_with_retry<T, F, Fut>(&self, data_type: &str, fetcher: F) -> Option<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        for attempt in 0..=self.config.max_retries {
            match tokio::time::timeout(
                Duration::from_millis(self.config.timeout_ms(data_type)),
                fetcher(),
            ).await {
                Ok(Ok(data)) => return Some(data),
                Ok(Err(e)) => tracing::warn!("{} attempt {} failed: {}", data_type, attempt, e),
                Err(_) => tracing::warn!("{} attempt {} timed out", data_type, attempt),
            }
            if attempt < self.config.max_retries {
                let delay = Duration::from_millis(
                    self.config.retry_base_delay_ms * 2u64.pow(attempt)
                );
                tokio::time::sleep(delay).await;
            }
        }
        None
    }
}
```

#### 2. DataCacheLayer

```rust
struct DataCacheLayer {
    cache: Mutex<LruCache<String, CacheEntry>>,
    config: CacheConfig,
}

struct CacheEntry {
    data: serde_json::Value,
    cached_at: Instant,
    ttl: Duration,
}

impl DataCacheLayer {
    fn get(&self, key: &str) -> Option<serde_json::Value> {
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
    
    fn set(&self, key: String, data: serde_json::Value, ttl: Duration) {
        let mut cache = self.cache.lock().unwrap();
        cache.put(key, CacheEntry {
            data,
            cached_at: Instant::now(),
            ttl,
        });
    }
}
```

#### 3. DataValidator

```rust
struct DataValidator;

impl DataValidator {
    fn validate(bundle: &DataBundle) -> DataQualityReport {
        let mut report = DataQualityReport::new();
        
        // Check quote completeness
        if let Some(quote) = &bundle.quote {
            report.quote_score = Self::score_quote(quote);
        } else {
            report.add_missing("quote");
        }
        
        // Check fundamentals completeness
        if let Some(fundamentals) = &bundle.fundamentals {
            report.fundamentals_score = Self::score_fundamentals(fundamentals);
        } else {
            report.add_missing("fundamentals");
        }
        
        // Check news completeness
        report.news_score = if bundle.news.is_empty() {
            report.add_missing("news");
            0.0
        } else {
            Self::score_news(&bundle.news)
        };
        
        // Calculate overall score
        report.overall_score = (
            report.quote_score * 0.3 +
            report.fundamentals_score * 0.3 +
            report.news_score * 0.2 +
            report.candles_score * 0.2
        ).clamp(0.0, 100.0);
        
        report
    }
    
    fn score_fundamentals(fundamentals: &FundamentalsSnapshot) -> f64 {
        let mut score = 0.0;
        let mut count = 0;
        
        if fundamentals.pe_like.is_some() { score += 20.0; count += 1; }
        if fundamentals.pb.is_some() { score += 20.0; count += 1; }
        if fundamentals.roe.is_some() { score += 20.0; count += 1; }
        if fundamentals.revenue.is_some() { score += 20.0; count += 1; }
        if fundamentals.net_income.is_some() { score += 20.0; count += 1; }
        
        score
    }
}
```

### Configuration

```rust
struct DataPipelineConfig {
    // Timeout configuration
    quote_timeout_ms: u64,        // Default: 15000
    fundamentals_timeout_ms: u64, // Default: 15000
    news_timeout_ms: u64,         // Default: 25000
    candles_timeout_ms: u64,      // Default: 15000
    
    // Retry configuration
    max_retries: u32,             // Default: 2
    retry_base_delay_ms: u64,     // Default: 1000
    
    // Cache configuration
    cache_enabled: bool,          // Default: true
    cache_ttl_seconds: u64,       // Default: 300 (5 minutes)
    cache_max_size: usize,        // Default: 1000
    
    // Fallback strategy
    fallback_enabled: bool,       // Default: true
    fallback_sources: Vec<String>, // Fallback data source list
}
```

---

## Design: LLM Reasoning Optimization

### Problem Analysis

Current LLM reasoning issues:
1. **Over-conservative when data missing** — Defaults to Hold when fundamental data is missing
2. **Inefficient debate structure** — Bull/bear debate may reach stalemate, resulting in Hold
3. **No confidence calibration** — No adjustment based on data completeness
4. **Unclear decision framework** — LLM doesn't know when to give Buy/Sell

### Solution: Data-Driven Decision Framework

#### 1. Decision Framework

```rust
struct DecisionFramework {
    // Data completeness thresholds
    min_data_completeness: f64,  // Below this, give Hold
    
    // Decision conditions
    buy_conditions: Vec<Condition>,   // Conditions for Buy
    sell_conditions: Vec<Condition>,  // Conditions for Sell
    hold_conditions: Vec<Condition>,  // Default Hold conditions
    
    // Confidence calibration
    confidence_calibration: ConfidenceCalibration,
}

struct Condition {
    name: String,
    weight: f64,
    threshold: f64,
    data_source: String,  // Required data sources
}
```

#### 2. Improved Prompt Template

```
## Decision Framework

You are a professional stock analyst. Make investment decisions based on the following data:

### Data Completeness
- Technical data: {technical_completeness}%
- Fundamental data: {fundamental_completeness}%
- News data: {news_completeness}%
- Sentiment data: {sentiment_completeness}%

### Decision Rules
1. If data completeness < 60%, give Hold and explain missing data
2. If data completeness >= 60%, must give clear directional judgment
3. Use this decision matrix:
   - Technical bullish + Fundamentals healthy → Buy
   - Technical bearish + Fundamentals deteriorating → Sell
   - Contradictory signals or insufficient data → Hold

### Output Requirements
1. Must give clear Buy/Sell/Hold recommendation
2. Must give confidence score (0-100)
3. Must list supporting and opposing evidence
4. Must explain impact of missing data on decision
```

#### 3. Confidence Calibration

```rust
struct ConfidenceCalibration {
    base_confidence: f64,
    data_completeness_factor: f64,  // 0.5-1.5
    signal_consistency_factor: f64, // 0.5-1.5
    historical_accuracy_factor: f64, // 0.5-1.5
    final_confidence: f64,
}

impl ConfidenceCalibration {
    fn calibrate(
        &self,
        data_completeness: f64,
        signal_consistency: f64,
        historical_accuracy: f64,
    ) -> f64 {
        let adjusted = self.base_confidence
            * self.data_completeness_factor * data_completeness
            * self.signal_consistency_factor * signal_consistency
            * self.historical_accuracy_factor * historical_accuracy;
        adjusted.clamp(0.0, 100.0)
    }
}
```

#### 4. Optimized Debate Structure

```
Current: Research → Bull/Bear Debate → Trader (easily Hold)

Optimized: Research → Bull/Bear Debate → Confidence Check → Decision
                                                        ↓
                                              ┌─────────┴─────────┐
                                              │                   │
                                         Confident?          Uncertain?
                                              │                   │
                                              ▼                   ▼
                                         Buy/Sell              Hold
                                         (with confidence)     (with reasons)
```

#### 5. Decision Matrix

| Technical | Fundamental | News/Sentiment | Recommendation | Confidence |
|-----------|-------------|----------------|----------------|------------|
| Bullish | Healthy | Positive | **Buy** | 80-95 |
| Bullish | Healthy | Neutral | **Buy** | 70-85 |
| Bullish | Healthy | Negative | Hold | 50-65 |
| Bullish | Deteriorating | Positive | Hold | 45-60 |
| Bullish | Deteriorating | Negative | **Sell** | 70-85 |
| Bearish | Healthy | Positive | Hold | 50-65 |
| Bearish | Healthy | Negative | **Sell** | 70-85 |
| Bearish | Deteriorating | Positive | Hold | 45-60 |
| Bearish | Deteriorating | Negative | **Sell** | 80-95 |

---

## Testing Strategy

### Data Pipeline Tests

1. **Unit Tests**
   - Test ParallelExecutor parallel fetching logic
   - Test DataCacheLayer cache hit/miss
   - Test DataValidator data completeness check

2. **Integration Tests**
   - Test complete data fetching flow
   - Test timeout and retry mechanisms
   - Test data source fallback strategy

3. **End-to-End Tests**
   - Test complete report generation for 3 markets
   - Verify data fetching success rate
   - Verify report quality (no longer all Hold)

### LLM Reasoning Tests

1. **Prompt Tests**
   - Test prompt generation with different data completeness levels
   - Test decision framework logic correctness

2. **Decision Matrix Tests**
   - Test all possible signal combinations
   - Verify decision logic correctness

3. **Regression Tests**
   - Ensure improvements don't break existing functionality
   - Verify report quality improvement

---

## Expected Outcomes

1. **Data fetching success rate**: From ~70% to ~95%
2. **Report differentiation**: From 100% Hold to ~40% Buy/Sell
3. **Execution confidence**: From 38-41/100 to 60-75/100
4. **Report generation time**: From ~18min to ~10min

---

## Implementation Phases

### Phase 1: Data Pipeline Refactoring (1-2 days)

- [ ] Implement ParallelExecutor
- [ ] Implement DataCacheLayer
- [ ] Implement DataValidator
- [ ] Add unit tests
- [ ] Add integration tests

### Phase 2: LLM Reasoning Optimization (1-2 days)

- [ ] Implement DecisionFramework
- [ ] Implement ConfidenceCalibration
- [ ] Optimize Prompt template
- [ ] Add unit tests
- [ ] Add integration tests

### Phase 3: Integration Testing (1 day)

- [ ] End-to-end testing
- [ ] Performance testing
- [ ] Regression testing
- [ ] Update documentation

---

## Risk Assessment

### Technical Risks

1. **Cache invalidation complexity** — Mitigated by using simple TTL-based invalidation
2. **Retry mechanism overhead** — Mitigated by exponential backoff and max retry limits
3. **LLM prompt instability** — Mitigated by extensive testing and fallback logic

### Schedule Risks

1. **Scope creep** — Mitigated by clear phase boundaries
2. **Integration issues** — Mitigated by incremental testing
3. **Performance regression** — Mitigated by benchmarking before/after

---

## Success Criteria

1. **Data completeness**: ≥95% of reports have complete fundamental data
2. **Report differentiation**: ≤60% of reports give Hold (down from 100%)
3. **Execution confidence**: Average confidence ≥60/100 (up from 38-41)
4. **Performance**: Report generation time ≤12 minutes (down from 18)
5. **Test coverage**: ≥90% code coverage for new components
