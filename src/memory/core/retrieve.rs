use anyhow::Context;
use serde_json::json;

use super::super::stats::SetupMatchStats;
use super::super::{ENTRY_SEPARATOR, MemoryContextBundle, MemoryEntry, MemoryQuery};

impl super::super::TradingMemoryLog {
    pub async fn load_entries(&self) -> anyhow::Result<Vec<MemoryEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let text = tokio::fs::read_to_string(&self.log_path)
            .await
            .with_context(|| format!("failed to read {}", self.log_path.display()))?;
        Ok(text
            .split(ENTRY_SEPARATOR)
            .filter_map(Self::parse_entry)
            .collect())
    }

    pub async fn past_context_bundle_async(
        &self,
        ticker: &str,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<MemoryContextBundle> {
        self.past_context_bundle_async_with_query(
            &MemoryQuery {
                ticker: ticker.to_string(),
                market: String::new(),
                setup_tags: Vec::new(),
                user_id: String::new(),
            },
            same_limit,
            cross_limit,
        )
        .await
    }

    pub async fn past_context_bundle_async_with_query(
        &self,
        query: &MemoryQuery,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<MemoryContextBundle> {
        let same_limit = self.rag.same_ticker_top_k.max(same_limit);
        let cross_limit = self.rag.cross_ticker_top_k.max(cross_limit);
        if let Some(context) = self
            .vector_past_context_bundle(query, same_limit, cross_limit)
            .await?
            .filter(|value| !value.context_text.trim().is_empty())
        {
            return Ok(context);
        }
        self.local_past_context_bundle(query, same_limit, cross_limit)
            .await
    }

    pub async fn past_context(
        &self,
        ticker: &str,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<String> {
        let entries = self
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| !entry.pending)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Ok(String::new());
        }

        let mut same = Vec::new();
        let mut cross = Vec::new();
        for entry in entries.into_iter().rev() {
            if same.len() >= same_limit && cross.len() >= cross_limit {
                break;
            }
            if entry.ticker.eq_ignore_ascii_case(ticker) && same.len() < same_limit {
                same.push(entry);
            } else if !entry.ticker.eq_ignore_ascii_case(ticker) && cross.len() < cross_limit {
                cross.push(entry);
            }
        }

        let mut parts = Vec::new();
        if !same.is_empty() {
            parts.push(format!("Past analyses of {ticker} (most recent first):"));
            parts.extend(same.iter().map(Self::format_full_entry));
        }
        if !cross.is_empty() {
            parts.push("Recent cross-ticker lessons:".to_string());
            parts.extend(cross.iter().map(Self::format_reflection_only));
        }
        Ok(parts.join("\n\n"))
    }

    async fn local_past_context_bundle(
        &self,
        query: &MemoryQuery,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<MemoryContextBundle> {
        let entries = self
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| !entry.pending)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Ok(MemoryContextBundle::default());
        }

        let same_entries = entries
            .iter()
            .filter(|entry| entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .cloned()
            .collect::<Vec<_>>();
        let cross_entries = entries
            .iter()
            .filter(|entry| !entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .cloned()
            .collect::<Vec<_>>();
        let same_entries =
            self.dedupe_entries(self.rerank_entries(same_entries, query, true), same_limit);
        let cross_entries = self.dedupe_entries(
            self.rerank_entries(cross_entries, query, false),
            cross_limit,
        );

        let parts = crate::memory::format_memory_parts(
            &same_entries,
            &cross_entries,
            Self::format_full_entry,
            Self::format_reflection_only,
        );

        let (same_ticker_highlights, cross_ticker_highlights) = crate::memory::build_highlights(
            &same_entries,
            &cross_entries,
            Self::highlight_from_entry,
        );

        Ok(MemoryContextBundle {
            context_text: Self::compact_context_text(&parts.join("\n\n"), 3200),
            source: "local".to_string(),
            retrieval_mode: "local".to_string(),
            embedding_provider: if self.rag.enabled {
                self.embedding.provider.clone()
            } else {
                "disabled".to_string()
            },
            embedding_failure_reason: self.embedding.failure_reason.clone(),
            same_ticker_count: same_entries.len(),
            cross_ticker_count: cross_entries.len(),
            vector_hit_count: 0,
            effective_top_k: same_limit + cross_limit,
            same_ticker_highlights,
            cross_ticker_highlights,
        })
    }

    pub(crate) async fn setup_match_stats(
        &self,
        query: &MemoryQuery,
    ) -> anyhow::Result<SetupMatchStats> {
        let effective_query_tags = Self::effective_setup_tags(&query.setup_tags);
        if effective_query_tags.is_empty() {
            return Ok(SetupMatchStats::default());
        }
        let query = MemoryQuery {
            ticker: query.ticker.clone(),
            market: query.market.clone(),
            setup_tags: effective_query_tags,
            user_id: query.user_id.clone(),
        };
        let matched_entries = self
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| Self::setup_overlap_count(entry, &query) > 0)
            .collect::<Vec<_>>();
        let pending_match_count = matched_entries.iter().filter(|entry| entry.pending).count();
        let entries = matched_entries
            .into_iter()
            .filter(|entry| !entry.pending)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Ok(SetupMatchStats {
                pending_match_count,
                ..SetupMatchStats::default()
            });
        }

        let resolved = entries
            .iter()
            .filter(|entry| entry.alpha_return.is_some() && entry.raw_return.is_some())
            .collect::<Vec<_>>();
        if resolved.is_empty() {
            return Ok(SetupMatchStats {
                total_match_count: entries.len(),
                pending_match_count,
                ..SetupMatchStats::default()
            });
        }

        let mut stats = Self::build_stats_from_resolved_entries(
            &resolved.into_iter().cloned().collect::<Vec<_>>(),
        );
        stats.total_match_count = entries.len();
        stats.pending_match_count = pending_match_count;
        Ok(stats)
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
            "overall": super::super::stats::summarize_entries(&resolved),
            "calibration_profile": super::super::stats::suggested_calibration_profile(&resolved),
            "by_rating": super::super::stats::group_summary(&resolved, |entry| entry.rating.clone()),
            "by_action": super::super::stats::group_summary(&resolved, |entry| {
                if entry.action.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    entry.action.clone()
                }
            }),
            "by_market": super::super::stats::group_summary(&resolved, |entry| {
                if entry.market.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    entry.market.clone()
                }
            }),
            "by_confidence_band": super::super::stats::group_summary(&resolved, |entry| {
                super::super::stats::bucket_score(entry.confidence_score, &[40, 55, 70, 85])
            }),
            "by_direction_band": super::super::stats::group_summary(&resolved, |entry| {
                super::super::stats::bucket_signed_score(entry.direction_score, &[-60, -20, 20, 60])
            }),
            "by_action_band": super::super::stats::group_summary(&resolved, |entry| {
                super::super::stats::bucket_score(entry.action_score, &[40, 55, 70, 85])
            }),
        }))
    }
}
