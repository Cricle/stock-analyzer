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
    let sum = config.weights.technical
        + config.weights.fundamental
        + config.weights.sentiment
        + config.weights.llm_analysis;
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
    assert_eq!(
        sa::value_utils::normalize_value(&serde_json::Value::Null),
        ""
    );
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
    assert_eq!(
        sa::value_utils::normalize_probability(&serde_json::json!(0.5)),
        0.5
    );
    assert_eq!(
        sa::value_utils::normalize_probability(&serde_json::json!(-0.1)),
        0.0
    );
    assert_eq!(
        sa::value_utils::normalize_probability(&serde_json::json!(1.5)),
        1.0
    );
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
    assert!(
        result.score >= 60,
        "expected bullish score, got {}",
        result.score
    );
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
    assert!(
        result.score <= 40,
        "expected bearish score, got {}",
        result.score
    );
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
    assert!(
        result.score >= 60,
        "expected good fundamental score, got {}",
        result.score
    );
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
    assert!(
        result.score <= 40,
        "expected bad fundamental score, got {}",
        result.score
    );
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
    assert!(
        result.score >= 55,
        "expected decent score, got {}",
        result.score
    );
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

use sa::CacheStore;

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
    let tech = sa::scoring::DimensionScore {
        score: 80,
        reason: String::new(),
    };
    let fund = sa::scoring::DimensionScore {
        score: 60,
        reason: String::new(),
    };
    let sent = sa::scoring::DimensionScore {
        score: 70,
        reason: String::new(),
    };
    let llm = sa::scoring::DimensionScore {
        score: 90,
        reason: String::new(),
    };
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

// =========================================================================
// pick/objective/constraints.rs — format_valuation_line
// =========================================================================

#[test]
fn format_valuation_line_valid() {
    let result = sa::pick::objective::format_valuation_line("PE", Some(15.0), 20.0);
    assert!(result.is_some());
    let line = result.unwrap();
    assert!(line.contains("PE"));
    assert!(line.contains("15.0"));
    assert!(line.contains("20.0"));
    assert!(line.contains("discount"));
}

#[test]
fn format_valuation_line_premium() {
    let result = sa::pick::objective::format_valuation_line("PE", Some(25.0), 20.0);
    assert!(result.is_some());
    let line = result.unwrap();
    assert!(line.contains("premium"));
}

#[test]
fn format_valuation_line_none_value() {
    let result = sa::pick::objective::format_valuation_line("PE", None, 20.0);
    assert!(result.is_none());
}

#[test]
fn format_valuation_line_zero_value() {
    let result = sa::pick::objective::format_valuation_line("PE", Some(0.0), 20.0);
    assert!(result.is_none());
}

#[test]
fn format_valuation_line_negative_value() {
    let result = sa::pick::objective::format_valuation_line("PE", Some(-5.0), 20.0);
    assert!(result.is_none());
}

// =========================================================================
// pick/objective/constraints.rs — stock_pick_objective_grade
// =========================================================================

#[test]
fn stock_pick_objective_grade_a() {
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(85), "A");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(95), "A");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(100), "A");
}

#[test]
fn stock_pick_objective_grade_b() {
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(75), "B");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(80), "B");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(84), "B");
}

#[test]
fn stock_pick_objective_grade_c() {
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(60), "C");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(70), "C");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(74), "C");
}

#[test]
fn stock_pick_objective_grade_d() {
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(0), "D");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(50), "D");
    assert_eq!(sa::pick::objective::stock_pick_objective_grade(59), "D");
}

// =========================================================================
// pick/objective/constraints.rs — stock_pick_objective_headline
// =========================================================================

#[test]
fn stock_pick_objective_headline_ready_high() {
    let headline = sa::pick::objective::stock_pick_objective_headline(90, true, &[]);
    assert!(headline.contains("High-quality"));
}

#[test]
fn stock_pick_objective_headline_ready_normal() {
    let headline = sa::pick::objective::stock_pick_objective_headline(75, true, &[]);
    assert!(headline.contains("Usable"));
}

#[test]
fn stock_pick_objective_headline_not_ready_no_gaps() {
    let headline = sa::pick::objective::stock_pick_objective_headline(60, false, &[]);
    assert!(headline.contains("mixed"));
}

#[test]
fn stock_pick_objective_headline_not_ready_with_gaps() {
    let gaps = vec![
        "missing_fundamentals".to_string(),
        "thin_evidence".to_string(),
    ];
    let headline = sa::pick::objective::stock_pick_objective_headline(50, false, &gaps);
    assert!(headline.contains("Not fully ready"));
    assert!(headline.contains("missing_fundamentals"));
}

// =========================================================================
// pick/pipeline/rank.rs
// =========================================================================

#[test]
fn rank_news_items_to_evidence_records() {
    let news = vec![sa::data::NewsItem {
        title: "Test news".to_string(),
        source: "Test source".to_string(),
        published_at: "2024-01-01".to_string(),
        url: Some("https://example.com".to_string()),
        summary: "Test summary".to_string(),
    }];
    let records = sa::pick::pipeline::rank::news_items_to_evidence_records(
        "AAPL",
        "美股",
        "tech",
        &[],
        &news,
    );
    assert!(!records.is_empty());
}

// =========================================================================
// pick/pipeline/filter.rs — capital_flow_source_score
// =========================================================================

#[test]
fn capital_flow_source_score_empty() {
    let score = sa::pick::pipeline::filter::capital_flow_source_score(&[]);
    assert_eq!(score, 0.0);
}

#[test]
fn capital_flow_source_score_with_data() {
    let items = vec![sa::data::CapitalFlowPoint {
        trade_date: "2024-01-01".to_string(),
        main_net_inflow: 1000000.0,
        small_net_inflow: 500000.0,
        medium_net_inflow: 300000.0,
        large_net_inflow: 200000.0,
        super_large_net_inflow: 100000.0,
        main_net_inflow_ratio_pct: 5.0,
        small_net_inflow_ratio_pct: 2.5,
        medium_net_inflow_ratio_pct: 1.5,
        large_net_inflow_ratio_pct: 1.0,
        super_large_net_inflow_ratio_pct: 0.5,
        close: 100.0,
        change_pct: 2.0,
    }];
    let score = sa::pick::pipeline::filter::capital_flow_source_score(&items);
    assert!(score > 0.0);
}

// =========================================================================
// pick/pipeline/filter.rs — billboard_source_score
// =========================================================================

#[test]
fn billboard_source_score_empty() {
    let score = sa::pick::pipeline::filter::billboard_source_score(&[]);
    assert_eq!(score, 0.0);
}

#[test]
fn billboard_source_score_with_data() {
    let items = vec![sa::data::BillboardEntry {
        trade_date: "2024-01-01".to_string(),
        symbol: "600519".to_string(),
        name: "贵州茅台".to_string(),
        close_price: 1800.0,
        change_rate_pct: 2.5,
        turnover_rate_pct: Some(1.2),
        net_amount: Some(50000000.0),
        buy_amount: Some(100000000.0),
        sell_amount: Some(50000000.0),
        explanation: Some("大宗交易".to_string()),
        reason: Some("机构买入".to_string()),
    }];
    let score = sa::pick::pipeline::filter::billboard_source_score(&items);
    assert!(score > 0.0);
}

// =========================================================================
// pick/objective/constraints.rs — build_valuation_vs_industry_block
// =========================================================================

#[test]
fn build_valuation_vs_industry_block_empty() {
    let block = sa::pick::objective::build_valuation_vs_industry_block(&[], &[]);
    assert!(block.is_empty());
}

