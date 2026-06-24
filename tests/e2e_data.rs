#[test]
fn e2e_data_fetch_diagnosis() {
    let diagnosis = sa_data::DataFetchDiagnosis::new("quote", "AAPL");
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
    let result = sa_data::normalized_news_date("2026-06-21");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "2026-06-21");

    // Invalid date
    let result = sa_data::normalized_news_date("not a date");
    assert!(result.is_none());
}
