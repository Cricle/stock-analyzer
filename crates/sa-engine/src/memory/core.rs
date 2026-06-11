use std::{collections::HashSet, fs, path::PathBuf};

use anyhow::Context;
use serde_json::json;

use super::stats::{
    MemoryOutcomeUpdate, SetupMatchStats, bucket_score, bucket_signed_score, group_summary,
    suggested_calibration_profile, summarize_entries,
};
use super::{
    ENTRY_SEPARATOR, MemoryContextBundle, MemoryEntry, MemoryQuery, ResearchMemoryRecord,
    TradingMemoryLog, WEAK_SETUP_TAGS,
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

    pub(super) fn env_truthy(key: &str, default: bool) -> bool {
        std::env::var(key)
            .map(|v| crate::config::env_flag_value(&v))
            .unwrap_or(default)
    }

    pub(super) fn non_empty_env(keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }

    pub(crate) fn rag_runtime_snapshot() -> super::RagRuntimeSnapshot {
        super::RagRuntimeSnapshot {
            enabled: Self::env_truthy("RAG_ENABLED", true),
            qdrant_url_configured: Self::non_empty_env(&["RAG_QDRANT_URL", "QDRANT_URL"]).is_some(),
            qdrant_collection: Self::non_empty_env(&["RAG_QDRANT_COLLECTION", "QDRANT_COLLECTION"])
                .unwrap_or_else(|| "tradingagents_memory".to_string()),
            embedding_provider: Self::non_empty_env(&["RAG_EMBEDDING_PROVIDER"])
                .unwrap_or_else(|| "fastembed".to_string()),
            embedding_model: Self::non_empty_env(&["RAG_EMBEDDING_MODEL"])
                .unwrap_or_else(|| "BAAI/bge-small-en-v1.5".to_string()),
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

    pub fn new(data_dir: &str, max_entries: usize) -> anyhow::Result<Self> {
        let base = PathBuf::from(data_dir).join("memory");
        fs::create_dir_all(&base)
            .with_context(|| format!("failed to create {}", base.display()))?;
        let rag = Self::load_rag_config();
        let vector_store = Self::build_vector_backend(&rag);
        let embedding = Self::build_embedding_backend(data_dir, &rag);
        Ok(Self {
            log_path: base.join("decisions.md"),
            max_entries,
            vector_store,
            rag,
            embedding,
        })
    }

    /// Create a new TradingMemoryLog with an injected vector store.
    pub fn with_vector_store(
        data_dir: &str,
        max_entries: usize,
        vector_store: super::VectorMemoryBackend,
    ) -> anyhow::Result<Self> {
        let base = PathBuf::from(data_dir).join("memory");
        fs::create_dir_all(&base)
            .with_context(|| format!("failed to create {}", base.display()))?;
        let rag = Self::load_rag_config();
        let embedding = Self::build_embedding_backend(data_dir, &rag);
        Ok(Self {
            log_path: base.join("decisions.md"),
            max_entries,
            vector_store: Some(vector_store),
            rag,
            embedding,
        })
    }

    pub async fn store_decision(
        &self,
        ticker: &str,
        trade_date: &str,
        final_trade_decision: &str,
        rating: &str,
        action: &str,
        market: &str,
        direction_score: i32,
        confidence_score: i32,
        action_score: i32,
        research: Option<&ResearchMemoryRecord>,
    ) -> anyhow::Result<()> {
        if self.log_path.exists() {
            let raw = tokio::fs::read_to_string(&self.log_path)
                .await
                .with_context(|| format!("failed to read {}", self.log_path.display()))?;
            let pending_tag_prefix = format!("[{trade_date} | {ticker} |");
            if raw
                .lines()
                .any(|line| line.starts_with(&pending_tag_prefix) && line.ends_with("| pending]"))
            {
                return Ok(());
            }
        }

        let tag = format!("[{trade_date} | {ticker} | {rating} | pending]");
        let meta = json!({
            "ticker": ticker,
            "trade_date": trade_date,
            "rating": rating,
            "action": action,
            "market": market,
            "direction_score": direction_score,
            "confidence_score": confidence_score,
            "action_score": action_score,
            "stock_name": research.map(|item| item.stock_name.clone()).unwrap_or_default(),
            "summary": research.map(|item| item.summary.clone()).unwrap_or_default(),
            "risk_assessment": research.map(|item| item.risk_assessment.clone()).unwrap_or_default(),
            "rationale": research.map(|item| item.rationale.clone()).unwrap_or_default(),
            "structured_risk": research.map(|item| item.structured_risk.clone()).unwrap_or_default(),
            "structured_reflection": research.map(|item| item.structured_reflection.clone()).unwrap_or_default(),
            "trigger_checklist": research.map(|item| item.trigger_checklist.clone()).unwrap_or_default(),
            "blocking_gaps": research.map(|item| item.blocking_gaps.clone()).unwrap_or_default(),
            "setup_tags": research.map(|item| item.setup_tags.clone()).unwrap_or_default(),
            "execution_boundary_complete": research.map(|item| item.execution_boundary_complete).unwrap_or(false),
            "pending": true,
        });
        let entry = format!(
            "{tag}\n\nMETA:\n{}\n\nDECISION:\n{final_trade_decision}{ENTRY_SEPARATOR}",
            serde_json::to_string_pretty(&meta)?
        );
        let mut current = if self.log_path.exists() {
            tokio::fs::read_to_string(&self.log_path)
                .await
                .with_context(|| format!("failed to read {}", self.log_path.display()))?
        } else {
            String::new()
        };
        current.push_str(&entry);
        tokio::fs::write(&self.log_path, current)
            .await
            .with_context(|| format!("failed to write {}", self.log_path.display()))?;
        Ok(())
    }

    pub async fn load_entries(&self) -> anyhow::Result<Vec<MemoryEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let text = tokio::fs::read_to_string(&self.log_path)
            .await
            .with_context(|| format!("failed to read {}", self.log_path.display()))?;
        Ok(text
            .split(ENTRY_SEPARATOR)
            .filter_map(|raw| Self::parse_entry(raw))
            .collect())
    }

    pub async fn past_context_async(
        &self,
        ticker: &str,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<String> {
        Ok(self
            .past_context_bundle_async_with_query(
                &MemoryQuery {
                    ticker: ticker.to_string(),
                    market: String::new(),
                    setup_tags: Vec::new(),
                    user_id: String::new(),
                },
                same_limit,
                cross_limit,
            )
            .await?
            .context_text)
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
            .qdrant_past_context_bundle(query, same_limit, cross_limit)
            .await?
            .filter(|value| !value.context_text.trim().is_empty())
        {
            return Ok(context);
        }
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

    pub async fn update_outcome(
        &self,
        ticker: &str,
        trade_date: &str,
        outcome_return: f64,
        benchmark_return: f64,
        reflection: String,
    ) -> anyhow::Result<()> {
        if !self.log_path.exists() {
            return Ok(());
        }

        let text = tokio::fs::read_to_string(&self.log_path)
            .await
            .with_context(|| format!("failed to read {}", self.log_path.display()))?;
        let blocks = text.split(ENTRY_SEPARATOR).collect::<Vec<_>>();
        let pending_prefix = format!("[{trade_date} | {ticker} |");
        let raw_pct = format!("{:+.1}%", outcome_return * 100.0);
        let alpha_pct = format!("{:+.1}%", (outcome_return - benchmark_return) * 100.0);
        let holding_days = 5usize;

        let mut updated = false;
        let mut new_blocks = Vec::new();
        for block in blocks {
            let stripped = block.trim();
            if stripped.is_empty() {
                continue;
            }
            let lines = stripped.lines().collect::<Vec<_>>();
            let tag_line = lines.first().copied().unwrap_or_default().trim();
            if !updated && tag_line.starts_with(&pending_prefix) && tag_line.ends_with("| pending]")
            {
                let fields = tag_line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split('|')
                    .map(|item| item.trim())
                    .collect::<Vec<_>>();
                let rating = fields.get(2).copied().unwrap_or("Hold");
                let new_tag = format!(
                    "[{trade_date} | {ticker} | {rating} | {raw_pct} | {alpha_pct} | {holding_days}d]"
                );
                let rest = lines.into_iter().skip(1).collect::<Vec<_>>().join("\n");
                new_blocks.push(format!(
                    "{new_tag}\n\n{}\n\nREFLECTION:\n{}",
                    rest.trim_start(),
                    reflection
                ));
                updated = true;
            } else {
                new_blocks.push(stripped.to_string());
            }
        }

        if !updated {
            return Ok(());
        }

        let rotated = self.apply_rotation(new_blocks);
        tokio::fs::write(
            &self.log_path,
            format!("{}{}", rotated.join(ENTRY_SEPARATOR), ENTRY_SEPARATOR),
        )
        .await
        .with_context(|| format!("failed to write {}", self.log_path.display()))?;
        Ok(())
    }

    pub async fn store_decision_async(
        &self,
        ticker: &str,
        trade_date: &str,
        final_trade_decision: &str,
        rating: &str,
        action: &str,
        market: &str,
        direction_score: i32,
        confidence_score: i32,
        action_score: i32,
        research: Option<&ResearchMemoryRecord>,
        user_id: &str,
    ) -> anyhow::Result<()> {
        self.store_decision(
            ticker,
            trade_date,
            final_trade_decision,
            rating,
            action,
            market,
            direction_score,
            confidence_score,
            action_score,
            research,
        )
        .await?;
        self.qdrant_upsert_entry(&MemoryEntry {
            ticker: ticker.to_string(),
            trade_date: trade_date.to_string(),
            rating: rating.to_string(),
            action: action.to_string(),
            market: market.to_string(),
            stock_name: research
                .map(|item| item.stock_name.clone())
                .unwrap_or_default(),
            direction_score: Some(direction_score),
            confidence_score: Some(confidence_score),
            action_score: Some(action_score),
            summary: research
                .map(|item| item.summary.clone())
                .unwrap_or_default(),
            risk_assessment: research
                .map(|item| item.risk_assessment.clone())
                .unwrap_or_default(),
            rationale: research
                .map(|item| item.rationale.clone())
                .unwrap_or_default(),
            structured_risk: research
                .map(|item| item.structured_risk.clone())
                .unwrap_or_default(),
            structured_reflection: research
                .map(|item| item.structured_reflection.clone())
                .unwrap_or_default(),
            trigger_checklist: research
                .map(|item| item.trigger_checklist.clone())
                .unwrap_or_default(),
            blocking_gaps: research
                .map(|item| item.blocking_gaps.clone())
                .unwrap_or_default(),
            setup_tags: research
                .map(|item| item.setup_tags.clone())
                .unwrap_or_default(),
            execution_boundary_complete: research.map(|item| item.execution_boundary_complete),
            final_trade_decision: final_trade_decision.to_string(),
            reflection: None,
            raw_return: None,
            alpha_return: None,
            holding_days: None,
            pending: true,
            user_id: user_id.to_string(),
        })
        .await?;
        if let Some(research) = research {
            self.qdrant_upsert_research_record(
                ticker, trade_date, rating, action, market, research,
            )
            .await?;
        }
        Ok(())
    }

    /// Store a decision from a MemoryEntry struct directly.
    pub async fn store_entry_async(&self, entry: &MemoryEntry) -> anyhow::Result<()> {
        self.store_decision(
            &entry.ticker,
            &entry.trade_date,
            &entry.final_trade_decision,
            &entry.rating,
            &entry.action,
            &entry.market,
            entry.direction_score.unwrap_or(0),
            entry.confidence_score.unwrap_or(0),
            entry.action_score.unwrap_or(0),
            None,
        )
        .await?;
        self.qdrant_upsert_entry(entry).await
    }

    pub async fn update_outcome_async(
        &self,
        ticker: &str,
        trade_date: &str,
        outcome_return: f64,
        benchmark_return: f64,
        reflection: String,
    ) -> anyhow::Result<()> {
        self.update_outcome(
            ticker,
            trade_date,
            outcome_return,
            benchmark_return,
            reflection,
        )
        .await?;
        if let Some(entry) = self
            .load_entries()
            .await?
            .into_iter()
            .find(|item| item.ticker.eq_ignore_ascii_case(ticker) && item.trade_date == trade_date)
        {
            self.qdrant_upsert_entry(&entry).await?;
        }
        Ok(())
    }

    pub async fn batch_update_with_outcomes(
        &self,
        updates: &[MemoryOutcomeUpdate],
    ) -> anyhow::Result<()> {
        if !self.log_path.exists() || updates.is_empty() {
            return Ok(());
        }

        let text = tokio::fs::read_to_string(&self.log_path)
            .await
            .with_context(|| format!("failed to read {}", self.log_path.display()))?;
        let blocks = text.split(ENTRY_SEPARATOR).collect::<Vec<_>>();
        let mut remaining = updates.to_vec();
        let mut new_blocks = Vec::new();

        for block in blocks {
            let stripped = block.trim();
            if stripped.is_empty() {
                continue;
            }
            let lines = stripped.lines().collect::<Vec<_>>();
            let tag_line = lines.first().copied().unwrap_or_default().trim();

            let matched_index = remaining.iter().position(|update| {
                let pending_prefix = format!("[{} | {} |", update.trade_date, update.ticker);
                tag_line.starts_with(&pending_prefix) && tag_line.ends_with("| pending]")
            });

            if let Some(index) = matched_index {
                let update = remaining.remove(index);
                let fields = tag_line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split('|')
                    .map(|item| item.trim())
                    .collect::<Vec<_>>();
                let rating = fields.get(2).copied().unwrap_or("Hold");
                let raw_pct = format!("{:+.1}%", update.outcome_return * 100.0);
                let alpha_pct = format!(
                    "{:+.1}%",
                    (update.outcome_return - update.benchmark_return) * 100.0
                );
                let new_tag = format!(
                    "[{} | {} | {} | {} | {} | {}d]",
                    update.trade_date,
                    update.ticker,
                    rating,
                    raw_pct,
                    alpha_pct,
                    update.holding_days
                );
                let rest = lines.into_iter().skip(1).collect::<Vec<_>>().join("\n");
                new_blocks.push(format!(
                    "{new_tag}\n\n{}\n\nREFLECTION:\n{}",
                    rest.trim_start(),
                    update.reflection
                ));
            } else {
                new_blocks.push(stripped.to_string());
            }
        }

        let rotated = self.apply_rotation(new_blocks);
        tokio::fs::write(
            &self.log_path,
            format!("{}{}", rotated.join(ENTRY_SEPARATOR), ENTRY_SEPARATOR),
        )
        .await
        .with_context(|| format!("failed to write {}", self.log_path.display()))?;
        Ok(())
    }
}
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
