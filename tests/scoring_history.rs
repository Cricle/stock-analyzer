use sa::scoring::history::{PriceSnapshot, StoredRecommendation, compute_performance_report};

#[test]
fn test_empty_report() {
    let report = compute_performance_report(&[], &[], "7d");
    assert_eq!(report.total_recommendations, 0);
    assert_eq!(report.accuracy_rate, 0.0);
}

#[test]
fn test_performance_report() {
    let recs = vec![
        StoredRecommendation {
            id: "rec-1".into(),
            symbol: "AAPL".into(),
            market: "美股".into(),
            score_total: 80,
            score_technical: 85,
            score_fundamental: 75,
            score_sentiment: 70,
            score_llm: 85,
            reasons: serde_json::json!({}),
            price_at_recommend: Some(150.0),
            recommended_at: "2026-01-01T00:00:00Z".into(),
        },
        StoredRecommendation {
            id: "rec-2".into(),
            symbol: "TSLA".into(),
            market: "美股".into(),
            score_total: 40,
            score_technical: 35,
            score_fundamental: 45,
            score_sentiment: 40,
            score_llm: 40,
            reasons: serde_json::json!({}),
            price_at_recommend: Some(200.0),
            recommended_at: "2026-01-01T00:00:00Z".into(),
        },
    ];
    let snaps = vec![
        PriceSnapshot {
            id: "snap-1".into(),
            recommendation_id: "rec-1".into(),
            days_after: 7,
            price: 160.0,
            return_pct: 6.67,
            max_drawdown: -2.0,
            recorded_at: "2026-01-08T00:00:00Z".into(),
        },
        PriceSnapshot {
            id: "snap-2".into(),
            recommendation_id: "rec-2".into(),
            days_after: 7,
            price: 190.0,
            return_pct: -5.0,
            max_drawdown: -8.0,
            recorded_at: "2026-01-08T00:00:00Z".into(),
        },
    ];
    let report = compute_performance_report(&recs, &snaps, "7d");
    assert_eq!(report.total_recommendations, 2);
    assert_eq!(report.accuracy_rate, 0.5);
    assert!((report.avg_return - 0.835).abs() < 0.1);
}
