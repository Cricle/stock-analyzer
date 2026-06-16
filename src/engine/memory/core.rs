use std::collections::HashSet;
use std::sync::Arc;

use serde_json::json;

use super::stats::{
    MemoryOutcomeUpdate, SetupMatchStats, bucket_score, bucket_signed_score, group_summary,
    suggested_calibration_profile, summarize_entries,
};
use super::{
    DecisionRecord, ENTRY_SEPARATOR, FilesystemMemoryStore, MemoryContextBundle, MemoryEntry,
    MemoryQuery, TradingMemoryLog, WEAK_SETUP_TAGS,
};

impl TradingMemoryLog {
    pub(super) fn compact_context_text(text: &str, max_chars: usize) -> String {
        let normalized = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let chars = normalized.chars().collect::<Vec<_>>();
        if chars.len() <= max_chars {
            return normalized;
        }
        let head_len = ((max_chars as f32) * 0.7).round() as usize;
        let tail_len = max_chars.saturating_sub(head_len + 24);
        let head = chars.iter().take(head_len).collect::<String>();
        let tail = chars
            .iter()
            .rev()
            .take(tail_len)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        if tail.trim().is_empty() {
            head.trim().to_string()
        } else {
            format!(
                "{}\n\n...[memory truncated]...\n\n{}",
                head.trim(),
                tail.trim()
            )
        }
    }

    fn effective_setup_tags(tags: &[String]) -> Vec<String> {
        let mut output = Vec::new();
        for tag in tags {
            let normalized = tag.trim();
            if normalized.is_empty()
                || WEAK_SETUP_TAGS
                    .iter()
                    .any(|weak| weak.eq_ignore_ascii_case(normalized))
            {
                continue;
            }
            if !output
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(normalized))
            {
                output.push(normalized.to_string());
            }
        }
        output
    }

    fn setup_overlap_count(entry: &MemoryEntry, query: &MemoryQuery) -> usize {
        let effective_query = Self::effective_setup_tags(&query.setup_tags);
        if effective_query.is_empty() {
            return 0;
        }
        let effective_entry = Self::effective_setup_tags(&entry.setup_tags);
        effective_query
            .iter()
            .filter(|tag| {
                effective_entry
                    .iter()
                    .any(|entry_tag| entry_tag.eq_ignore_ascii_case(tag))
            })
            .count()
    }

    pub fn new(store: Arc<dyn super::MemoryStore>, max_entries: usize) -> Self {
        Self { store, max_entries }
    }

    /// Convenience constructor using filesystem storage.
    pub fn with_filesystem(data_dir: &str, max_entries: usize) -> anyhow::Result<Self> {
        let store = Arc::new(FilesystemMemoryStore::new(data_dir)?);
        Ok(Self { store, max_entries })
    }

    pub async fn store_decision(
        &self,
        record: &DecisionRecord<'_>,
    ) -> anyhow::Result<()> {
        // Check for duplicate pending entry
        let entries = self.store.load_entries().await?;
        if entries.iter().any(|e| {
            e.pending && e.trade_date == record.trade_date && e.ticker == record.ticker
        }) {
            return Ok(());
        }

        let tag = format!("[{} | {} | {} | pending]", record.trade_date, record.ticker, record.rating);
        let meta = json!({
            "ticker": record.ticker,
            "trade_date": record.trade_date,
            "rating": record.rating,
            "action": record.action,
            "market": record.market,
            "direction_score": record.direction_score,
            "confidence_score": record.confidence_score,
            "action_score": record.action_score,
            "stock_name": record.research.map(|item| item.stock_name.clone()).unwrap_or_default(),
            "summary": record.research.map(|item| item.summary.clone()).unwrap_or_default(),
            "risk_assessment": record.research.map(|item| item.risk_assessment.clone()).unwrap_or_default(),
            "rationale": record.research.map(|item| item.rationale.clone()).unwrap_or_default(),
            "structured_risk": record.research.map(|item| item.structured_risk.clone()).unwrap_or_default(),
            "structured_reflection": record.research.map(|item| item.structured_reflection.clone()).unwrap_or_default(),
            "trigger_checklist": record.research.map(|item| item.trigger_checklist.clone()).unwrap_or_default(),
            "blocking_gaps": record.research.map(|item| item.blocking_gaps.clone()).unwrap_or_default(),
            "setup_tags": record.research.map(|item| item.setup_tags.clone()).unwrap_or_default(),
            "execution_boundary_complete": record.research.map(|item| item.execution_boundary_complete).unwrap_or(false),
            "pending": true,
        });
        let entry = format!(
            "{tag}\n\nMETA:\n{}\n\nDECISION:\n{}{ENTRY_SEPARATOR}",
            serde_json::to_string_pretty(&meta)?,
            record.final_trade_decision,
        );
        self.store.append_entry(&entry).await?;
        Ok(())
    }

    pub async fn load_entries(&self) -> anyhow::Result<Vec<MemoryEntry>> {
        self.store.load_entries().await
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
        self.local_past_context_bundle(query, same_limit, cross_limit).await
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

        let mut parts = Vec::new();
        if !same_entries.is_empty() {
            parts.push(format!(
                "Past analyses of {} (most relevant first):",
                query.ticker.trim().to_uppercase()
            ));
            parts.extend(same_entries.iter().map(Self::format_full_entry));
        }
        if !cross_entries.is_empty() {
            parts.push("Recent cross-ticker lessons:".to_string());
            parts.extend(cross_entries.iter().map(Self::format_reflection_only));
        }

        Ok(MemoryContextBundle {
            context_text: Self::compact_context_text(&parts.join("\n\n"), 3200),
            source: "local".to_string(),
            retrieval_mode: "local".to_string(),
            embedding_provider: "disabled".to_string(),
            embedding_failure_reason: None,
            same_ticker_count: same_entries.len(),
            cross_ticker_count: cross_entries.len(),
            vector_hit_count: 0,
            effective_top_k: same_limit + cross_limit,
            same_ticker_highlights: same_entries
                .iter()
                .take(3)
                .map(|entry| Self::highlight_from_entry(entry, true))
                .collect(),
            cross_ticker_highlights: cross_entries
                .iter()
                .take(3)
                .map(|entry| Self::highlight_from_entry(entry, false))
                .collect(),
        })
    }
}
impl TradingMemoryLog {

