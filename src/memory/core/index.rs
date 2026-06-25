use std::collections::HashSet;

use super::super::stats::MemoryOutcomeUpdate;
use super::super::{MemoryEntry, MemoryQuery, WEAK_SETUP_TAGS};

impl super::super::TradingMemoryLog {
    pub(in crate::memory) fn compact_context_text(text: &str, max_chars: usize) -> String {
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

    pub(in crate::memory) fn env_truthy(key: &str, default: bool) -> bool {
        std::env::var(key)
            .map(|v| crate::env_config::env_flag_value(&v))
            .unwrap_or(default)
    }

    pub(in crate::memory) fn non_empty_env(keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }

    pub(crate) fn rag_runtime_snapshot() -> super::super::RagRuntimeSnapshot {
        super::super::RagRuntimeSnapshot {
            enabled: Self::env_truthy("RAG_ENABLED", true),
            embedding_provider: Self::non_empty_env(&["RAG_EMBEDDING_PROVIDER"])
                .unwrap_or_else(|| "fastembed".to_string()),
            embedding_model: Self::non_empty_env(&["RAG_EMBEDDING_MODEL"])
                .unwrap_or_else(|| "BAAI/bge-small-en-v1.5".to_string()),
        }
    }

    pub(in crate::memory) fn effective_setup_tags(tags: &[String]) -> Vec<String> {
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

    pub(in crate::memory) fn setup_overlap_count(
        entry: &MemoryEntry,
        query: &MemoryQuery,
    ) -> usize {
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

    pub(in crate::memory) fn entry_id(ticker: &str, trade_date: &str) -> String {
        format!("{}:{}", ticker.trim().to_uppercase(), trade_date.trim())
    }

    pub(in crate::memory) fn research_entry_id(ticker: &str, trade_date: &str) -> String {
        format!(
            "research:{}:{}",
            ticker.trim().to_uppercase(),
            trade_date.trim()
        )
    }

    pub(in crate::memory) fn entry_text(entry: &MemoryEntry) -> String {
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

    pub(in crate::memory) fn query_text(ticker: &str) -> String {
        format!(
            "stock {} decision execution risk reflection lesson analysis",
            ticker.trim().to_uppercase()
        )
    }

    pub(in crate::memory) fn research_query_text(query: &MemoryQuery) -> String {
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

    pub(in crate::memory) fn dedupe_entries(
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

    pub(in crate::memory) fn rerank_entries(
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
                self.vector_upsert_entry(&entry).await?;
            }
            if pending_ids.is_empty() {
                break;
            }
        }
        Ok(())
    }
}
