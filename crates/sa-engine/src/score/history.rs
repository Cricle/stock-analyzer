use serde::{Deserialize, Serialize};

/// Stored recommendation for performance tracking.
///
/// TODO: Previously from sa_storage::pg::scoring. Needs to be reconciled with
/// the actual storage layer when it's implemented.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredRecommendation {
    pub id: String,
    pub symbol: String,
    pub market: String,
    pub score_total: i32,
    pub score_technical: i32,
    pub score_fundamental: i32,
    pub score_sentiment: i32,
    pub score_llm: i32,
    pub reasons: serde_json::Value,
    pub price_at_recommend: Option<f64>,
    pub recommended_at: String,
}

/// Price snapshot for performance tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub id: String,
    pub recommendation_id: String,
    pub days_after: i32,
    pub price: f64,
    pub return_pct: f64,
    pub max_drawdown: f64,
    pub recorded_at: String,
}

/// Performance report for a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub period: String,
    pub total_recommendations: u32,
    pub accuracy_rate: f64,
    pub avg_return: f64,
    pub max_drawdown: f64,
    /// (score_bucket, avg_return) pairs.
    pub score_vs_return: Vec<(u8, f64)>,
}

/// Compute performance report from stored snapshots.
pub fn compute_performance_report(
    recommendations: &[StoredRecommendation],
    snapshots: &[PriceSnapshot],
    period: &str,
) -> PerformanceReport {
    if snapshots.is_empty() {
        return PerformanceReport {
            period: period.to_string(),
            total_recommendations: 0,
            accuracy_rate: 0.0,
            avg_return: 0.0,
            max_drawdown: 0.0,
            score_vs_return: Vec::new(),
        };
    }

    let total = snapshots.len() as f64;
    let winners = snapshots.iter().filter(|s| s.return_pct > 0.0).count() as f64;
    let accuracy_rate = winners / total;
    let avg_return = snapshots.iter().map(|s| s.return_pct).sum::<f64>() / total;
    let max_drawdown = snapshots.iter().map(|s| s.max_drawdown).fold(0.0f64, f64::min);

    // Score vs return buckets (20-point buckets)
    let mut bucket_sums: std::collections::HashMap<u8, (f64, u32)> = std::collections::HashMap::new();
    for snap in snapshots {
        if let Some(rec) = recommendations.iter().find(|r| r.id == snap.recommendation_id) {
            let bucket = (rec.score_total as u8 / 20) * 20;
            let entry = bucket_sums.entry(bucket).or_insert((0.0, 0));
            entry.0 += snap.return_pct;
            entry.1 += 1;
        }
    }
    let mut score_vs_return: Vec<(u8, f64)> = bucket_sums
        .into_iter()
        .map(|(bucket, (sum, count))| (bucket, sum / count as f64))
        .collect();
    score_vs_return.sort_by_key(|(b, _)| *b);

    PerformanceReport {
        period: period.to_string(),
        total_recommendations: snapshots.len() as u32,
        accuracy_rate,
        avg_return,
        max_drawdown,
        score_vs_return,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                id: "rec-1".into(), symbol: "AAPL".into(), market: "美股".into(),
                score_total: 80, score_technical: 85, score_fundamental: 75,
                score_sentiment: 70, score_llm: 85,
                reasons: serde_json::json!({}), price_at_recommend: Some(150.0),
                recommended_at: "2026-01-01T00:00:00Z".into(),
            },
            StoredRecommendation {
                id: "rec-2".into(), symbol: "TSLA".into(), market: "美股".into(),
                score_total: 40, score_technical: 35, score_fundamental: 45,
                score_sentiment: 40, score_llm: 40,
                reasons: serde_json::json!({}), price_at_recommend: Some(200.0),
                recommended_at: "2026-01-01T00:00:00Z".into(),
            },
        ];
        let snaps = vec![
            PriceSnapshot {
                id: "snap-1".into(), recommendation_id: "rec-1".into(),
                days_after: 7, price: 160.0, return_pct: 6.67, max_drawdown: -2.0,
                recorded_at: "2026-01-08T00:00:00Z".into(),
            },
            PriceSnapshot {
                id: "snap-2".into(), recommendation_id: "rec-2".into(),
                days_after: 7, price: 190.0, return_pct: -5.0, max_drawdown: -8.0,
                recorded_at: "2026-01-08T00:00:00Z".into(),
            },
        ];
        let report = compute_performance_report(&recs, &snaps, "7d");
        assert_eq!(report.total_recommendations, 2);
        assert_eq!(report.accuracy_rate, 0.5);
        assert!((report.avg_return - 0.835).abs() < 0.1);
    }
}
