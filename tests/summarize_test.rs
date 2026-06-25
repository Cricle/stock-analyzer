use sa::report::runtime::summarize_stock_data_output;

#[test]
fn summarize_stock_data_output_supports_pre_summarized_payload() {
    let output = r#"{
      "symbol": "600036",
      "market_type": "股",
      "start_date": "2025-05-01",
      "end_date": "2026-05-27",
      "row_count": 258,
      "first_trade_date": "2025-05-06",
      "last_trade_date": "2026-05-27",
      "first_close": 38.36,
      "last_close": 36.86,
      "high_max": 45.54,
      "low_min": 36.78,
      "volume_sum": 196210100.0,
      "data_gap": null
    }"#;

    let summary = summarize_stock_data_output(output);

    assert!(summary.contains("rows: 258"));
    assert!(summary.contains("first_close: 38.36"));
    assert!(summary.contains("last_close: 36.86"));
    assert!(!summary.contains("rows: 0"));
}
