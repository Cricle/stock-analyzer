//! Stock pick enrichment and price/name lookups.

use rust_decimal::prelude::ToPrimitive;

use super::*;

impl DailyGuidanceGenerator {
    /// Enrich stock guidances with live price data and company names.
    /// Uses batch fetch to minimize Redis roundtrips.
    pub(super) async fn enrich_stock_guidances(&self, stock_guidances: &mut [StockGuidance]) {
        if stock_guidances.is_empty() {
            return;
        }
        let symbols: Vec<&str> = stock_guidances.iter().map(|g| g.symbol.as_str()).collect();
        let names_missing: Vec<bool> = stock_guidances.iter().map(|g| g.stock_name.is_empty()).collect();

        // Batch fetch quotes
        let quotes = self.market_data.fetch_quotes_batch(&symbols).await;

        // Only fetch fundamentals for symbols missing names
        let fund_symbols: Vec<&str> = symbols
            .iter()
            .zip(names_missing.iter())
            .filter(|(_, missing)| **missing)
            .map(|(&sym, _)| sym)
            .collect();
        let fundamentals = self.market_data.fetch_fundamentals_batch(&fund_symbols).await;
        let fund_map: std::collections::HashMap<&str, &sa_data::FundamentalsSnapshot> =
            fundamentals.iter().filter_map(|(s, f)| f.as_ref().map(|f| (s.as_str(), f))).collect();

        for (guidance, (_, quote_opt)) in stock_guidances.iter_mut().zip(quotes.iter()) {
            if let Some(quote) = quote_opt {
                guidance.current_price = Some(quote.close.to_f64().unwrap_or_default());
                let open = quote.open.to_f64().unwrap_or_default();
                if open > 0.0 {
                    let close = quote.close.to_f64().unwrap_or_default();
                    guidance.price_change_pct = Some(((close - open) / open) * 100.0);
                }
            }
            if guidance.stock_name.is_empty()
                && let Some(fund) = fund_map.get(guidance.symbol.as_str())
                && !fund.company_name.is_empty()
            {
                guidance.stock_name = fund.company_name.clone();
            }
        }
    }

    /// Fetch recent stock pick summary from Qdrant for inclusion in the report.
    pub(super) async fn fetch_recent_stock_picks(
        &self,
        market: &GuidanceMarket,
    ) -> Option<RecentStockPickSummary> {
        let summary = self
            .store
            .get_latest_stock_pick_summary(market.as_str())
            .await
            .ok()??;

        let picks = summary
            .get("picks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .take(3)
                    .filter_map(|pick| {
                        Some(StockPickGuidanceEntry {
                            symbol: pick.get("symbol")?.as_str()?.to_string(),
                            name: pick.get("name")?.as_str().unwrap_or("").to_string(),
                            score: pick.get("score")?.as_f64().unwrap_or(0.0),
                            confidence: pick.get("confidence")?.as_f64().unwrap_or(0.0),
                            thesis: pick.get("thesis")?.as_str().unwrap_or("").to_string(),
                            current_price: pick.get("price")?.as_f64(),
                            alpha_return: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let average_score = if !picks.is_empty() {
            picks.iter().map(|p| p.score).sum::<f64>() / picks.len() as f64
        } else {
            0.0
        };

        Some(RecentStockPickSummary {
            run_id: summary
                .get("run_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            analysis_date: summary
                .get("analysis_date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            market: market.as_str().to_string(),
            strategy: summary
                .get("strategy")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            picks,
            average_score,
            average_alpha: None,
        })
    }
}
