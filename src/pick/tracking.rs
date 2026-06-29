//! Stock pick alpha return tracking.
//!
//! Tracks actual price performance of stock picks over time.

use serde::{Deserialize, Serialize};

/// Alpha tracking record for a single stock pick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlphaTrackingRecord {
    pub id: String,
    pub run_id: String,
    pub symbol: String,
    pub market: String,
    pub entry_price: f64,
    pub entry_date: String,
    pub exit_price: Option<f64>,
    pub exit_date: Option<String>,
    pub alpha_return: Option<f64>,
    pub benchmark_return: Option<f64>,
    pub tracking_status: String,
    pub manual_return: Option<f64>,
    pub manual_note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Summary of alpha tracking for a stock pick run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlphaTrackingSummary {
    pub total_picks: usize,
    pub tracked_count: usize,
    pub pending_count: usize,
    pub manual_count: usize,
    pub hit_count: usize,
    pub hit_rate: Option<f64>,
    pub average_alpha: Option<f64>,
    pub max_alpha: Option<f64>,
    pub min_alpha: Option<f64>,
    pub records: Vec<AlphaTrackingRecord>,
}

/// Summarize alpha tracking for a given run.
pub fn summarize_tracking(records: &[AlphaTrackingRecord]) -> AlphaTrackingSummary {
    let total_picks = records.len();
    let tracked_count = records
        .iter()
        .filter(|r| r.tracking_status == "tracked")
        .count();
    let pending_count = records
        .iter()
        .filter(|r| r.tracking_status == "pending")
        .count();
    let manual_count = records
        .iter()
        .filter(|r| r.tracking_status == "manual")
        .count();

    let alpha_values: Vec<f64> = records
        .iter()
        .filter_map(|r| {
            if r.tracking_status == "manual" {
                r.manual_return
            } else {
                r.alpha_return
            }
        })
        .collect();

    let hit_count = alpha_values.iter().filter(|a| **a > 0.0).count();
    let hit_rate = if !alpha_values.is_empty() {
        Some(hit_count as f64 / alpha_values.len() as f64)
    } else {
        None
    };
    let average_alpha = if !alpha_values.is_empty() {
        Some(alpha_values.iter().sum::<f64>() / alpha_values.len() as f64)
    } else {
        None
    };
    let max_alpha = alpha_values.iter().cloned().reduce(f64::max);
    let min_alpha = alpha_values.iter().cloned().reduce(f64::min);

    AlphaTrackingSummary {
        total_picks,
        tracked_count,
        pending_count,
        manual_count,
        hit_count,
        hit_rate,
        average_alpha,
        max_alpha,
        min_alpha,
        records: records.to_vec(),
    }
}
