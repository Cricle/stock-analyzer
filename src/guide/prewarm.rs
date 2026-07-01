//! Pre-warming system for daily guidance reports.

/// Pre-warm task data for a specific market.
#[derive(Clone, Debug)]
pub struct PrewarmTask {
    pub market: String,
    pub date: String,
    pub tickers: Vec<String>,
}

/// Generate pre-warm tasks for specified markets and tickers.
///
/// If `market_tickers` is empty, returns empty tasks.
pub fn generate_prewarm_tasks(market_tickers: &[(&str, Vec<String>)]) -> Vec<PrewarmTask> {
    let date = chrono::Utc::now().date_naive().to_string();

    market_tickers
        .iter()
        .filter(|(_, tickers)| !tickers.is_empty())
        .map(|(market, tickers)| PrewarmTask {
            market: market.to_string(),
            date: date.clone(),
            tickers: tickers.clone(),
        })
        .collect()
}