// =========================================================================
// user_preferences.rs
// =========================================================================

#[test]
fn user_preferences_default() {
    let prefs = sa::user_preferences::UserPreferences::default();
    assert!(prefs.watchlist.is_empty());
}

#[test]
fn user_preferences_add_remove() {
    let mut prefs = sa::user_preferences::UserPreferences::default();
    let item = sa::user_preferences::WatchlistItem {
        symbol: "AAPL".to_string(),
        name: "Apple Inc.".to_string(),
        market: "美股".to_string(),
        notes: String::new(),
        added_at: "2024-01-01".to_string(),
    };
    prefs.add_to_watchlist(item);
    assert_eq!(prefs.watchlist.len(), 1);
    prefs.remove_from_watchlist("AAPL", "美股");
    assert!(prefs.watchlist.is_empty());
}

// =========================================================================
// pick/pipeline/filter.rs — market helpers
// =========================================================================

#[test]
fn market_kind_from_value_a_share() {
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("a"),
        sa::data::MarketKind::AShare
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("A-share"),
        sa::data::MarketKind::AShare
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("cn"),
        sa::data::MarketKind::AShare
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("a股"),
        sa::data::MarketKind::AShare
    );
}

#[test]
fn market_kind_from_value_hk() {
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("hk"),
        sa::data::MarketKind::HongKong
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("港股"),
        sa::data::MarketKind::HongKong
    );
}

#[test]
fn market_kind_from_value_us() {
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("us"),
        sa::data::MarketKind::UsEquity
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_kind_from_value("unknown"),
        sa::data::MarketKind::UsEquity
    );
}

#[test]
fn market_display_label_all() {
    assert_eq!(
        sa::pick::pipeline::filter::market_display_label(sa::data::MarketKind::AShare),
        "A-share"
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_display_label(sa::data::MarketKind::HongKong),
        "HK"
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_display_label(sa::data::MarketKind::UsEquity),
        "US"
    );
}

#[test]
fn market_search_label_all() {
    assert_eq!(
        sa::pick::pipeline::filter::market_search_label(sa::data::MarketKind::AShare),
        "A-share"
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_search_label(sa::data::MarketKind::HongKong),
        "HK"
    );
}

#[test]
fn market_exchange_code_all() {
    assert_eq!(
        sa::pick::pipeline::filter::market_exchange_code(sa::data::MarketKind::AShare),
        "CN"
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_exchange_code(sa::data::MarketKind::HongKong),
        "HK"
    );
    assert_eq!(
        sa::pick::pipeline::filter::market_exchange_code(sa::data::MarketKind::UsEquity),
        "US"
    );
}

// =========================================================================
// checkpoint/mod.rs — hex_16
// =========================================================================

#[test]
fn hex_16_basic() {
    let bytes: &[u8] = &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    let result = sa::checkpoint::hex_16(bytes);
    assert_eq!(result, "0123456789abcdef");
}

#[test]
fn hex_16_zeros() {
    let bytes: &[u8] = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let result = sa::checkpoint::hex_16(bytes);
    assert_eq!(result, "0000000000000000");
}

#[test]
fn hex_16_short_input() {
    let bytes: &[u8] = &[0xff, 0x00];
    let result = sa::checkpoint::hex_16(bytes);
    assert_eq!(result, "ff00");
}

// =========================================================================
// task_manager.rs — seconds_until_local_midnight
// =========================================================================

#[test]
fn seconds_until_local_midnight_positive() {
    let secs = sa::task_manager::seconds_until_local_midnight();
    assert!(secs > 0);
    assert!(secs <= 86400);
}

// =========================================================================
// memory/stats.rs — extract_labeled_block
// =========================================================================

#[test]
fn extract_labeled_block_found() {
    let text = "META:\nmeta content\nDECISION:\ndecision content\nREFLECTION:\nreflection content";
    assert_eq!(
        sa::memory::stats::extract_labeled_block(text, "META"),
        Some("meta content")
    );
    assert_eq!(
        sa::memory::stats::extract_labeled_block(text, "DECISION"),
        Some("decision content")
    );
    assert_eq!(
        sa::memory::stats::extract_labeled_block(text, "REFLECTION"),
        Some("reflection content")
    );
}

#[test]
fn extract_labeled_block_not_found() {
    let text = "DECISION:\ndecision content";
    assert!(sa::memory::stats::extract_labeled_block(text, "META").is_none());
}

#[test]
fn extract_labeled_block_empty_text() {
    assert!(sa::memory::stats::extract_labeled_block("", "META").is_none());
}

// =========================================================================
// memory/stats.rs — realized_call_hit
// =========================================================================

#[test]
fn realized_call_hit_buy() {
    assert!(sa::memory::stats::realized_call_hit("Buy", 0.05, 0.03));
    assert!(!sa::memory::stats::realized_call_hit("Buy", -0.05, 0.03));
    assert!(!sa::memory::stats::realized_call_hit("Buy", 0.05, -0.03));
}

#[test]
fn realized_call_hit_overweight() {
    assert!(sa::memory::stats::realized_call_hit(
        "Overweight",
        -0.05,
        0.03
    ));
    assert!(!sa::memory::stats::realized_call_hit(
        "Overweight",
        0.05,
        -0.03
    ));
}

#[test]
fn realized_call_hit_hold() {
    assert!(sa::memory::stats::realized_call_hit("Hold", 0.02, 0.01));
    assert!(!sa::memory::stats::realized_call_hit("Hold", 0.10, 0.10));
}

#[test]
fn realized_call_hit_underweight() {
    assert!(sa::memory::stats::realized_call_hit(
        "Underweight",
        0.05,
        -0.03
    ));
    assert!(!sa::memory::stats::realized_call_hit(
        "Underweight",
        0.05,
        0.03
    ));
}

#[test]
fn realized_call_hit_sell() {
    assert!(sa::memory::stats::realized_call_hit("Sell", -0.05, -0.03));
    assert!(!sa::memory::stats::realized_call_hit("Sell", 0.05, -0.03));
}

#[test]
fn realized_call_hit_unknown() {
    assert!(!sa::memory::stats::realized_call_hit("Unknown", 0.05, 0.03));
}

// =========================================================================
// memory/stats.rs — bucket_score & bucket_signed_score
// =========================================================================

#[test]
fn bucket_score_various() {
    assert_eq!(
        sa::memory::stats::bucket_score(None, &[40, 55, 70, 85]),
        "unknown"
    );
    assert_eq!(
        sa::memory::stats::bucket_score(Some(30), &[40, 55, 70, 85]),
        "<40"
    );
    assert_eq!(
        sa::memory::stats::bucket_score(Some(50), &[40, 55, 70, 85]),
        "40-54"
    );
    assert_eq!(
        sa::memory::stats::bucket_score(Some(60), &[40, 55, 70, 85]),
        "55-69"
    );
    assert_eq!(
        sa::memory::stats::bucket_score(Some(80), &[40, 55, 70, 85]),
        "70-84"
    );
    assert_eq!(
        sa::memory::stats::bucket_score(Some(90), &[40, 55, 70, 85]),
        ">=85"
    );
}

#[test]
fn bucket_signed_score_various() {
    assert_eq!(
        sa::memory::stats::bucket_signed_score(None, &[-60, -20, 20, 60]),
        "unknown"
    );
    assert_eq!(
        sa::memory::stats::bucket_signed_score(Some(-70), &[-60, -20, 20, 60]),
        "<-60"
    );
    assert_eq!(
        sa::memory::stats::bucket_signed_score(Some(-30), &[-60, -20, 20, 60]),
        "-60..-21"
    );
    assert_eq!(
        sa::memory::stats::bucket_signed_score(Some(0), &[-60, -20, 20, 60]),
        "-20..19"
    );
    assert_eq!(
        sa::memory::stats::bucket_signed_score(Some(30), &[-60, -20, 20, 60]),
        "20..59"
    );
    assert_eq!(
        sa::memory::stats::bucket_signed_score(Some(70), &[-60, -20, 20, 60]),
        ">=60"
    );
}

