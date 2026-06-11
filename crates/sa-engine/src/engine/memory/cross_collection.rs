use serde::{Deserialize, Serialize};

use super::TradingMemoryLog;
use crate::engine::guidance::GuidanceStore;
use crate::engine::stock_pick::StockPickHistoryStore;

#[derive(Clone)]
pub struct CrossCollectionSearcher {
    pub memory: TradingMemoryLog,
    pub guidance: GuidanceStore,
    pub(crate) stock_pick: StockPickHistoryStore,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossCollectionResult {
    pub source: String,
    pub entry_kind: String,
    pub ticker: String,
    pub date: String,
    pub summary: String,
    pub score: f64,
    pub payload: serde_json::Value,
}

impl CrossCollectionSearcher {
    pub fn new(memory: TradingMemoryLog) -> Self {
        Self {
            memory,
            guidance: GuidanceStore::from_env(),
            stock_pick: StockPickHistoryStore::from_env().unwrap_or_else(|_| {
                StockPickHistoryStore::from_env().expect("stock pick store init")
            }),
        }
    }

    pub async fn search(
        &self,
        query: &str,
        ticker: Option<&str>,
        market: Option<&str>,
        limit_per_source: usize,
    ) -> Vec<CrossCollectionResult> {
        let embedding = self.memory.embed_text(query);
        if embedding.is_empty() {
            return Vec::new();
        }

        let (memory_results, guidance_results, stock_pick_results) = tokio::join!(
            self.search_memory(&embedding, ticker, market, limit_per_source),
            self.search_guidance(&embedding, market, limit_per_source),
            self.search_stock_pick(ticker, market, limit_per_source),
        );

        let mut all_results = Vec::new();
        all_results.extend(memory_results);
        all_results.extend(guidance_results);
        all_results.extend(stock_pick_results);

        // Deduplicate by ticker+date
        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        all_results
            .into_iter()
            .filter(|r| {
                let key = format!("{}:{}", r.ticker.to_lowercase(), r.date);
                seen.insert(key)
            })
            .collect()
    }

    async fn search_memory(
        &self,
        embedding: &[f32],
        ticker: Option<&str>,
        market: Option<&str>,
        limit: usize,
    ) -> Vec<CrossCollectionResult> {
        let Some(backend) = &self.memory.vector_store else {
            return Vec::new();
        };
        let entries = match self
            .memory
            .vector_search_filtered(backend.as_ref(), embedding, limit, ticker, market, None)
            .await
        {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(error = %e, "cross-collection memory search failed");
                return Vec::new();
            }
        };
        entries
            .into_iter()
            .map(|entry| CrossCollectionResult {
                source: "memory".to_string(),
                entry_kind: "decision".to_string(),
                ticker: entry.ticker.clone(),
                date: entry.trade_date.clone(),
                summary: entry.summary.clone(),
                score: 0.0,
                payload: serde_json::to_value(&entry).unwrap_or_default(),
            })
            .collect()
    }

    async fn search_guidance(
        &self,
        embedding: &[f32],
        market: Option<&str>,
        limit: usize,
    ) -> Vec<CrossCollectionResult> {
        let summaries = match self
            .guidance
            .search_daily_summaries(embedding, market, limit)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!(error = %e, "cross-collection guidance search failed");
                return Vec::new();
            }
        };
        summaries
            .into_iter()
            .filter_map(|point| {
                let payload = point.get("payload")?.clone();
                let date = payload.get("date")?.as_str()?.to_string();
                let market_str = payload.get("market").and_then(|v| v.as_str()).unwrap_or("");
                let entry_kind = payload
                    .get("entry_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("daily_guidance");
                let score = point.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let summary = format!(
                    "market={} sentiment={}",
                    market_str,
                    payload
                        .get("sentiment_label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                );
                Some(CrossCollectionResult {
                    source: "guidance".to_string(),
                    entry_kind: entry_kind.to_string(),
                    ticker: String::new(),
                    date,
                    summary,
                    score,
                    payload,
                })
            })
            .collect()
    }

    async fn search_stock_pick(
        &self,
        ticker: Option<&str>,
        market: Option<&str>,
        _limit: usize,
    ) -> Vec<CrossCollectionResult> {
        let symbol = ticker.unwrap_or_default();
        let market_str = market.unwrap_or("us_equity");
        if symbol.is_empty() {
            return Vec::new();
        }
        let history = match self.stock_pick.read_history(symbol, market_str, "", None).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(error = %e, "cross-collection stock pick search failed");
                return Vec::new();
            }
        };
        history
            .top_matches
            .into_iter()
            .enumerate()
            .map(|(i, sym)| CrossCollectionResult {
                source: "stock_pick".to_string(),
                entry_kind: "stock_pick_history".to_string(),
                ticker: sym,
                date: String::new(),
                summary: format!(
                    "avg_score={:?} hit_rate={:?}",
                    history.average_score, history.hit_rate
                ),
                score: 1.0 - (i as f64 * 0.1),
                payload: serde_json::json!({
                    "sample_count": history.sample_count,
                    "average_score": history.average_score,
                    "hit_rate": history.hit_rate,
                    "average_alpha_return": history.average_alpha_return,
                }),
            })
            .collect()
    }
}
