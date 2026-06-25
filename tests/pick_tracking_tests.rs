use sa::pick::tracking::{AlphaTrackingRecord, AlphaTrackingSummary, summarize_tracking};

fn make_record(
    id: &str,
    status: &str,
    alpha: Option<f64>,
    manual: Option<f64>,
) -> AlphaTrackingRecord {
    AlphaTrackingRecord {
        id: id.into(),
        run_id: "run-1".into(),
        symbol: "AAPL".into(),
        market: "美股".into(),
        entry_price: 150.0,
        entry_date: "2026-01-01".into(),
        exit_price: None,
        exit_date: None,
        alpha_return: alpha,
        benchmark_return: None,
        tracking_status: status.into(),
        manual_return: manual,
        manual_note: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn summarize_tracking_empty() {
    let summary = summarize_tracking(&[]);
    assert_eq!(summary.total_picks, 0);
    assert_eq!(summary.tracked_count, 0);
    assert_eq!(summary.pending_count, 0);
    assert!(summary.hit_rate.is_none());
    assert!(summary.average_alpha.is_none());
}

#[test]
fn summarize_tracking_counts_by_status() {
    let records = vec![
        make_record("1", "tracked", Some(5.0), None),
        make_record("2", "tracked", Some(-3.0), None),
        make_record("3", "pending", None, None),
        make_record("4", "manual", None, Some(10.0)),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.total_picks, 4);
    assert_eq!(summary.tracked_count, 2);
    assert_eq!(summary.pending_count, 1);
    assert_eq!(summary.manual_count, 1);
}

#[test]
fn summarize_tracking_hit_rate() {
    let records = vec![
        make_record("1", "tracked", Some(5.0), None),
        make_record("2", "tracked", Some(-3.0), None),
        make_record("3", "tracked", Some(10.0), None),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.hit_count, 2);
    let rate = summary.hit_rate.unwrap();
    assert!((rate - 2.0 / 3.0).abs() < 0.01);
}

#[test]
fn summarize_tracking_uses_manual_return_for_manual_records() {
    let records = vec![
        make_record("1", "manual", None, Some(15.0)),
        make_record("2", "manual", None, Some(-5.0)),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.hit_count, 1);
    let avg = summary.average_alpha.unwrap();
    assert!((avg - 5.0).abs() < 0.01);
}

#[test]
fn summarize_tracking_max_min_alpha() {
    let records = vec![
        make_record("1", "tracked", Some(20.0), None),
        make_record("2", "tracked", Some(-10.0), None),
        make_record("3", "tracked", Some(5.0), None),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.max_alpha, Some(20.0));
    assert_eq!(summary.min_alpha, Some(-10.0));
}

#[test]
fn summarize_tracking_all_positive() {
    let records = vec![
        make_record("1", "tracked", Some(5.0), None),
        make_record("2", "tracked", Some(10.0), None),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.hit_count, 2);
    assert_eq!(summary.hit_rate, Some(1.0));
}

#[test]
fn summarize_tracking_all_negative() {
    let records = vec![
        make_record("1", "tracked", Some(-5.0), None),
        make_record("2", "tracked", Some(-10.0), None),
    ];
    let summary = summarize_tracking(&records);
    assert_eq!(summary.hit_count, 0);
    assert_eq!(summary.hit_rate, Some(0.0));
}

#[test]
fn alpha_tracking_summary_default() {
    let summary = AlphaTrackingSummary::default();
    assert_eq!(summary.total_picks, 0);
    assert!(summary.records.is_empty());
}