// =========================================================================
// memory/stats.rs — summarize_entries
// =========================================================================

#[test]
fn summarize_entries_empty() {
    let result = sa::memory::stats::summarize_entries(&[]);
    assert_eq!(result["count"], 0);
    assert_eq!(result["hit_rate"], 0.0);
}

#[test]
fn summarize_entries_with_data() {
    use sa::memory::MemoryEntry;
    let entries = vec![
        MemoryEntry {
            ticker: "AAPL".to_string(),
            rating: "Buy".to_string(),
            raw_return: Some(0.05),
            alpha_return: Some(0.03),
            ..MemoryEntry::default()
        },
        MemoryEntry {
            ticker: "MSFT".to_string(),
            rating: "Sell".to_string(),
            raw_return: Some(-0.02),
            alpha_return: Some(-0.01),
            ..MemoryEntry::default()
        },
    ];
    let result = sa::memory::stats::summarize_entries(&entries);
    assert_eq!(result["count"], 2);
    assert!(result["avg_raw_return"].as_f64().unwrap() > 0.0);
}

// =========================================================================
// memory/stats.rs — derive_calibration_profile
// =========================================================================

#[test]
fn derive_calibration_profile_insufficient() {
    use sa::memory::MemoryEntry;
    let entries: Vec<sa::memory::MemoryEntry> = (0..5).map(|_| MemoryEntry::default()).collect();
    let profile = sa::memory::stats::derive_calibration_profile(&entries);
    assert_eq!(profile.sample_count, 0);
}

// =========================================================================
// memory/stats.rs — evaluate_profile_candidate
// =========================================================================

#[test]
fn evaluate_profile_candidate_buy_hit() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        rating: "Buy".to_string(),
        direction_score: Some(80),
        confidence_score: Some(70),
        action_score: Some(60),
        raw_return: Some(0.05),
        alpha_return: Some(0.03),
        ..MemoryEntry::default()
    }];
    let score = sa::memory::stats::evaluate_profile_candidate(&entries, 60, 50, 20, 60);
    assert!(score > 0.0);
}

#[test]
fn evaluate_profile_candidate_hold() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        rating: "Hold".to_string(),
        direction_score: Some(10),
        confidence_score: Some(50),
        action_score: Some(40),
        raw_return: Some(0.01),
        alpha_return: Some(0.005),
        ..MemoryEntry::default()
    }];
    let score = sa::memory::stats::evaluate_profile_candidate(&entries, 60, 50, 20, 60);
    // Hold entries don't count as coverage
    assert!(score >= 0.0);
}

#[test]
fn evaluate_profile_candidate_sell() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        rating: "Sell".to_string(),
        direction_score: Some(-80),
        confidence_score: Some(70),
        action_score: Some(60),
        raw_return: Some(-0.05),
        alpha_return: Some(-0.03),
        ..MemoryEntry::default()
    }];
    let score = sa::memory::stats::evaluate_profile_candidate(&entries, 60, 50, 20, 60);
    assert!(score > 0.0);
}

#[test]
fn evaluate_profile_candidate_overweight() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        rating: "Overweight".to_string(),
        direction_score: Some(40),
        confidence_score: Some(70),
        action_score: Some(60),
        raw_return: Some(0.02),
        alpha_return: Some(0.01),
        ..MemoryEntry::default()
    }];
    let score = sa::memory::stats::evaluate_profile_candidate(&entries, 60, 50, 20, 60);
    assert!(score >= 0.0);
}

#[test]
fn evaluate_profile_candidate_underweight() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        rating: "Underweight".to_string(),
        direction_score: Some(-40),
        confidence_score: Some(70),
        action_score: Some(60),
        raw_return: Some(-0.02),
        alpha_return: Some(-0.01),
        ..MemoryEntry::default()
    }];
    let score = sa::memory::stats::evaluate_profile_candidate(&entries, 60, 50, 20, 60);
    assert!(score >= 0.0);
}

#[test]
fn suggested_calibration_profile_basic() {
    use sa::memory::MemoryEntry;
    let entries: Vec<_> = (0..15)
        .map(|_i| MemoryEntry {
            ticker: "AAPL".to_string(),
            rating: "Buy".to_string(),
            direction_score: Some(80),
            confidence_score: Some(70),
            action_score: Some(60),
            raw_return: Some(0.05),
            alpha_return: Some(0.03),
            ..MemoryEntry::default()
        })
        .collect();
    let result = sa::memory::stats::suggested_calibration_profile(&entries);
    assert!(result.get("sample_count").is_some());
    assert!(result.get("is_default_profile").is_some());
}

// =========================================================================
// memory/stats.rs — group_summary
// =========================================================================

#[test]
fn group_summary_by_ticker() {
    use sa::memory::MemoryEntry;
    let entries = vec![
        MemoryEntry {
            ticker: "AAPL".to_string(),
            raw_return: Some(0.05),
            alpha_return: Some(0.03),
            rating: "Buy".to_string(),
            ..MemoryEntry::default()
        },
        MemoryEntry {
            ticker: "AAPL".to_string(),
            raw_return: Some(-0.02),
            alpha_return: Some(-0.01),
            rating: "Hold".to_string(),
            ..MemoryEntry::default()
        },
        MemoryEntry {
            ticker: "MSFT".to_string(),
            raw_return: Some(0.10),
            alpha_return: Some(0.08),
            rating: "Buy".to_string(),
            ..MemoryEntry::default()
        },
    ];
    let result = sa::memory::stats::group_summary(&entries, |e| e.ticker.clone());
    assert!(result.get("AAPL").is_some());
    assert!(result.get("MSFT").is_some());
    assert_eq!(result["AAPL"]["count"], 2);
    assert_eq!(result["MSFT"]["count"], 1);
}

// =========================================================================
// scoring/dimensions/mod.rs — weighted_score
// =========================================================================

#[test]
fn weighted_score_balanced() {
    // total/weight_sum * 100 = 2/4 * 100 = 50
    let result = sa::scoring::dimensions::weighted_score(2.0, 4.0, "default", &[]);
    assert_eq!(result.score, 50);
    assert_eq!(result.reason, "default");
}

#[test]
fn weighted_score_with_reasons() {
    let reasons = vec!["reason1".to_string(), "reason2".to_string()];
    // total/weight_sum * 100 = 3/4 * 100 = 75
    let result = sa::scoring::dimensions::weighted_score(3.0, 4.0, "default", &reasons);
    assert_eq!(result.score, 75);
    assert!(result.reason.contains("reason1"));
}

#[test]
fn weighted_score_zero_weight() {
    let result = sa::scoring::dimensions::weighted_score(100.0, 0.0, "no data", &[]);
    assert_eq!(result.score, 50);
}

#[test]
fn weighted_score_clamps() {
    let result = sa::scoring::dimensions::weighted_score(500.0, 4.0, "default", &[]);
    assert_eq!(result.score, 100);
}

// =========================================================================
// analysis/derived.rs — trait methods (via public types)
// =========================================================================

