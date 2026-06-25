use serde::{Deserialize, Serialize};

/// Stored recommendation for performance tracking.
///
/// Implement `AnalysisStore` to persist these records.
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
    let max_drawdown = snapshots
        .iter()
        .map(|s| s.max_drawdown)
        .fold(0.0f64, f64::min);

    // Score vs return buckets (20-point buckets)
    let mut bucket_sums: std::collections::HashMap<u8, (f64, u32)> =
        std::collections::HashMap::new();
    for snap in snapshots {
        if let Some(rec) = recommendations
            .iter()
            .find(|r| r.id == snap.recommendation_id)
        {
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
