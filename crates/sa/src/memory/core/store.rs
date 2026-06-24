use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde_json::json;

use super::super::stats::MemoryOutcomeUpdate;
use super::super::{ENTRY_SEPARATOR, MemoryEntry, ResearchMemoryRecord};

impl super::super::TradingMemoryLog {
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
        vector_store: super::super::VectorMemoryBackend,
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
        if let Some(entry) =
            self.load_entries().await?.into_iter().find(|item| {
                item.ticker.eq_ignore_ascii_case(ticker) && item.trade_date == trade_date
            })
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