#[test]
fn report_stage_variants() {
    // Just test that the function doesn't panic with a default ReportCandle
    use sa::analysis::ReportCandle;
    let candle = ReportCandle {
        trade_date: "2024-01-01".to_string(),
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 102.0,
        volume: 1000,
        amount: 100000.0,
        amplitude_pct: 5.0,
        change_pct: 2.0,
        change_amount: 2.0,
        turnover_pct: 1.0,
    };
    // These are mainly coverage tests — ensure no panic
    let _ = format!("{:?}", candle);
}

// =========================================================================
// pick/pipeline/rank.rs — more coverage
// =========================================================================

#[test]
fn news_items_to_evidence_records_empty() {
    let records =
        sa::pick::pipeline::rank::news_items_to_evidence_records("AAPL", "美股", "tech", &[], &[]);
    assert!(records.is_empty());
}

#[test]
fn news_items_to_evidence_records_with_queries() {
    let news = vec![
        sa::data::NewsItem {
            title: "Apple beats earnings".to_string(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: Some("https://example.com/1".to_string()),
            summary: "Apple reported strong earnings".to_string(),
        },
        sa::data::NewsItem {
            title: "Apple beats earnings".to_string(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: Some("https://example.com/1".to_string()),
            summary: "Duplicate".to_string(),
        },
    ];
    let queries = vec!["Apple".to_string(), "earnings".to_string()];
    let records = sa::pick::pipeline::rank::news_items_to_evidence_records(
        "AAPL", "美股", "tech", &queries, &news,
    );
    // Should deduplicate
    assert_eq!(records.len(), 1);
}

// =========================================================================
// memory/mod.rs — hash_embed_text
// =========================================================================

#[test]
fn hash_embed_text_deterministic() {
    let v1 = sa::memory::hash_embed_text("hello world", 128);
    let v2 = sa::memory::hash_embed_text("hello world", 128);
    assert_eq!(v1, v2);
}

#[test]
fn hash_embed_text_different_inputs() {
    let v1 = sa::memory::hash_embed_text("hello", 128);
    let v2 = sa::memory::hash_embed_text("world", 128);
    assert_ne!(v1, v2);
}

#[test]
fn hash_embed_text_dimension() {
    let v = sa::memory::hash_embed_text("test", 64);
    assert_eq!(v.len(), 64);
}

#[test]
fn hash_embed_text_normalized() {
    let v = sa::memory::hash_embed_text("some text for testing", 128);
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "vector should be normalized, got norm={norm}"
    );
}

// =========================================================================
// memory/mod.rs — format_memory_parts & build_highlights
// =========================================================================

#[test]
fn format_memory_parts_empty() {
    let parts =
        sa::memory::format_memory_parts(&[], &[], |e| e.ticker.clone(), |e| e.ticker.clone());
    assert!(parts.is_empty());
}

#[test]
fn format_memory_parts_same_only() {
    use sa::memory::MemoryEntry;
    let entries = vec![MemoryEntry {
        ticker: "AAPL".to_string(),
        ..MemoryEntry::default()
    }];
    let parts =
        sa::memory::format_memory_parts(&entries, &[], |e| e.ticker.clone(), |e| e.ticker.clone());
    assert!(!parts.is_empty());
    assert!(parts[0].contains("AAPL"));
}

#[test]
fn build_highlights_empty() {
    let (same, cross) =
        sa::memory::build_highlights(&[], &[], |_, _| sa::HistoricalMemoryHighlight::default());
    assert!(same.is_empty());
    assert!(cross.is_empty());
}

#[test]
fn build_highlights_limits_to_three() {
    use sa::memory::MemoryEntry;
    let entries: Vec<_> = (0..5)
        .map(|i| MemoryEntry {
            ticker: format!("T{i}"),
            ..MemoryEntry::default()
        })
        .collect();
    let (same, cross) = sa::memory::build_highlights(&entries, &entries, |_, _| {
        sa::HistoricalMemoryHighlight::default()
    });
    assert_eq!(same.len(), 3);
    assert_eq!(cross.len(), 3);
}

// =========================================================================
// store/mod.rs — additional coverage
// =========================================================================

#[tokio::test]
async fn in_memory_cache_store_overwrite() {
    use sa::CacheStore;
    let store = sa::store::InMemoryCacheStore::new();
    store.set("key", b"v1", None).await.unwrap();
    store.set("key", b"v2", None).await.unwrap();
    let result = store.get("key").await.unwrap();
    assert_eq!(result, Some(b"v2".to_vec()));
}

#[tokio::test]
async fn in_memory_cache_store_with_ttl() {
    use sa::CacheStore;
    let store = sa::store::InMemoryCacheStore::new();
    store.set("key", b"value", Some(3600)).await.unwrap();
    let result = store.get("key").await.unwrap();
    assert_eq!(result, Some(b"value".to_vec()));
}

// =========================================================================
// data/mod.rs — NewsItem construction
// =========================================================================

#[test]
fn news_item_construction() {
    let item = sa::data::NewsItem {
        title: "Test".to_string(),
        source: "Source".to_string(),
        published_at: "2024-01-01".to_string(),
        url: Some("https://example.com".to_string()),
        summary: "Summary".to_string(),
    };
    assert_eq!(item.title, "Test");
    assert!(item.url.is_some());
}

// =========================================================================
// indicators.rs — technical indicator functions
// =========================================================================

fn make_candles(n: usize, base: f64, trend: f64) -> Vec<sa::analysis::ReportCandle> {
    (0..n)
        .map(|i| {
            let price = base + trend * i as f64;
            sa::analysis::ReportCandle {
                trade_date: format!("2024-01-{:02}", (i % 28) + 1),
                open: price - 0.5,
                high: price + 1.0,
                low: price - 1.0,
                close: price,
                volume: 1000 + i as i64 * 100,
                amount: 100000.0,
                amplitude_pct: 2.0,
                change_pct: trend,
                change_amount: trend,
                turnover_pct: 1.0,
            }
        })
        .collect()
}

#[test]
fn sma_basic() {
    let candles = make_candles(10, 100.0, 1.0);
    let result = sa::indicators::sma(&candles, 5);
    assert!(result.is_some());
    // Last 5 closes: 105, 106, 107, 108, 109 → avg = 107
    assert!((result.unwrap() - 107.0).abs() < 0.01);
}

#[test]
fn sma_insufficient_data() {
    let candles = make_candles(3, 100.0, 1.0);
    assert!(sa::indicators::sma(&candles, 5).is_none());
}

#[test]
fn ema_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::ema(&candles, 10);
    assert!(result.is_some());
    assert!(result.unwrap() > 100.0);
}

#[test]
fn ema_insufficient_data() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::ema(&candles, 10).is_none());
}

#[test]
fn ema_series_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::ema_series(&candles, 10);
    assert!(result.is_some());
    let series = result.unwrap();
    assert_eq!(series.len(), 11); // 20 - 10 + 1
}

#[test]
fn ema_series_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::ema_series(&candles, 10).is_none());
}

#[test]
fn ema_values_basic() {
    let values: Vec<f64> = (0..20).map(|i| 100.0 + i as f64 * 0.5).collect();
    let result = sa::indicators::ema_values(&values, 10);
    assert!(result.is_some());
    assert_eq!(result.unwrap().len(), 11);
}

#[test]
fn ema_values_insufficient() {
    let values = vec![1.0, 2.0, 3.0];
    assert!(sa::indicators::ema_values(&values, 5).is_none());
}

#[test]
fn rsi_basic() {
    let candles = make_candles(20, 100.0, 1.0);
    let result = sa::indicators::rsi(&candles, 14);
    assert!(result.is_some());
    let rsi = result.unwrap();
    assert!(rsi > 50.0 && rsi <= 100.0, "RSI={rsi}");
}

