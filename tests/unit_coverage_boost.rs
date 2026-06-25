use sa::CacheStore;

// =========================================================================
// shared.rs — safe_ticker_component
// =========================================================================

#[test]
fn safe_ticker_component_valid() {
    let result = sa::shared::safe_ticker_component("AAPL", 10).unwrap();
    assert_eq!(result, "AAPL");
}

#[test]
fn safe_ticker_component_with_special_chars() {
    let result = sa::shared::safe_ticker_component("600519.SH", 20).unwrap();
    assert_eq!(result, "600519.SH");
}

#[test]
fn safe_ticker_component_replaces_invalid_chars() {
    let result = sa::shared::safe_ticker_component("AAPL/MSFT", 20).unwrap();
    assert_eq!(result, "AAPL_MSFT");
}

#[test]
fn safe_ticker_component_truncates() {
    let result = sa::shared::safe_ticker_component("VERYLONGTICKER", 5).unwrap();
    assert_eq!(result, "VERYL");
}

#[test]
fn safe_ticker_component_empty_fails() {
    assert!(sa::shared::safe_ticker_component("", 10).is_err());
    assert!(sa::shared::safe_ticker_component("   ", 10).is_err());
}

// =========================================================================
// task.rs — TaskStatus
// =========================================================================

#[test]
fn task_status_as_str() {
    assert_eq!(sa::task::TaskStatus::Pending.as_str(), "pending");
    assert_eq!(sa::task::TaskStatus::Running.as_str(), "running");
    assert_eq!(sa::task::TaskStatus::Completed.as_str(), "completed");
    assert_eq!(sa::task::TaskStatus::Cancelled.as_str(), "cancelled");
    assert_eq!(sa::task::TaskStatus::Failed.as_str(), "failed");
}

#[test]
fn task_status_from_str_roundtrip() {
    let cases = ["pending", "running", "completed", "cancelled", "failed"];
    for s in cases {
        let status: sa::task::TaskStatus = s.parse().unwrap();
        assert_eq!(status.as_str(), s);
    }
}

#[test]
fn task_status_from_str_invalid() {
    assert!("unknown".parse::<sa::task::TaskStatus>().is_err());
    assert!("".parse::<sa::task::TaskStatus>().is_err());
}

// =========================================================================
// scoring/config.rs — ScoreConfig
// =========================================================================

#[test]
fn score_config_default() {
    let config = sa::scoring::config::ScoreConfig::default();
    assert_eq!(config.sentiment_news_limit, 10);
    // Weights should sum to 100
    let sum = config.weights.technical + config.weights.fundamental + config.weights.sentiment + config.weights.llm_analysis;
    assert_eq!(sum, 100);
}

// =========================================================================
// scoring/score_types.rs — ScoreWeights
// =========================================================================

#[test]
fn score_weights_default_valid() {
    let weights = sa::scoring::types::ScoreWeights::default();
    assert!(weights.validate().is_ok());
}

#[test]
fn score_weights_invalid_sum() {
    let weights = sa::scoring::types::ScoreWeights {
        technical: 50,
        fundamental: 50,
        sentiment: 50,
        llm_analysis: 50,
    };
    assert!(weights.validate().is_err());
}

#[test]
fn score_label_mapping() {
    assert_eq!(sa::scoring::types::score_label(85), "strong_buy");
    assert_eq!(sa::scoring::types::score_label(70), "buy");
    assert_eq!(sa::scoring::types::score_label(55), "neutral");
    assert_eq!(sa::scoring::types::score_label(35), "cautious");
    assert_eq!(sa::scoring::types::score_label(20), "avoid");
}

// =========================================================================
// value_utils.rs
// =========================================================================

#[test]
fn normalize_value_null() {
    assert_eq!(sa::value_utils::normalize_value(&serde_json::Value::Null), "");
}

#[test]
fn normalize_value_string() {
    assert_eq!(
        sa::value_utils::normalize_value(&serde_json::json!("hello")),
        "hello"
    );
}

#[test]
fn normalize_value_number() {
    assert_eq!(
        sa::value_utils::normalize_value(&serde_json::json!(42)),
        "42"
    );
}

#[test]
fn normalize_value_bool() {
    assert_eq!(
        sa::value_utils::normalize_value(&serde_json::json!(true)),
        "true"
    );
}

#[test]
fn normalize_probability_valid() {
    assert_eq!(sa::value_utils::normalize_probability(&serde_json::json!(0.5)), 0.5);
    assert_eq!(sa::value_utils::normalize_probability(&serde_json::json!(-0.1)), 0.0);
    assert_eq!(sa::value_utils::normalize_probability(&serde_json::json!(1.5)), 1.0);
}

// =========================================================================
// scoring/dimensions/technical.rs
// =========================================================================

#[test]
fn score_technical_bullish() {
    let input = sa::scoring::dimensions::technical::TechnicalInput {
        rsi: Some(35.0),
        macd: Some(0.5),
        macd_signal: Some(0.3),
        macd_hist: Some(0.2),
        adx: Some(25.0),
        close_10_ema: Some(180.0),
        close_50_sma: Some(175.0),
        close_200_sma: Some(170.0),
        obv: None,
        current_price: Some(185.0),
        volume_elevated: true,
        latest_positive: true,
    };
    let result = sa::scoring::dimensions::technical::score_technical(&input);
    assert!(result.score >= 60, "expected bullish score, got {}", result.score);
}

