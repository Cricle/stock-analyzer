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

/// Fetch trending tickers from market data and generate prewarm tasks.
pub async fn generate_prewarm_tasks_from_market(
    market_data: &crate::data::MarketDataClient,
    tickers_per_market: usize,
) -> anyhow::Result<Vec<PrewarmTask>> {
    let limit = tickers_per_market.clamp(3, 10);

    let a_share_tickers = fetch_trending_a_share(market_data, limit).await;
    let hk_tickers = fetch_trending_by_market(market_data, "港股", limit).await;
    let us_tickers = fetch_trending_by_market(market_data, "美股", limit).await;

    Ok(generate_prewarm_tasks(&[
        ("a_share", a_share_tickers),
        ("hong_kong", hk_tickers),
        ("us_equity", us_tickers),
    ]))
}

async fn fetch_trending_a_share(
    market_data: &crate::data::MarketDataClient,
    limit: usize,
) -> Vec<String> {
    // Try sector-based trending stocks
    let sectors = market_data
        .fetch_a_share_sector_rankings("industry", 3)
        .await
        .unwrap_or_default();

    let mut tickers = Vec::new();
    for sector in sectors.iter().take(2) {
        if let Ok(constituents) = market_data
            .fetch_a_share_sector_constituents(&sector.sector_code, limit)
            .await
        {
            for c in constituents.iter().take(limit / 2) {
                if tickers.len() < limit {
                    tickers.push(c.symbol.clone());
                }
            }
        }
    }

    // Fallback to search if sector fetch fails
    if tickers.is_empty() {
        if let Ok(items) = market_data.search_stocks("industry", Some("A-share"), limit).await {
            tickers.extend(items.into_iter().map(|i| i.symbol));
        }
    }

    tickers
}

async fn fetch_trending_by_market(
    market_data: &crate::data::MarketDataClient,
    market: &str,
    limit: usize,
) -> Vec<String> {
    let query = match market {
        "港股" => "blue chip",
        _ => "technology",
    };
    market_data
        .search_stocks(query, Some(market), limit)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|i| i.symbol)
        .collect()
}