#[test]
fn rsi_all_gains() {
    let candles = make_candles(20, 100.0, 2.0);
    let result = sa::indicators::rsi(&candles, 14);
    assert_eq!(result.unwrap(), 100.0);
}

#[test]
fn rsi_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::rsi(&candles, 14).is_none());
}

#[test]
fn atr_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::atr(&candles, 14);
    assert!(result.is_some());
    assert!(result.unwrap() > 0.0);
}

#[test]
fn atr_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::atr(&candles, 14).is_none());
}

#[test]
fn vwma_basic() {
    let candles = make_candles(10, 100.0, 1.0);
    let result = sa::indicators::vwma(&candles, 5);
    assert!(result.is_some());
    assert!(result.unwrap() > 0.0);
}

#[test]
fn vwma_insufficient() {
    let candles = make_candles(3, 100.0, 1.0);
    assert!(sa::indicators::vwma(&candles, 5).is_none());
}

#[test]
fn bollinger_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::bollinger(&candles, 10);
    assert!(result.is_some());
    let (mid, upper, lower) = result.unwrap();
    assert!(upper > mid);
    assert!(lower < mid);
}

#[test]
fn bollinger_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::bollinger(&candles, 10).is_none());
}

#[test]
fn macd_basic() {
    let candles = make_candles(40, 100.0, 0.5);
    let result = sa::indicators::macd(&candles);
    assert!(result.is_some());
    let (macd, signal, hist) = result.unwrap();
    // hist = macd - signal
    assert!((hist - (macd - signal)).abs() < 0.01);
}

#[test]
fn macd_insufficient() {
    let candles = make_candles(20, 100.0, 1.0);
    assert!(sa::indicators::macd(&candles).is_none());
}

#[test]
fn kdj_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::kdj(&candles, 9);
    assert!(result.is_some());
    let (k, d, j) = result.unwrap();
    assert!((j - (3.0 * k - 2.0 * d)).abs() < 0.01);
}

#[test]
fn kdj_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::kdj(&candles, 9).is_none());
}

#[test]
fn cci_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::cci(&candles, 14);
    assert!(result.is_some());
}

#[test]
fn cci_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::cci(&candles, 14).is_none());
}

#[test]
fn wr_basic() {
    let candles = make_candles(20, 100.0, 0.5);
    let result = sa::indicators::wr(&candles, 14);
    assert!(result.is_some());
    let wr = result.unwrap();
    assert!(wr >= -100.0 && wr <= 0.0, "WR={wr}");
}

#[test]
fn wr_insufficient() {
    let candles = make_candles(5, 100.0, 1.0);
    assert!(sa::indicators::wr(&candles, 14).is_none());
}

#[test]
fn adx_basic() {
    let candles = make_candles(30, 100.0, 0.5);
    let result = sa::indicators::adx(&candles, 14);
    assert!(result.is_some());
    let adx = result.unwrap();
    assert!(adx >= 0.0 && adx <= 100.0, "ADX={adx}");
}

#[test]
fn adx_insufficient() {
    let candles = make_candles(10, 100.0, 1.0);
    assert!(sa::indicators::adx(&candles, 14).is_none());
}

#[test]
fn obv_basic() {
    let candles = make_candles(10, 100.0, 1.0);
    let result = sa::indicators::obv(&candles);
    assert!(result.is_some());
    let (obv, delta) = result.unwrap();
    // With upward trend, OBV should be positive
    assert!(obv > 0.0, "OBV={obv}");
    assert!(delta > 0.0);
}

#[test]
fn obv_insufficient() {
    let candles = make_candles(1, 100.0, 1.0);
    assert!(sa::indicators::obv(&candles).is_none());
}

#[test]
fn vwap_basic() {
    let candles = make_candles(10, 100.0, 1.0);
    let result = sa::indicators::vwap(&candles, 5);
    assert!(result.is_some());
    assert!(result.unwrap() > 0.0);
}

#[test]
fn vwap_insufficient() {
    let candles = make_candles(3, 100.0, 1.0);
    assert!(sa::indicators::vwap(&candles, 5).is_none());
}

#[test]
fn candle_volume_ratio_basic() {
    let candles = make_candles(10, 100.0, 1.0);
    let result = sa::indicators::candle_volume_ratio(&candles, 5);
    assert!(result.is_some());
    assert!(result.unwrap() > 0.0);
}

#[test]
fn candle_volume_ratio_insufficient() {
    let candles = make_candles(3, 100.0, 1.0);
    assert!(sa::indicators::candle_volume_ratio(&candles, 5).is_none());
}

// =========================================================================
// indicators.rs — CandleLike trait on ReportCandle
// =========================================================================

#[test]
fn candle_like_trait() {
    use sa::indicators::CandleLike;
    let candle = sa::analysis::ReportCandle {
        trade_date: "2024-01-01".to_string(),
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 102.0,
        volume: 5000,
        amount: 500000.0,
        amplitude_pct: 10.0,
        change_pct: 2.0,
        change_amount: 2.0,
        turnover_pct: 1.0,
    };
    assert_eq!(candle.close(), 102.0);
    assert_eq!(candle.high(), 105.0);
    assert_eq!(candle.low(), 95.0);
    assert_eq!(candle.volume(), 5000);
}

// =========================================================================
// scoring/scorer.rs — more weighted_total edge cases
// =========================================================================

#[test]
fn weighted_total_all_zero() {
    let weights = sa::scoring::types::ScoreWeights::default();
    let zero = sa::scoring::DimensionScore {
        score: 0,
        reason: String::new(),
    };
    let total = sa::scoring::scorer::weighted_total(&weights, &zero, &zero, &zero, &zero);
    assert_eq!(total, 0);
}

#[test]
fn weighted_total_all_hundred() {
    let weights = sa::scoring::types::ScoreWeights::default();
    let hundred = sa::scoring::DimensionScore {
        score: 100,
        reason: String::new(),
    };
    let total =
        sa::scoring::scorer::weighted_total(&weights, &hundred, &hundred, &hundred, &hundred);
    assert_eq!(total, 100);
}

// =========================================================================
// analysis/report_types — type construction coverage
// =========================================================================

#[test]
fn structured_report_default() {
    let report = sa::analysis::StructuredReport::default();
    // StructuredReport has nested fields, just verify it constructs
    let _ = format!("{:?}", report);
}

#[test]
fn calibration_profile_default() {
    let profile = sa::scoring::CalibrationProfile::default();
    assert_eq!(profile.sample_count, 0);
}

// =========================================================================
// scoring/score_types — DimensionScore and StockScore
// =========================================================================

#[test]
fn dimension_score_construction() {
    let ds = sa::scoring::DimensionScore {
        score: 42,
        reason: "test".to_string(),
    };
    assert_eq!(ds.score, 42);
    assert_eq!(ds.reason, "test");
}

#[test]
fn score_weights_label_mapping() {
    assert_eq!(sa::scoring::types::score_label(0), "avoid");
    assert_eq!(sa::scoring::types::score_label(100), "strong_buy");
}

// =========================================================================
// scoring/config.rs — ScoreConfig::from_env
// =========================================================================

#[test]
fn score_config_from_env_default() {
    // Without env vars set, should return defaults
    let config = sa::scoring::config::ScoreConfig::from_env();
    assert_eq!(config.sentiment_news_limit, 10);
    assert_eq!(config.weights.technical, 30);
}

#[test]
fn score_config_from_env_with_vars() {
    // This test covers the env var parsing branches
    // Note: env vars may or may not be set in CI, so we just verify it doesn't panic
    let _config = sa::scoring::config::ScoreConfig::from_env();
}