#[test]
fn score_technical_bearish() {
    let input = sa::scoring::dimensions::technical::TechnicalInput {
        rsi: Some(75.0),
        macd: Some(-0.5),
        macd_signal: Some(-0.2),
        macd_hist: Some(-0.3),
        adx: Some(30.0),
        close_10_ema: Some(90.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(85.0),
        volume_elevated: true,
        latest_positive: false,
    };
    let result = sa::scoring::dimensions::technical::score_technical(&input);
    assert!(result.score <= 40, "expected bearish score, got {}", result.score);
}

// =========================================================================
// scoring/dimensions/fundamental.rs
// =========================================================================

#[test]
fn score_fundamental_good() {
    let input = sa::scoring::dimensions::fundamental::FundamentalInput {
        pe_like: Some(12.0),
        ps_like: Some(3.0),
        roe: Some(25.0),
        leverage: Some(0.8),
        market_cap: Some(100_000_000_000.0),
        revenues_usd: Some(50_000_000_000.0),
        net_income_usd: Some(10_000_000_000.0),
    };
    let result = sa::scoring::dimensions::fundamental::score_fundamental(&input);
    assert!(result.score >= 60, "expected good fundamental score, got {}", result.score);
}

#[test]
fn score_fundamental_bad() {
    let input = sa::scoring::dimensions::fundamental::FundamentalInput {
        pe_like: Some(100.0),
        ps_like: Some(20.0),
        roe: Some(-10.0),
        leverage: Some(5.0),
        market_cap: Some(1_000_000_000.0),
        revenues_usd: Some(100_000_000.0),
        net_income_usd: Some(-50_000_000.0),
    };
    let result = sa::scoring::dimensions::fundamental::score_fundamental(&input);
    assert!(result.score <= 40, "expected bad fundamental score, got {}", result.score);
}

// =========================================================================
// scoring/dimensions/llm_analysis.rs
// =========================================================================

#[test]
fn score_llm_analysis_consensus() {
    let input = sa::scoring::dimensions::llm_analysis::LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 70.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 6,
        hard_negative_count: 0,
        volume_ratio: Some(1.2),
        period_return_pct: Some(3.0),
    };
    let result = sa::scoring::dimensions::llm_analysis::score_llm_analysis(&input);
    assert!(result.score >= 55, "expected decent score, got {}", result.score);
}

// =========================================================================
// analysis/derived.rs — rr_label
// =========================================================================

#[test]
fn rr_label_sufficient() {
    assert_eq!(sa::analysis::rr_label(2.5), "赔率充裕");
    assert_eq!(sa::analysis::rr_label(2.0), "赔率充裕");
}

#[test]
fn rr_label_moderate() {
    assert_eq!(sa::analysis::rr_label(1.5), "赔率尚可");
    assert_eq!(sa::analysis::rr_label(1.2), "赔率尚可");
}

#[test]
fn rr_label_weak() {
    assert_eq!(sa::analysis::rr_label(0.8), "赔率偏弱");
    assert_eq!(sa::analysis::rr_label(0.5), "赔率偏弱");
}

#[test]
fn rr_label_poor() {
    assert_eq!(sa::analysis::rr_label(0.3), "赔率极差");
    assert_eq!(sa::analysis::rr_label(0.0), "赔率极差");
}

// =========================================================================
// task_manager.rs — task steps
// =========================================================================

#[test]
fn task_steps_not_empty() {
    assert!(!sa::task_manager::TASK_STEPS.is_empty());
}

// =========================================================================
// store/ traits — InMemory stores
// =========================================================================

#[tokio::test]
async fn in_memory_cache_store_basic() {
    let store = sa::store::InMemoryCacheStore::new();
    // Set and get
    store.set("key1", b"value1", None).await.unwrap();
    let result = store.get("key1").await.unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));
    // Exists
    assert!(store.exists("key1").await.unwrap());
    assert!(!store.exists("key2").await.unwrap());
    // Delete
    store.delete("key1").await.unwrap();
    assert!(store.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn in_memory_cache_store_list_entries() {
    let store = sa::store::InMemoryCacheStore::new();
    store.set("prefix:a", b"1", None).await.unwrap();
    store.set("prefix:b", b"2", None).await.unwrap();
    store.set("other:c", b"3", None).await.unwrap();
    let entries = store.list_entries("prefix:").await.unwrap();
    assert_eq!(entries.len(), 2);
}

// =========================================================================
// scoring/scorer.rs — weighted_total
// =========================================================================

#[test]
fn weighted_total_balanced() {
    let weights = sa::scoring::types::ScoreWeights::default();
    let tech = sa::scoring::DimensionScore { score: 80, reason: String::new() };
    let fund = sa::scoring::DimensionScore { score: 60, reason: String::new() };
    let sent = sa::scoring::DimensionScore { score: 70, reason: String::new() };
    let llm = sa::scoring::DimensionScore { score: 90, reason: String::new() };
    let total = sa::scoring::scorer::weighted_total(&weights, &tech, &fund, &sent, &llm);
    assert!(total >= 70 && total <= 80, "expected ~75, got {total}");
}

// =========================================================================
// data.rs — MarketKind
// =========================================================================

#[test]
fn market_kind_variants() {
    let a = sa::data::MarketKind::AShare;
    let hk = sa::data::MarketKind::HongKong;
    let us = sa::data::MarketKind::UsEquity;
    assert_ne!(a, hk);
    assert_ne!(hk, us);
    assert_ne!(a, us);
}
