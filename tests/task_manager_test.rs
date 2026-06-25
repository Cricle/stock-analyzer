use sa::memory::MemoryContextBundleWithTags;
use sa::task_manager::{
    TaskManager, TaskRunParams, memory_snapshot_from_bundle, seconds_until_local_midnight,
};
use sa::{AnalysisScenarioContext, AnalysisUserContext, MemoryContextSnapshot};

#[test]
fn memory_snapshot_from_bundle_maps_fields() {
    let bundle = MemoryContextBundleWithTags {
        context_text: "test context".into(),
        source: "vector".into(),
        retrieval_mode: "hybrid".into(),
        embedding_provider: "openai".into(),
        embedding_failure_reason: None,
        same_ticker_count: 3,
        cross_ticker_count: 5,
        vector_hit_count: 8,
        effective_top_k: 10,
        same_ticker_highlights: vec![],
        cross_ticker_highlights: vec![],
        setup_tags: vec!["trend".into()],
        used_setup_filtered_retrieval: true,
        used_setup_fallback_calibration: false,
        setup_calibration_sample_count: 10,
        setup_match_count: 5,
        setup_pending_match_count: 2,
        setup_resolved_match_count: 3,
        setup_match_hit_rate: 0.7,
        setup_match_avg_alpha_return: 0.05,
        setup_long_match_count: 4,
        setup_short_match_count: 1,
        setup_neutral_match_count: 0,
    };
    let snapshot = memory_snapshot_from_bundle(&bundle);
    assert_eq!(snapshot.source, "vector");
    assert_eq!(snapshot.retrieval_mode, "hybrid");
    assert_eq!(snapshot.same_ticker_count, 3);
    assert_eq!(snapshot.cross_ticker_count, 5);
    assert_eq!(snapshot.vector_hit_count, 8);
    assert!(snapshot.used_setup_filtered_retrieval);
    assert_eq!(snapshot.setup_match_hit_rate, 0.7);
}

#[test]
fn memory_snapshot_from_bundle_truncates_context() {
    let long_text = "x".repeat(2000);
    let bundle = MemoryContextBundleWithTags {
        context_text: long_text,
        ..Default::default()
    };
    let snapshot = memory_snapshot_from_bundle(&bundle);
    assert_eq!(snapshot.context_excerpt.len(), 1200);
}

#[test]
fn memory_snapshot_from_bundle_default_bundle() {
    let bundle = MemoryContextBundleWithTags::default();
    let snapshot = memory_snapshot_from_bundle(&bundle);
    assert!(snapshot.source.is_empty());
    assert_eq!(snapshot.same_ticker_count, 0);
}

#[test]
fn seconds_until_local_midnight_positive() {
    let result = seconds_until_local_midnight();
    assert!(result > 0, "expected positive seconds, got {}", result);
    assert!(result <= 86400, "expected <= 86400 seconds, got {}", result);
}

#[test]
fn analysis_reuse_cache_key_format() {
    let key = TaskManager::analysis_reuse_cache_key("Alice", "nvda", "US");
    assert_eq!(key, "analysis:reuse:alice:NVDA:US");
}

#[test]
fn analysis_reuse_cache_key_trims_whitespace() {
    let key = TaskManager::analysis_reuse_cache_key("  Bob  ", "  tsla  ", "  HK  ");
    assert_eq!(key, "analysis:reuse:bob:TSLA:HK");
}

#[test]
fn task_run_params_for_reflection_defaults() {
    let params = TaskRunParams::for_reflection("2026-01-15".into(), "zh");
    assert_eq!(params.analysis_date, "2026-01-15");
    assert_eq!(params.language, "zh");
    assert_eq!(params.market_type, "unknown");
    assert!(params.selected_analysts.is_empty());
    assert!(params.past_context.is_empty());
    assert!(params.llm_base_url.is_none());
    assert!(params.llm_api_key.is_none());
}

#[test]
fn task_run_params_for_reflection_with_llm_inherits_settings() {
    let base = TaskRunParams {
        market_type: "US".into(),
        analysis_date: "2026-01-01".into(),
        scenario: AnalysisScenarioContext::from_market_type("US"),
        selected_analysts: vec!["market".into()],
        past_context: "ctx".into(),
        memory_context: MemoryContextSnapshot::default(),
        llm_base_url: Some("https://api.llm.com".into()),
        llm_api_key: Some("sk-key".into()),
        quick_analysis_model: Some("gpt-4".into()),
        deep_analysis_model: Some("claude-3".into()),
        language: "en".into(),
        user_context: AnalysisUserContext::default(),
        user_context_prompt: "prompt".into(),
        sector_context: "sector".into(),
    };
    let params = TaskRunParams::for_reflection_with_llm("2026-06-01".into(), "zh", &base);
    assert_eq!(params.analysis_date, "2026-06-01");
    assert_eq!(params.language, "zh");
    assert_eq!(params.llm_base_url, Some("https://api.llm.com".into()));
    assert_eq!(params.llm_api_key, Some("sk-key".into()));
    assert_eq!(params.quick_analysis_model, Some("gpt-4".into()));
    assert_eq!(params.deep_analysis_model, Some("claude-3".into()));
    assert_eq!(params.market_type, "unknown");
    assert!(params.selected_analysts.is_empty());
}

#[test]
fn task_run_params_for_reflection_empty_language() {
    let params = TaskRunParams::for_reflection("2026-01-01".into(), "");
    assert_eq!(params.language, "");
}