// =========================================================================
// llm/retry.rs — default_retry_hint_builder
// =========================================================================

#[test]
fn default_retry_hint_builder_basic() {
    let issues = vec![sa::llm::parse::DiagnosisIssue {
        severity: sa::llm::parse::IssueSeverity::Error,
        category: "missing".to_string(),
        field: "summary".to_string(),
        message: "Field is empty".to_string(),
    }];
    let hint = sa::llm::retry::default_retry_hint_builder(&issues, 1);
    assert!(hint.contains("retry 1"));
    assert!(hint.contains("summary"));
    assert!(hint.contains("Field is empty"));
}

#[test]
fn default_retry_hint_builder_empty_issues() {
    let hint = sa::llm::retry::default_retry_hint_builder(&[], 3);
    assert!(hint.contains("retry 3"));
}

// =========================================================================
// llm/parse/diagnosis.rs — DiagnosisIssue constructors
// =========================================================================

#[test]
fn diagnosis_issue_error() {
    let issue = sa::llm::parse::DiagnosisIssue::error("missing", "field1", "message1");
    assert!(matches!(
        issue.severity,
        sa::llm::parse::IssueSeverity::Error
    ));
    assert_eq!(issue.category, "missing");
    assert_eq!(issue.field, "field1");
    assert_eq!(issue.message, "message1");
}

#[test]
fn diagnosis_issue_warning() {
    let issue = sa::llm::parse::DiagnosisIssue::warning("quality", "field2", "message2");
    assert!(matches!(
        issue.severity,
        sa::llm::parse::IssueSeverity::Warning
    ));
}

#[test]
fn diagnosis_issue_info() {
    let issue = sa::llm::parse::DiagnosisIssue::info("info", "field3", "message3");
    assert!(matches!(
        issue.severity,
        sa::llm::parse::IssueSeverity::Info
    ));
}

// =========================================================================
// analysis/report_types/decision.rs — LocalText
// =========================================================================

#[test]
fn local_text_new() {
    let lt = sa::analysis::LocalText::new("test_key");
    assert_eq!(lt.as_str(), "test_key");
    assert!(!lt.is_empty());
}

#[test]
fn local_text_empty() {
    let lt = sa::analysis::LocalText::new("");
    assert!(lt.is_empty());
}

#[test]
fn local_text_with_params() {
    let lt = sa::analysis::LocalText::new("key")
        .with_str("name", "value")
        .with_f64("price", 100.0)
        .with_i32("count", 5)
        .with_bool("active", true)
        .with_param("extra", serde_json::json!([1, 2]));
    assert_eq!(lt.as_str(), "key");
}

#[test]
fn local_text_from_str() {
    let lt: sa::analysis::LocalText = "hello".into();
    assert_eq!(lt.as_str(), "hello");
}

#[test]
fn local_text_from_string() {
    let lt: sa::analysis::LocalText = "hello".to_string().into();
    assert_eq!(lt.as_str(), "hello");
}

#[test]
fn local_text_display() {
    let lt = sa::analysis::LocalText::new("test");
    assert_eq!(format!("{lt}"), "test");
}

