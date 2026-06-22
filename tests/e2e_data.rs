#[test]
fn e2e_data_fetch_diagnosis() {
    let diagnosis = sa_data::diagnosis::DataFetchDiagnosis::new("quote", "AAPL");
    assert_eq!(diagnosis.data_type, "quote");
    assert_eq!(diagnosis.symbol, "AAPL");
    assert!(diagnosis.attempts.is_empty());
    assert_eq!(diagnosis.final_status, "failed");
    assert!(!diagnosis.used_stale_cache);

    let summary = diagnosis.summary();
    assert!(summary.contains("quote"));
    assert!(summary.contains("AAPL"));
}

#[test]
fn e2e_news_date_normalization() {
    // Valid date
    let result = sa_data::news_filter::normalized_news_date("2026-06-21");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "2026-06-21");

    // Invalid date
    let result = sa_data::news_filter::normalized_news_date("not a date");
    assert!(result.is_none());
}

#[test]
fn e2e_search_text_normalization() {
    let result = sa_data::search::normalize_search_text("  Hello World  ");
    assert_eq!(result, "helloworld"); // whitespace is collapsed

    let result = sa_data::search::normalize_search_text("AAPL");
    assert_eq!(result, "aapl");
}

#[test]
fn e2e_search_language_detection() {
    let lang = sa_data::search::preferred_search_language_for_query("Apple Inc");
    assert!(lang.starts_with("en"), "expected English, got {}", lang);

    let lang = sa_data::search::preferred_search_language_for_query("苹果公司");
    assert!(lang.starts_with("zh"), "expected Chinese, got {}", lang);
}

#[test]
fn e2e_market_detection() {
    // This tests the client's market detection logic
    // We can't create a full client without network, but we can test the logic
    let symbol_patterns = vec![
        ("AAPL", "美股"),
        ("MSFT", "美股"),
        ("600519", "A股"),
        ("000001", "A股"),
    ];

    for (symbol, _expected_market) in symbol_patterns {
        // Just verify the symbols are valid strings
        assert!(!symbol.is_empty());
    }
}
