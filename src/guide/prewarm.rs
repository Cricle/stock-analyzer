//! Pre-warming system for daily guidance reports.
//!
//! TODO: This module previously published pre-warm tasks to NATS.
//! With the trait-based architecture, this needs to be refactored to use
//! an event bus trait or similar mechanism.
//!
//! For now, this module provides the data structures for prewarm tasks
//! but the actual publishing is deferred to the caller.

/// Pre-warm task data for a specific market.
#[derive(Clone, Debug)]
pub struct PrewarmTask {
    pub market: String,
    pub date: String,
    pub tickers: Vec<String>,
}

/// Generate pre-warm tasks for all markets.
pub fn generate_prewarm_tasks() -> Vec<PrewarmTask> {
    let date = chrono::Utc::now().date_naive().to_string();

    vec![
        PrewarmTask {
            market: "a_share".to_string(),
            date: date.clone(),
            tickers: vec!["600519", "000858", "601318", "600036", "000001"]
                .into_iter()
                .map(String::from)
                .collect(),
        },
        PrewarmTask {
            market: "hong_kong".to_string(),
            date: date.clone(),
            tickers: vec!["00700", "09988", "00005", "01299", "02318"]
                .into_iter()
                .map(String::from)
                .collect(),
        },
        PrewarmTask {
            market: "us_equity".to_string(),
            date,
            tickers: vec!["AAPL", "TSLA", "NVDA", "MSFT", "GOOGL"]
                .into_iter()
                .map(String::from)
                .collect(),
        },
    ]
}
