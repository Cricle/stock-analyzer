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
            .map(|v| crate::engine::config::env_flag_value(&v))
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

include!("storage.rs");
include!("helpers.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_context_text_short_text() {
        let text = "Hello world\nSecond line";
        let result = TradingMemoryLog::compact_context_text(text, 1000);
        assert_eq!(result, "Hello world\nSecond line");
    }

    #[test]
    fn test_compact_context_text_empty() {
        let result = TradingMemoryLog::compact_context_text("", 100);
        assert_eq!(result, "");
    }

    #[test]
    fn test_compact_context_text_truncates_long() {
        let text = "a".repeat(5000);
        let result = TradingMemoryLog::compact_context_text(&text, 200);
        assert!(result.len() <= 250); // some slack for truncation markers
        assert!(result.contains("memory truncated"));
    }

    #[test]
    fn test_compact_context_text_removes_blank_lines() {
        let text = "line1\n\n\n  \nline2";
        let result = TradingMemoryLog::compact_context_text(text, 1000);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_effective_setup_tags_filters_empty() {
        let tags = vec!["".to_string(), "  ".to_string(), "breakout".to_string()];
        let result = TradingMemoryLog::effective_setup_tags(&tags);
        assert_eq!(result, vec!["breakout"]);
    }

    #[test]
    fn test_effective_setup_tags_deduplicates_case_insensitive() {
        let tags = vec![
            "Breakout".to_string(),
            "BREAKOUT".to_string(),
            "breakout".to_string(),
        ];
        let result = TradingMemoryLog::effective_setup_tags(&tags);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "Breakout");
    }

    #[test]
    fn test_setup_overlap_count_no_overlap() {
        let entry = MemoryEntry {
            setup_tags: vec!["breakout".to_string()],
            ..Default::default()
        };
        let query = MemoryQuery {
            setup_tags: vec!["mean_reversion".to_string()],
            ..Default::default()
        };
        assert_eq!(TradingMemoryLog::setup_overlap_count(&entry, &query), 0);
    }

    #[test]
    fn test_setup_overlap_count_with_overlap() {
        let entry = MemoryEntry {
            setup_tags: vec!["breakout".to_string(), "momentum".to_string()],
            ..Default::default()
        };
        let query = MemoryQuery {
            setup_tags: vec!["breakout".to_string(), "value".to_string()],
            ..Default::default()
        };
        assert_eq!(TradingMemoryLog::setup_overlap_count(&entry, &query), 1);
    }

    #[test]
    fn test_setup_overlap_count_empty_query() {
        let entry = MemoryEntry {
            setup_tags: vec!["breakout".to_string()],
            ..Default::default()
        };
        let query = MemoryQuery {
            setup_tags: vec![],
            ..Default::default()
        };
        assert_eq!(TradingMemoryLog::setup_overlap_count(&entry, &query), 0);
    }

    #[test]
    fn test_entry_id_format() {
        let id = TradingMemoryLog::entry_id("600519", "2024-01-15");
        assert_eq!(id, "600519:2024-01-15");
    }

    #[test]
    fn test_entry_id_trims_and_uppercases() {
        let id = TradingMemoryLog::entry_id("  aapl  ", "2024-01-15");
        assert_eq!(id, "AAPL:2024-01-15");
    }

    #[test]
    fn test_research_entry_id_format() {
        let id = TradingMemoryLog::research_entry_id("600519", "2024-01-15");
        assert_eq!(id, "research:600519:2024-01-15");
    }

    #[test]
    fn test_query_text_format() {
        let text = TradingMemoryLog::query_text("AAPL");
        assert!(text.contains("AAPL"));
        assert!(text.contains("decision"));
        assert!(text.contains("risk"));
    }

    #[test]
    fn test_research_query_text_basic() {
        let query = MemoryQuery {
            ticker: "AAPL".to_string(),
            market: "US".to_string(),
            setup_tags: vec![],
            user_id: String::new(),
        };
        let text = TradingMemoryLog::research_query_text(&query);
        assert!(text.contains("AAPL"));
        assert!(text.contains("US"));
    }

    #[test]
    fn test_research_query_text_with_tags() {
        let query = MemoryQuery {
            ticker: "AAPL".to_string(),
            market: "US".to_string(),
            setup_tags: vec!["breakout".to_string(), "momentum".to_string()],
            user_id: String::new(),
        };
        let text = TradingMemoryLog::research_query_text(&query);
        assert!(text.contains("breakout"));
        assert!(text.contains("momentum"));
    }

    #[test]
    fn test_entry_text_basic() {
        let entry = MemoryEntry {
            ticker: "AAPL".to_string(),
            trade_date: "2024-01-15".to_string(),
            market: "US".to_string(),
            rating: "BUY".to_string(),
            action: "buy".to_string(),
            final_trade_decision: "Strong buy".to_string(),
            reflection: None,
            ..Default::default()
        };
        let text = TradingMemoryLog::entry_text(&entry);
        assert!(text.contains("AAPL"));
        assert!(text.contains("BUY"));
        assert!(text.contains("Strong buy"));
    }
}