#[test]
fn local_text_partial_eq() {
    let a = sa::analysis::LocalText::new("same");
    let b = sa::analysis::LocalText::new("same");
    let c = sa::analysis::LocalText::new("different");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn local_text_trim() {
    let lt = sa::analysis::LocalText::new("  hello  ");
    assert_eq!(lt.trim(), "hello");
}

#[test]
fn local_text_split() {
    let lt = sa::analysis::LocalText::new("a,b,c");
    let parts: Vec<&str> = lt.split(",").collect();
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn local_text_contains() {
    let lt = sa::analysis::LocalText::new("hello world");
    assert!(lt.contains("world"));
    assert!(!lt.contains("xyz"));
}

#[test]
fn local_text_starts_with() {
    let lt = sa::analysis::LocalText::new("hello world");
    assert!(lt.starts_with("hello"));
    assert!(!lt.starts_with("world"));
}

#[test]
fn local_text_to_ascii_lowercase() {
    let lt = sa::analysis::LocalText::new("Hello World");
    assert_eq!(lt.to_ascii_lowercase(), "hello world");
}

// =========================================================================
// analysis/report_types/decision.rs — Rating
// =========================================================================

#[test]
fn rating_parse_all() {
    assert!(matches!(
        sa::analysis::Rating::parse("Buy"),
        sa::analysis::Rating::Buy
    ));
    assert!(matches!(
        sa::analysis::Rating::parse("Overweight"),
        sa::analysis::Rating::Overweight
    ));
    assert!(matches!(
        sa::analysis::Rating::parse("Hold"),
        sa::analysis::Rating::Hold
    ));
    assert!(matches!(
        sa::analysis::Rating::parse("Underweight"),
        sa::analysis::Rating::Underweight
    ));
    assert!(matches!(
        sa::analysis::Rating::parse("Sell"),
        sa::analysis::Rating::Sell
    ));
    assert!(matches!(
        sa::analysis::Rating::parse("unknown"),
        sa::analysis::Rating::Hold
    ));
}

#[test]
fn rating_is_bullish() {
    assert!(sa::analysis::Rating::Buy.is_bullish());
    assert!(sa::analysis::Rating::Overweight.is_bullish());
    assert!(!sa::analysis::Rating::Hold.is_bullish());
    assert!(!sa::analysis::Rating::Sell.is_bullish());
}

#[test]
fn rating_is_bearish() {
    assert!(sa::analysis::Rating::Sell.is_bearish());
    assert!(sa::analysis::Rating::Underweight.is_bearish());
    assert!(!sa::analysis::Rating::Hold.is_bearish());
    assert!(!sa::analysis::Rating::Buy.is_bearish());
}

#[test]
fn rating_is_neutral() {
    assert!(sa::analysis::Rating::Hold.is_neutral());
    assert!(!sa::analysis::Rating::Buy.is_neutral());
    assert!(!sa::analysis::Rating::Sell.is_neutral());
}

#[test]
fn rating_bias() {
    assert_eq!(sa::analysis::Rating::Buy.bias(100), 100);
    assert_eq!(sa::analysis::Rating::Overweight.bias(100), 75);
    assert_eq!(sa::analysis::Rating::Hold.bias(100), 0);
    assert_eq!(sa::analysis::Rating::Underweight.bias(100), -75);
    assert_eq!(sa::analysis::Rating::Sell.bias(100), -100);
}

#[test]
fn rating_to_score() {
    assert_eq!(sa::analysis::Rating::Buy.to_score(), 2);
    assert_eq!(sa::analysis::Rating::Overweight.to_score(), 1);
    assert_eq!(sa::analysis::Rating::Hold.to_score(), 0);
    assert_eq!(sa::analysis::Rating::Underweight.to_score(), -1);
    assert_eq!(sa::analysis::Rating::Sell.to_score(), -2);
}

#[test]
fn rating_to_action_group() {
    assert_eq!(sa::analysis::Rating::Buy.to_action_group(), "Buy");
    assert_eq!(sa::analysis::Rating::Overweight.to_action_group(), "Buy");
    assert_eq!(sa::analysis::Rating::Hold.to_action_group(), "Hold");
    assert_eq!(sa::analysis::Rating::Sell.to_action_group(), "Sell");
    assert_eq!(sa::analysis::Rating::Underweight.to_action_group(), "Sell");
}

#[test]
fn rating_display() {
    assert_eq!(format!("{}", sa::analysis::Rating::Buy), "Buy");
    assert_eq!(format!("{}", sa::analysis::Rating::Hold), "Hold");
    assert_eq!(format!("{}", sa::analysis::Rating::Sell), "Sell");
}

#[test]
fn rating_default() {
    let r = sa::analysis::Rating::default();
    assert!(matches!(r, sa::analysis::Rating::Hold));
}

// =========================================================================
// analysis/derived.rs — AnalysisResult derived methods
// =========================================================================

#[test]
fn analysis_result_derived_methods() {
    let result = sa::analysis::AnalysisResult {
        task_id: "task1".to_string(),
        report_id: "report1".to_string(),
        symbol: "AAPL".to_string(),
        stock_name: "Apple Inc.".to_string(),
        analysis_date: "2024-01-15".to_string(),
        market_type: "美股".to_string(),
        graph: sa::analysis::AnalysisGraph::default(),
        agent_state: sa::analysis::AgentStateSnapshot::default(),
        artifacts: sa::analysis::AnalysisArtifacts::default(),
        report: sa::analysis::StructuredReport::default(),
        ic_report: sa::analysis::StructuredReport::default(),
        created_at: "2024-01-15T10:00:00Z".to_string(),
    };

    // Test derived methods with empty portfolio decision (should fall through to defaults)
    let summary = result.derived_summary();
    assert!(!summary.is_empty());

    let recommendation = result.derived_recommendation();
    assert!(!recommendation.is_empty());

    let risk = result.derived_risk_assessment();
    assert!(!risk.is_empty());

    let _confidence = result.derived_confidence();

    let _rationale = result.derived_rationale();
}

// =========================================================================
// analysis/report_logic/setup_quality.rs — gap utilities
// =========================================================================

#[test]
fn normalize_gap_match_text_basic() {
    let result = sa::analysis::normalize_gap_match_text("Hello, World.");
    assert_eq!(result, "hello  world ");
}

#[test]
fn normalize_gap_match_text_chinese() {
    let result = sa::analysis::normalize_gap_match_text("测试，内容：");
    assert_eq!(result, "测试 内容 ");
}

#[test]
fn tokenize_gap_match_text_basic() {
    let tokens = sa::analysis::tokenize_gap_match_text("hello world test");
    assert_eq!(tokens, vec!["hello", "world", "test"]);
}

#[test]
fn tokenize_gap_match_text_short_tokens() {
    let tokens = sa::analysis::tokenize_gap_match_text("a bb ccc");
    assert_eq!(tokens, vec!["bb", "ccc"]);
}

#[test]
fn tokenize_gap_match_text_empty() {
    let tokens = sa::analysis::tokenize_gap_match_text("");
    assert!(tokens.is_empty());
}

#[test]
fn score_related_gap_match_basic() {
    let base = vec!["missing".to_string(), "fundamentals".to_string()];
    let score = sa::analysis::score_related_gap_match(&base, "missing fundamentals data");
    assert_eq!(score, 2);
}

#[test]
fn score_related_gap_match_partial() {
    let base = vec!["missing".to_string(), "fundamentals".to_string()];
    let score = sa::analysis::score_related_gap_match(&base, "missing data");
    assert_eq!(score, 1);
}

#[test]
fn score_related_gap_match_none() {
    let base = vec!["missing".to_string(), "fundamentals".to_string()];
    let score = sa::analysis::score_related_gap_match(&base, "no match here");
    assert_eq!(score, 0);
}

// =========================================================================
// analysis/report_types/graph.rs — DiagnosisSummary
// =========================================================================

#[test]
fn diagnosis_summary_from_issues() {
    let issues = vec![
        sa::analysis::DiagnosisIssue {
            severity: "error".to_string(),
            check_name: "consistency".to_string(),
            field: "entry_price".to_string(),
            original_value: "0".to_string(),
            fixed_value: "100.0".to_string(),
            message: "Missing entry price".to_string(),
        },
        sa::analysis::DiagnosisIssue {
            severity: "warning".to_string(),
            check_name: "format".to_string(),
            field: "summary".to_string(),
            original_value: "".to_string(),
            fixed_value: "".to_string(),
            message: "Empty summary".to_string(),
        },
    ];
    let summary = sa::analysis::DiagnosisSummary::from_issues(&issues);
    assert_eq!(summary.total_issues, 2);
    assert_eq!(summary.fixed_count, 1);
    assert_eq!(summary.unfixed_count, 1);
}

#[test]
fn diagnosis_summary_empty() {
    let summary = sa::analysis::DiagnosisSummary::from_issues(&[]);
    assert_eq!(summary.total_issues, 0);
    assert_eq!(summary.fixed_count, 0);
}

// =========================================================================
// llm/generated/helpers.rs — utility functions
// =========================================================================

#[test]
fn is_zero_value_number() {
    assert!(sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!(0)
    ));
    assert!(sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!(0.0)
    ));
    assert!(!sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!(1.0)
    ));
}

#[test]
fn is_zero_value_string() {
    assert!(sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!("0")
    ));
    assert!(sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!("0.0")
    ));
    assert!(!sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!("1")
    ));
}

#[test]
fn is_zero_value_other() {
    assert!(!sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!(true)
    ));
    assert!(!sa::llm::generated::helpers::is_zero_value(
        &serde_json::json!(null)
    ));
}

#[test]
fn is_uniform_distribution_yes() {
    assert!(sa::llm::generated::helpers::is_uniform_distribution(
        &serde_json::json!(0.333),
        &serde_json::json!(0.333),
        &serde_json::json!(0.333),
    ));
}

#[test]
fn is_uniform_distribution_no() {
    assert!(!sa::llm::generated::helpers::is_uniform_distribution(
        &serde_json::json!(0.6),
        &serde_json::json!(0.3),
        &serde_json::json!(0.1),
    ));
}

#[test]
fn is_meaningful_value_various() {
    assert!(!sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!(null)
    ));
    assert!(!sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!("")
    ));
    assert!(!sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!("  ")
    ));
    assert!(sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!("hello")
    ));
    assert!(sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!(42)
    ));
    assert!(sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!(true)
    ));
    assert!(sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!([1, 2])
    ));
    assert!(sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!({"a": 1})
    ));
    assert!(!sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!([])
    ));
    assert!(!sa::llm::generated::helpers::is_meaningful_value(
        &serde_json::json!({})
    ));
}

#[test]
fn meaningful_value_some() {
    assert!(
        sa::llm::generated::helpers::meaningful_value(Some(serde_json::json!("test"))).is_some()
    );
    assert!(sa::llm::generated::helpers::meaningful_value(Some(serde_json::json!(null))).is_none());
    assert!(sa::llm::generated::helpers::meaningful_value(None).is_none());
}

#[test]
fn object_value_some() {
    assert!(sa::llm::generated::helpers::object_value(Some(serde_json::json!("test"))).is_some());
    assert!(sa::llm::generated::helpers::object_value(None).is_none());
}

#[test]
fn extract_object_value_basic() {
    let obj = serde_json::json!({"name": "test", "value": 42});
    let result = sa::llm::generated::helpers::extract_object_value(Some(&obj), &["name"]);
    assert_eq!(result, Some(serde_json::json!("test")));
}

#[test]
fn extract_object_value_missing_key() {
    let obj = serde_json::json!({"name": "test"});
    let result = sa::llm::generated::helpers::extract_object_value(Some(&obj), &["missing"]);
    assert!(result.is_none());
}

#[test]
fn extract_object_value_none() {
    let result = sa::llm::generated::helpers::extract_object_value(None, &["key"]);
    assert!(result.is_none());
}

