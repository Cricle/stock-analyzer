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

#[cfg(test)]
mod prewarm_tests {
    use super::*;

    #[test]
    fn generate_prewarm_tasks_returns_three_markets() {
        let tasks = generate_prewarm_tasks();
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn generate_prewarm_tasks_markets() {
        let tasks = generate_prewarm_tasks();
        let markets: Vec<&str> = tasks.iter().map(|t| t.market.as_str()).collect();
        assert!(markets.contains(&"a_share"));
        assert!(markets.contains(&"hong_kong"));
        assert!(markets.contains(&"us_equity"));
    }

    #[test]
    fn generate_prewarm_tasks_ticker_counts() {
        let tasks = generate_prewarm_tasks();
        for task in &tasks {
            assert_eq!(
                task.tickers.len(),
                5,
                "{} should have 5 tickers",
                task.market
            );
        }
    }

    #[test]
    fn generate_prewarm_tasks_date_is_today() {
        let tasks = generate_prewarm_tasks();
        let today = chrono::Utc::now().date_naive().to_string();
        for task in &tasks {
            assert_eq!(task.date, today);
        }
    }

    #[test]
    fn generate_prewarm_tasks_a_share_tickers() {
        let tasks = generate_prewarm_tasks();
        let a_share = tasks.iter().find(|t| t.market == "a_share").unwrap();
        assert!(a_share.tickers.contains(&"600519".to_string()));
        assert!(a_share.tickers.contains(&"000001".to_string()));
    }

    #[test]
    fn generate_prewarm_tasks_us_tickers() {
        let tasks = generate_prewarm_tasks();
        let us = tasks.iter().find(|t| t.market == "us_equity").unwrap();
        assert!(us.tickers.contains(&"AAPL".to_string()));
        assert!(us.tickers.contains(&"NVDA".to_string()));
    }
}
