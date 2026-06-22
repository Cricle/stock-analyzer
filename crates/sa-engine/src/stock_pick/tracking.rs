//! Stock pick alpha return tracking.
//!
//! Tracks actual price performance of stock picks over time.
//! Supports both automatic (scheduled) and manual return entry.

#![allow(dead_code)]

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

/// Request to manually record alpha return.
#[derive(Clone, Debug, Deserialize)]
pub struct ManualAlphaRequest {
    pub symbol: String,
    pub alpha_return: f64,
    pub note: Option<String>,
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

/// Configuration for auto-tracking.
pub struct TrackingConfig {
    pub track_days: i64,
}

impl TrackingConfig {
    pub fn from_env() -> Self {
        let track_days = std::env::var("STOCK_PICK_ALPHA_TRACK_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        Self { track_days }
    }
}

/// Refresh alpha returns for all pending tracking records.
/// Called by the scheduler or manually via API.

/// Update alpha_return in Qdrant stock_pick_history points.
async fn update_qdrant_alpha_return(
    symbol: &str,
    market: &str,
    alpha_return: f64,
) -> anyhow::Result<()> {
    let qdrant_url = std::env::var("QDRANT_URL")
        .or_else(|_| std::env::var("RAG_QDRANT_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
    let collection = std::env::var("STOCK_PICK_QDRANT_COLLECTION")
        .unwrap_or_else(|_| "tradingagents_stock_pick".to_string());

    let client = crate::shared::shared_http_client();
    // Update all stock_pick_history points for this symbol
    let response = client
        .post(format!(
            "{}/collections/{}/points/payload",
            qdrant_url.trim().trim_end_matches('/'),
            collection
        ))
        .json(&serde_json::json!({
            "points": [],  // We'll use filter-based update
            "filter": {
                "must": [
                    {"key": "entry_kind", "match": {"value": "stock_pick_history"}},
                    {"key": "symbol", "match": {"value": symbol}},
                    {"key": "market", "match": {"value": market}}
                ]
            },
            "payload": {
                "alpha_return": alpha_return
            }
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("qdrant update failed: {body}");
    }

    Ok(())
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

/// Entry for creating alpha tracking records.
#[derive(Clone, Debug)]
pub struct AlphaTrackingEntry {
    pub symbol: String,
    pub market: String,
    pub entry_price: f64,
    pub entry_date: String,
}

/// Helper to update Qdrant alpha_return for a specific symbol in a run.
/// Queries Qdrant to find the matching stock_pick_history point and updates its alpha_return.
pub async fn update_qdrant_alpha_return_for_run(
    run_id: &str,
    symbol: &str,
    alpha_return: f64,
) -> anyhow::Result<()> {
    let qdrant_url = std::env::var("QDRANT_URL")
        .or_else(|_| std::env::var("RAG_QDRANT_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
    let collection = std::env::var("STOCK_PICK_QDRANT_COLLECTION")
        .unwrap_or_else(|_| "tradingagents_stock_pick".to_string());

    let client = crate::shared::shared_http_client();
    let response = client
        .post(format!(
            "{}/collections/{}/points/payload",
            qdrant_url.trim().trim_end_matches('/'),
            collection
        ))
        .json(&serde_json::json!({
            "points": [],
            "filter": {
                "must": [
                    {"key": "entry_kind", "match": {"value": "stock_pick_history"}},
                    {"key": "symbol", "match": {"value": symbol}},
                    {"key": "run_id", "match": {"value": run_id}}
                ]
            },
            "payload": {
                "alpha_return": alpha_return
            }
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("qdrant update alpha_return failed: {body}");
    }

    Ok(())
}