#[test]
fn extract_object_value_not_object() {
    let val = serde_json::json!("not an object");
    let result = sa::llm::generated::helpers::extract_object_value(Some(&val), &["key"]);
    assert!(result.is_none());
}

#[test]
fn extract_object_string_list_basic() {
    let obj = serde_json::json!({"tags": ["a", "b", "c"]});
    let result = sa::llm::generated::helpers::extract_object_string_list(Some(&obj), &["tags"]);
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn extract_object_string_list_missing() {
    let obj = serde_json::json!({"other": "value"});
    let result = sa::llm::generated::helpers::extract_object_string_list(Some(&obj), &["tags"]);
    assert!(result.is_empty());
}

#[test]
fn split_list_like_text_basic() {
    let result = sa::llm::generated::helpers::split_list_like_text("a;b;c");
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn split_list_like_text_multiline() {
    let result = sa::llm::generated::helpers::split_list_like_text("line1\nline2");
    assert_eq!(result, vec!["line1", "line2"]);
}

#[test]
fn split_list_like_text_dedup() {
    let result = sa::llm::generated::helpers::split_list_like_text("a;b;a");
    assert_eq!(result, vec!["a", "b"]);
}

#[test]
fn split_list_like_text_empty() {
    let result = sa::llm::generated::helpers::split_list_like_text("");
    assert!(result.is_empty());
}

#[test]
fn derive_probabilities_from_text_bullish() {
    let (up, down, sideways) = sa::llm::generated::helpers::derive_probabilities_from_text(
        "strong bullish growth positive",
        "",
        "",
    );
    assert!(up > down, "up={up} should be > down={down}");
    assert!((up + down + sideways - 1.0).abs() < 0.01);
}

#[test]
fn derive_probabilities_from_text_bearish() {
    let (up, down, _sideways) = sa::llm::generated::helpers::derive_probabilities_from_text(
        "bearish decline risk negative",
        "",
        "",
    );
    assert!(down > up, "down={down} should be > up={up}");
}

#[test]
fn derive_probabilities_from_text_neutral() {
    let (up, down, _sideways) =
        sa::llm::generated::helpers::derive_probabilities_from_text("", "", "");
    // Default to slightly bearish
    assert!((up + down + _sideways - 1.0).abs() < 0.01);
}

#[test]
fn extract_numbered_trigger_lines_basic() {
    let text = "1) First trigger\n2) Second trigger\n3) Third trigger";
    let lines = sa::llm::generated::helpers::extract_numbered_trigger_lines(text);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "First trigger");
}

#[test]
fn extract_numbered_trigger_lines_empty() {
    let lines = sa::llm::generated::helpers::extract_numbered_trigger_lines("no triggers here");
    assert!(lines.is_empty());
}

#[test]
fn format_price_like_text_integer() {
    assert_eq!(
        sa::llm::generated::helpers::format_price_like_text(100.0),
        "100"
    );
}

#[test]
fn format_price_like_text_large() {
    assert_eq!(
        sa::llm::generated::helpers::format_price_like_text(1234.5),
        "1234.5"
    );
}

#[test]
fn format_price_like_text_small() {
    assert_eq!(
        sa::llm::generated::helpers::format_price_like_text(12.34),
        "12.34"
    );
}

#[test]
fn extract_time_horizon_from_texts_basic() {
    let texts = vec!["目标 2-3 weeks 达成"];
    let result = sa::llm::generated::helpers::extract_time_horizon_from_texts(&texts);
    assert!(result.is_some());
    assert!(result.unwrap().contains("weeks"));
}

#[test]
fn extract_time_horizon_from_texts_none() {
    let texts = vec!["no time horizon"];
    assert!(sa::llm::generated::helpers::extract_time_horizon_from_texts(&texts).is_none());
}

#[test]
fn extract_position_sizing_from_texts_basic() {
    let texts = vec!["建仓 5% 仓位"];
    let result = sa::llm::generated::helpers::extract_position_sizing_from_texts(&texts);
    assert!(result.is_some());
    assert!(result.unwrap().contains("%"));
}

#[test]
fn extract_position_sizing_from_texts_none() {
    let texts = vec!["no position sizing"];
    assert!(sa::llm::generated::helpers::extract_position_sizing_from_texts(&texts).is_none());
}

// =========================================================================
// llm/parse/json_string.rs — JSON parsing utilities
// =========================================================================

#[test]
fn skip_json_whitespace_basic() {
    assert_eq!(sa::llm::parse::skip_json_whitespace("  hello", 0), 2);
    assert_eq!(sa::llm::parse::skip_json_whitespace("hello", 0), 0);
    assert_eq!(sa::llm::parse::skip_json_whitespace("\t\n  x", 0), 4);
}

#[test]
fn find_json_string_end_basic() {
    assert_eq!(
        sa::llm::parse::find_json_string_end(r#""hello""#, 0),
        Some(6)
    );
    assert_eq!(
        sa::llm::parse::find_json_string_end(r#""he\"llo""#, 0),
        Some(8)
    );
    assert!(sa::llm::parse::find_json_string_end(r#"hello"#, 0).is_none());
}

#[test]
fn find_json_string_end_escaped() {
    assert_eq!(
        sa::llm::parse::find_json_string_end(r#""test\\end""#, 0),
        Some(10)
    );
}

#[test]
fn find_json_value_end_string() {
    assert_eq!(
        sa::llm::parse::find_json_value_end(r#""hello""#, 0),
        Some(6)
    );
}

#[test]
fn find_json_value_end_object() {
    assert_eq!(
        sa::llm::parse::find_json_value_end(r#"{"a":1}"#, 0),
        Some(6)
    );
}

#[test]
fn find_json_value_end_array() {
    assert_eq!(sa::llm::parse::find_json_value_end("[1,2,3]", 0), Some(6));
}

#[test]
fn find_json_value_end_number() {
    let result = sa::llm::parse::find_json_value_end("42,", 0);
    assert_eq!(result, Some(1));
}

#[test]
fn decode_json_string_literal_basic() {
    let result = sa::llm::parse::decode_json_string_literal(r#""hello world""#).unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn decode_json_string_literal_escaped() {
    let result = sa::llm::parse::decode_json_string_literal(r#""he\"llo""#).unwrap();
    assert_eq!(result, "he\"llo");
}

#[test]
fn extract_simple_json_string_field_basic() {
    let json = r#"{"name":"test","value":42}"#;
    let result = sa::llm::parse::extract_simple_json_string_field(json, "name");
    assert_eq!(result, Some("test".to_string()));
}

#[test]
fn extract_simple_json_string_field_missing() {
    let json = r#"{"name":"test"}"#;
    let result = sa::llm::parse::extract_simple_json_string_field(json, "missing");
    assert!(result.is_none());
}

#[test]
fn normalize_relaxed_json_string_basic() {
    assert_eq!(
        sa::llm::parse::normalize_relaxed_json_string("hello"),
        "hello"
    );
    assert_eq!(
        sa::llm::parse::normalize_relaxed_json_string("he\"llo"),
        "he\"llo"
    );
}

#[test]
fn normalize_relaxed_json_string_escapes() {
    assert_eq!(
        sa::llm::parse::normalize_relaxed_json_string("line1\\nline2"),
        "line1\nline2"
    );
    assert_eq!(
        sa::llm::parse::normalize_relaxed_json_string("tab\\there"),
        "tab\there"
    );
}

// =========================================================================
// llm/client/types.rs — token estimation
// =========================================================================

#[test]
fn approximate_tokens_from_chars_basic() {
    let tokens = sa::llm::client::approximate_tokens_from_chars(100);
    assert!(tokens > 0);
    assert!(tokens < 100);
}
