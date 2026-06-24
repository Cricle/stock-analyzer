impl TradingMemoryLog {
    pub async fn batch_update_with_outcomes_async(
        &self,
        updates: &[MemoryOutcomeUpdate],
    ) -> anyhow::Result<()> {
        self.batch_update_with_outcomes(updates).await?;
        if updates.is_empty() {
            return Ok(());
        }
        let mut pending_ids = updates
            .iter()
            .map(|item| Self::entry_id(&item.ticker, &item.trade_date))
            .collect::<HashSet<_>>();
        for entry in self.load_entries().await?.into_iter() {
            if pending_ids.remove(&Self::entry_id(&entry.ticker, &entry.trade_date)) {
                self.qdrant_upsert_entry(&entry).await?;
            }
            if pending_ids.is_empty() {
                break;
            }
        }
        Ok(())
    }

    /// Search for successful trading patterns across tickers for lesson propagation.
    /// Returns entries with positive alpha that share similar setup tags.
    pub async fn cross_ticker_lessons(
        &self,
        market: &str,
        setup_tags: &[String],
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let Some(backend) = &self.vector_store else {
            return Ok(Vec::new());
        };
        if !self.embedding.retrieval_enabled {
            return Ok(Vec::new());
        }
        let query_text = format!(
            "market {} setup {} successful positive alpha return lesson",
            market,
            setup_tags.join(" ")
        );
        let vector = self.embed_text(&query_text);
        let mut entries = self
            .vector_search_filtered(
                backend.as_ref(),
                &vector,
                limit * 3,
                None,
                Some(market),
                None,
            )
            .await?;
        // Filter to only entries with positive alpha return
        entries.retain(|e| e.alpha_return.unwrap_or(0.0) > 0.0 && !e.pending);
        // Sort by alpha return descending
        entries.sort_by(|a, b| {
            b.alpha_return
                .unwrap_or(0.0)
                .partial_cmp(&a.alpha_return.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries.truncate(limit);
        Ok(entries)
    }

    pub async fn evaluation_summary(&self) -> anyhow::Result<serde_json::Value> {
        let entries = self.load_entries().await?;
        let resolved = entries
            .into_iter()
            .filter(|entry| {
                !entry.pending && entry.raw_return.is_some() && entry.alpha_return.is_some()
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "sample_count": resolved.len(),
            "overall": summarize_entries(&resolved),
            "calibration_profile": suggested_calibration_profile(&resolved),
            "by_rating": group_summary(&resolved, |entry| entry.rating.clone()),
            "by_action": group_summary(&resolved, |entry| {
                if entry.action.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    entry.action.clone()
                }
            }),
            "by_market": group_summary(&resolved, |entry| {
                if entry.market.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    entry.market.clone()
                }
            }),
            "by_confidence_band": group_summary(&resolved, |entry| {
                bucket_score(entry.confidence_score, &[40, 55, 70, 85])
            }),
            "by_direction_band": group_summary(&resolved, |entry| {
                bucket_signed_score(entry.direction_score, &[-60, -20, 20, 60])
            }),
            "by_action_band": group_summary(&resolved, |entry| {
                bucket_score(entry.action_score, &[40, 55, 70, 85])
            }),
        }))
    }

    pub(super) fn entry_id(ticker: &str, trade_date: &str) -> String {
        format!("{}:{}", ticker.trim().to_uppercase(), trade_date.trim())
    }

    pub(super) fn research_entry_id(ticker: &str, trade_date: &str) -> String {
        format!(
            "research:{}:{}",
            ticker.trim().to_uppercase(),
            trade_date.trim()
        )
    }

    pub(super) fn entry_text(entry: &MemoryEntry) -> String {
        [
            format!("ticker {}", entry.ticker),
            format!("date {}", entry.trade_date),
            format!("market {}", entry.market),
            format!("rating {}", entry.rating),
            format!("action {}", entry.action),
            entry.final_trade_decision.clone(),
            entry.reflection.clone().unwrap_or_default(),
        ]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
    }

    pub(super) fn query_text(ticker: &str) -> String {
        format!(
            "stock {} decision execution risk reflection lesson analysis",
            ticker.trim().to_uppercase()
        )
    }

    pub(super) fn research_query_text(query: &MemoryQuery) -> String {
        let mut text = format!(
            "stock {} market {} research setup trigger risk execution checklist historical lesson",
            query.ticker.trim().to_uppercase(),
            query.market.trim()
        );
        if !query.setup_tags.is_empty() {
            text.push_str(" tags ");
            text.push_str(&query.setup_tags.join(" "));
        }
        text
    }

    pub(super) fn dedupe_entries(
        &self,
        entries: Vec<MemoryEntry>,
        limit: usize,
    ) -> Vec<MemoryEntry> {
        let mut seen = HashSet::new();
        let mut output = Vec::new();
        for entry in entries {
            let key = Self::entry_id(&entry.ticker, &entry.trade_date);
            if seen.insert(key) {
                output.push(entry);
            }
            if output.len() >= limit {
                break;
            }
        }
        output
    }

    pub(super) fn rerank_entries(
        &self,
        mut entries: Vec<MemoryEntry>,
        query: &MemoryQuery,
        same_ticker: bool,
    ) -> Vec<MemoryEntry> {
        entries.sort_by(|left, right| {
            let left_score = Self::entry_rank_score(left, query, same_ticker);
            let right_score = Self::entry_rank_score(right, query, same_ticker);
            right_score
                .partial_cmp(&left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.trade_date.cmp(&left.trade_date))
        });
        entries
    }

    fn entry_rank_score(entry: &MemoryEntry, query: &MemoryQuery, same_ticker: bool) -> f64 {
        let same_market = !query.market.trim().is_empty()
            && entry
                .market
                .trim()
                .eq_ignore_ascii_case(query.market.trim());
        let resolved = !entry.pending && entry.raw_return.is_some() && entry.alpha_return.is_some();
        let alpha = entry.alpha_return.unwrap_or_default();
        let raw = entry.raw_return.unwrap_or_default();
        let boundary = entry.execution_boundary_complete.unwrap_or(false);
        let setup_overlap = Self::setup_overlap_count(entry, query) as f64;

        let mut score = 0.0;
        if same_ticker {
            score += 8.0;
        }
        if same_market {
            score += 5.0;
        }
        score += setup_overlap * 3.0;
        if resolved {
            score += 6.0;
        }
        if boundary {
            score += 2.5;
        }
        score += alpha * 20.0;
        score += raw * 10.0;
        score += (entry.confidence_score.unwrap_or_default() as f64) / 100.0;
        score += (entry.action_score.unwrap_or_default() as f64) / 200.0;
        score
    }
}