    pub(crate) async fn setup_match_stats(&self, query: &MemoryQuery) -> anyhow::Result<SetupMatchStats> {
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

    pub async fn batch_update_with_outcomes(
        &self,
        updates: &[MemoryOutcomeUpdate],
    ) -> anyhow::Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let entries = self.store.load_entries().await?;
        if entries.is_empty() {
            return Ok(());
        }

        let mut remaining = updates.to_vec();
        let mut new_blocks = Vec::new();

        for entry in &entries {
            let matched_index = remaining.iter().position(|update| {
                entry.pending
                    && entry.ticker.eq_ignore_ascii_case(&update.ticker)
                    && entry.trade_date == update.trade_date
            });

            if let Some(index) = matched_index {
                let update = remaining.remove(index);
                let raw_pct = format!("{:+.1}%", update.outcome_return * 100.0);
                let alpha_pct = format!(
                    "{:+.1}%",
                    (update.outcome_return - update.benchmark_return) * 100.0
                );
                let new_tag = format!(
                    "[{} | {} | {} | {} | {} | {}d]",
                    update.trade_date,
                    update.ticker,
                    entry.rating,
                    raw_pct,
                    alpha_pct,
                    update.holding_days
                );
                let meta = json!({
                    "ticker": entry.ticker,
                    "trade_date": entry.trade_date,
                    "rating": entry.rating,
                    "action": entry.action,
                    "market": entry.market,
                    "direction_score": entry.direction_score,
                    "confidence_score": entry.confidence_score,
                    "action_score": entry.action_score,
                    "stock_name": entry.stock_name,
                    "summary": entry.summary,
                    "risk_assessment": entry.risk_assessment,
                    "rationale": entry.rationale,
                    "structured_risk": entry.structured_risk,
                    "structured_reflection": entry.structured_reflection,
                    "trigger_checklist": entry.trigger_checklist,
                    "blocking_gaps": entry.blocking_gaps,
                    "setup_tags": entry.setup_tags,
                    "execution_boundary_complete": entry.execution_boundary_complete,
                    "pending": false,
                });
                let meta_str = serde_json::to_string_pretty(&meta).unwrap_or_default();
                new_blocks.push(format!(
                    "{new_tag}\n\nMETA:\n{meta_str}\n\nDECISION:\n{}",
                    entry.final_trade_decision
                ));
            } else {
                // Reconstruct block
                let pending = entry.pending;
                let tag = if pending {
                    format!("[{} | {} | {} | pending]", entry.trade_date, entry.ticker, entry.rating)
                } else {
                    let raw = entry.raw_return.map(|r| format!("{:+.1}%", r * 100.0)).unwrap_or_default();
                    let alpha = entry.alpha_return.map(|r| format!("{:+.1}%", r * 100.0)).unwrap_or_default();
                    let days = entry.holding_days.unwrap_or(0);
                    format!("[{} | {} | {} | {raw} | {alpha} | {days}d]", entry.trade_date, entry.ticker, entry.rating)
                };
                let meta = json!({
                    "ticker": entry.ticker,
                    "trade_date": entry.trade_date,
                    "rating": entry.rating,
                    "action": entry.action,
                    "market": entry.market,
                    "direction_score": entry.direction_score,
                    "confidence_score": entry.confidence_score,
                    "action_score": entry.action_score,
                    "stock_name": entry.stock_name,
                    "summary": entry.summary,
                    "risk_assessment": entry.risk_assessment,
                    "rationale": entry.rationale,
                    "structured_risk": entry.structured_risk,
                    "structured_reflection": entry.structured_reflection,
                    "trigger_checklist": entry.trigger_checklist,
                    "blocking_gaps": entry.blocking_gaps,
                    "setup_tags": entry.setup_tags,
                    "execution_boundary_complete": entry.execution_boundary_complete,
                    "pending": entry.pending,
                });
                let meta_str = serde_json::to_string_pretty(&meta).unwrap_or_default();
                let mut block = format!(
                    "{tag}\n\nMETA:\n{meta_str}\n\nDECISION:\n{}",
                    entry.final_trade_decision
                );
                if let Some(ref refl) = entry.reflection {
                    block.push_str(&format!("\n\nREFLECTION:\n{refl}"));
                }
                new_blocks.push(block);
            }
        }

        let rotated = self.apply_rotation(new_blocks);
        self.store
            .write_all(&format!(
                "{}{}",
                rotated.join(ENTRY_SEPARATOR),
                ENTRY_SEPARATOR
            ))
            .await?;
        Ok(())
    }
}
impl TradingMemoryLog {

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
