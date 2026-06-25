use sa::analysis::{
    AnalysisUserContext, HistoricalMemoryHighlight, LlmTokenUsageByModel, LlmTokenUsageSummary,
    MemoryContextSnapshot,
};

#[test]
fn analysis_user_context_serde_roundtrip() {
    let c = AnalysisUserContext {
        language: "zh".into(),
        position_state: "holding".into(),
        workflow_intent: "analysis".into(),
        holding_cost: Some(150.0),
        holding_ratio_pct: Some(5.0),
        risk_preference: "medium".into(),
        investment_horizon: "swing".into(),
        notes: "test".into(),
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: AnalysisUserContext = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.language, "zh");
    assert_eq!(restored.holding_cost, Some(150.0));
}

#[test]
fn llm_token_usage_summary_serde_roundtrip() {
    let u = LlmTokenUsageSummary {
        total_requests: 10,
        prompt_tokens: 5000,
        completion_tokens: 2000,
        total_tokens: 7000,
        by_model: vec![LlmTokenUsageByModel {
            model: "claude".into(),
            requests: 10,
            prompt_tokens: 5000,
            completion_tokens: 2000,
            total_tokens: 7000,
        }],
    };
    let json = serde_json::to_string(&u).unwrap();
    let restored: LlmTokenUsageSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.total_requests, 10);
    assert_eq!(restored.by_model.len(), 1);
}

#[test]
fn memory_context_snapshot_serde_roundtrip() {
    let m = MemoryContextSnapshot {
        source: "vector".into(),
        retrieval_mode: "hybrid".into(),
        embedding_provider: "openai".into(),
        embedding_failure_reason: None,
        same_ticker_count: 3,
        cross_ticker_count: 5,
        vector_hit_count: 8,
        effective_top_k: 10,
        market_sample_count: 20,
        used_market_profile: true,
        setup_tags: vec!["breakout".into()],
        resolved_setup_tags: vec!["breakout".into()],
        used_setup_filtered_retrieval: true,
        used_setup_fallback_calibration: false,
        setup_calibration_sample_count: 5,
        setup_match_count: 3,
        setup_pending_match_count: 1,
        setup_resolved_match_count: 2,
        setup_match_hit_rate: 0.65,
        setup_match_avg_alpha_return: 0.04,
        setup_long_match_count: 2,
        setup_short_match_count: 1,
        setup_neutral_match_count: 0,
        historical_same_ticker_highlights: vec![],
        historical_cross_ticker_highlights: vec![],
        context_excerpt: "excerpt".into(),
    };
    let json = serde_json::to_string(&m).unwrap();
    let restored: MemoryContextSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.same_ticker_count, 3);
    assert!(restored.used_market_profile);
}

#[test]
fn historical_memory_highlight_serde_roundtrip() {
    let h = HistoricalMemoryHighlight {
        trade_date: "2025-01-01".into(),
        ticker: "AAPL".into(),
        rating: "Buy".into(),
        action: "buy".into(),
        summary: "good".into(),
        key_risk: "risk".into(),
        lesson: "lesson".into(),
        same_ticker: true,
    };
    let json = serde_json::to_string(&h).unwrap();
    let restored: HistoricalMemoryHighlight = serde_json::from_str(&json).unwrap();
    assert!(restored.same_ticker);
    assert_eq!(restored.ticker, "AAPL");
}

#[test]
fn llm_token_usage_by_model_serde_roundtrip() {
    let m = LlmTokenUsageByModel {
        model: "gpt-4".into(),
        requests: 5,
        prompt_tokens: 3000,
        completion_tokens: 1000,
        total_tokens: 4000,
    };
    let json = serde_json::to_string(&m).unwrap();
    let restored: LlmTokenUsageByModel = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.model, "gpt-4");
}

#[test]
fn all_defaults() {
    assert!(AnalysisUserContext::default().language.is_empty());
    assert_eq!(LlmTokenUsageSummary::default().total_requests, 0);
    assert_eq!(LlmTokenUsageByModel::default().requests, 0);
    assert_eq!(MemoryContextSnapshot::default().same_ticker_count, 0);
    assert!(HistoricalMemoryHighlight::default().trade_date.is_empty());
}
