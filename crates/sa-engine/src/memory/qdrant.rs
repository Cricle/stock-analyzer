//! Vector store operations for memory, using the trait-based VectorStore.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::stats::QdrantMemoryPayload;
use super::{
    MemoryContextBundle, MemoryEntry, MemoryQuery, ResearchMemoryRecord, TradingMemoryLog,
};
use sa_models::{StructuredReflection, StructuredRiskAssessment};

const MEMORY_VECTOR_COLLECTION: &str = "tradingagents_memory";

impl TradingMemoryLog {
    pub(super) fn qdrant_point_id(entry_id: &str) -> String {
        let digest = Sha256::digest(entry_id.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    }

    /// Insert a memory entry into the vector store.
    #[tracing::instrument(skip_all, fields(ticker = %entry.ticker, trade_date = %entry.trade_date))]
    pub(super) async fn qdrant_upsert_entry(&self, entry: &MemoryEntry) -> anyhow::Result<()> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(());
        };
        if !self.embedding.retrieval_enabled {
            tracing::warn!(
                provider = %self.embedding.provider,
                reason = self.embedding.failure_reason.as_deref().unwrap_or("unknown"),
                "skip vector decision upsert because embedding backend is unavailable"
            );
            return Ok(());
        }
        let started = std::time::Instant::now();
        let text = Self::entry_text(entry);
        let point_id = Self::qdrant_point_id(&Self::entry_id(&entry.ticker, &entry.trade_date));
        let payload = serde_json::json!({
            "memory_entry_id": Self::entry_id(&entry.ticker, &entry.trade_date),
            "ticker": entry.ticker,
            "ticker_lc": entry.ticker.to_ascii_lowercase(),
            "market_lc": entry.market.to_ascii_lowercase(),
            "trade_date": entry.trade_date,
            "rating": entry.rating,
            "action": entry.action,
            "market": entry.market,
            "direction_score": entry.direction_score,
            "confidence_score": entry.confidence_score,
            "action_score": entry.action_score,
            "summary": entry.summary,
            "risk_assessment": entry.risk_assessment,
            "rationale": entry.rationale,
            "structured_risk": entry.structured_risk,
            "structured_reflection": entry.structured_reflection,
            "trigger_checklist": entry.trigger_checklist,
            "blocking_gaps": entry.blocking_gaps,
            "setup_tags": entry.setup_tags,
            "execution_boundary_complete": entry.execution_boundary_complete,
            "final_trade_decision": entry.final_trade_decision,
            "reflection": entry.reflection,
            "raw_return": entry.raw_return,
            "alpha_return": entry.alpha_return,
            "holding_days": entry.holding_days,
            "pending": entry.pending,
            "user_id": entry.user_id,
            "entry_kind": "decision",
            "text": text,
            "embedding_provider": self.embedding.provider,
            "embedding_model": self.embedding.model
        });
        store
            .insert(
                MEMORY_VECTOR_COLLECTION,
                &point_id,
                &self.embed_text(&text),
                payload,
            )
            .await?;
        tracing::info!(
            op = "vector_upsert",
            ticker = %entry.ticker,
            date = %entry.trade_date,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "decision upsert ok"
        );
        Ok(())
    }

    /// Batch upsert multiple memory entries.
    #[allow(dead_code)]
    #[tracing::instrument(skip_all, fields(count = entries.len()))]
    pub(super) async fn qdrant_batch_upsert_entries(
        &self,
        entries: &[MemoryEntry],
    ) -> anyhow::Result<()> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(());
        };
        if !self.embedding.retrieval_enabled || entries.is_empty() {
            return Ok(());
        }
        let started = std::time::Instant::now();
        for entry in entries {
            let text = Self::entry_text(entry);
            let point_id = Self::qdrant_point_id(&Self::entry_id(&entry.ticker, &entry.trade_date));
            let payload = serde_json::json!({
                "memory_entry_id": Self::entry_id(&entry.ticker, &entry.trade_date),
                "ticker": entry.ticker,
                "ticker_lc": entry.ticker.to_ascii_lowercase(),
                "market_lc": entry.market.to_ascii_lowercase(),
                "trade_date": entry.trade_date,
                "rating": entry.rating,
                "action": entry.action,
                "market": entry.market,
                "direction_score": entry.direction_score,
                "confidence_score": entry.confidence_score,
                "action_score": entry.action_score,
                "summary": entry.summary,
                "risk_assessment": entry.risk_assessment,
                "rationale": entry.rationale,
                "structured_risk": entry.structured_risk,
                "structured_reflection": entry.structured_reflection,
                "trigger_checklist": entry.trigger_checklist,
                "blocking_gaps": entry.blocking_gaps,
                "setup_tags": entry.setup_tags,
                "execution_boundary_complete": entry.execution_boundary_complete,
                "final_trade_decision": entry.final_trade_decision,
                "reflection": entry.reflection,
                "raw_return": entry.raw_return,
                "alpha_return": entry.alpha_return,
                "holding_days": entry.holding_days,
                "pending": entry.pending,
                "user_id": entry.user_id,
                "entry_kind": "decision",
                "text": text,
                "embedding_provider": self.embedding.provider,
                "embedding_model": self.embedding.model
            });
            store
                .insert(
                    MEMORY_VECTOR_COLLECTION,
                    &point_id,
                    &self.embed_text(&text),
                    payload,
                )
                .await?;
        }
        tracing::info!(
            op = "vector_batch_upsert",
            count = entries.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "batch upsert ok"
        );
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(ticker = %ticker, trade_date = %trade_date))]
    pub(super) async fn qdrant_upsert_research_record(
        &self,
        ticker: &str,
        trade_date: &str,
        rating: &str,
        action: &str,
        market: &str,
        research: &ResearchMemoryRecord,
    ) -> anyhow::Result<()> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(());
        };
        if !self.embedding.retrieval_enabled {
            tracing::warn!(
                provider = %self.embedding.provider,
                reason = self.embedding.failure_reason.as_deref().unwrap_or("unknown"),
                "skip vector research upsert because embedding backend is unavailable"
            );
            return Ok(());
        }
        let text = [
            format!("ticker {}", ticker.trim().to_uppercase()),
            format!("stock {}", research.stock_name.trim()),
            format!("market {}", market.trim()),
            format!("rating {}", rating.trim()),
            format!("action {}", action.trim()),
            research.summary.clone(),
            research.risk_assessment.clone(),
            research.rationale.clone(),
            if research.trigger_checklist.is_empty() {
                String::new()
            } else {
                format!("triggers {}", research.trigger_checklist.join(" | "))
            },
            if research.blocking_gaps.is_empty() {
                String::new()
            } else {
                format!("blocking_gaps {}", research.blocking_gaps.join(" | "))
            },
            if research.setup_tags.is_empty() {
                String::new()
            } else {
                format!("setup_tags {}", research.setup_tags.join(" | "))
            },
            if research.structured_snapshot.is_null() {
                String::new()
            } else {
                format!("structured_snapshot {}", research.structured_snapshot)
            },
            format!(
                "execution_boundary_complete {}",
                if research.execution_boundary_complete {
                    "true"
                } else {
                    "false"
                }
            ),
        ]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

        let point_id = Self::qdrant_point_id(&Self::research_entry_id(ticker, trade_date));
        let payload = serde_json::json!({
            "memory_entry_id": Self::research_entry_id(ticker, trade_date),
            "ticker": ticker,
            "ticker_lc": ticker.to_ascii_lowercase(),
            "market_lc": market.to_ascii_lowercase(),
            "trade_date": trade_date,
            "rating": rating,
            "action": action,
            "market": market,
            "stock_name": research.stock_name,
            "summary": research.summary,
            "risk_assessment": research.risk_assessment,
            "rationale": research.rationale,
            "structured_risk": research.structured_risk,
            "structured_reflection": research.structured_reflection,
            "trigger_checklist": research.trigger_checklist,
            "blocking_gaps": research.blocking_gaps,
            "setup_tags": research.setup_tags,
            "structured_snapshot": research.structured_snapshot,
            "execution_boundary_complete": research.execution_boundary_complete,
            "pending": false,
            "user_id": "",
            "entry_kind": "research",
            "text": text,
            "embedding_provider": self.embedding.provider,
            "embedding_model": self.embedding.model
        });
        store
            .insert(
                MEMORY_VECTOR_COLLECTION,
                &point_id,
                &self.embed_text(&text),
                payload,
            )
            .await
    }

    #[tracing::instrument(skip_all, fields(ticker = %query.ticker))]
    pub(super) async fn qdrant_past_context_bundle(
        &self,
        query: &MemoryQuery,
        same_limit: usize,
        cross_limit: usize,
    ) -> anyhow::Result<Option<MemoryContextBundle>> {
        if !self.rag.enabled {
            return Ok(None);
        }
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(None);
        };
        if !self.embedding.retrieval_enabled {
            return Ok(None);
        }
        let same_fetch_limit = self
            .rag
            .top_k
            .max(same_limit.saturating_mul(3).max(same_limit));
        let cross_fetch_limit = self
            .rag
            .top_k
            .max(cross_limit.saturating_mul(3).max(cross_limit));
        let same_entries = self
            .vector_search_filtered(
                store,
                &self.embed_text(&Self::research_query_text(query)),
                same_fetch_limit,
                Some(&query.ticker),
                None,
                Some("research"),
            )
            .await?;
        let same_decisions = self
            .vector_search_filtered(
                store,
                &self.embed_text(&Self::query_text(&query.ticker)),
                same_fetch_limit,
                Some(&query.ticker),
                None,
                Some("decision"),
            )
            .await?;
        let cross_research_same_market = self
            .vector_search_filtered(
                store,
                &self.embed_text(&Self::research_query_text(query)),
                cross_fetch_limit,
                None,
                (!query.market.trim().is_empty()).then_some(query.market.as_str()),
                Some("research"),
            )
            .await?
            .into_iter()
            .filter(|entry| !entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .collect::<Vec<_>>();
        let cross_research_any_market = self
            .vector_search_filtered(
                store,
                &self.embed_text(&Self::research_query_text(query)),
                cross_fetch_limit,
                None,
                None,
                Some("research"),
            )
            .await?
            .into_iter()
            .filter(|entry| !entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .collect::<Vec<_>>();
        let vector = self.embed_text(&Self::query_text(&query.ticker));
        let same_entries = self.dedupe_entries(
            self.rerank_entries(
                same_entries.into_iter().chain(same_decisions).collect(),
                query,
                true,
            ),
            same_limit,
        );
        let cross_entries = self
            .vector_search_filtered(
                store,
                &vector,
                cross_fetch_limit,
                None,
                (!query.market.trim().is_empty()).then_some(query.market.as_str()),
                Some("decision"),
            )
            .await?
            .into_iter()
            .filter(|entry| !entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .chain(cross_research_same_market)
            .chain(cross_research_any_market)
            .collect::<Vec<_>>();
        let cross_entries = self.dedupe_entries(
            self.rerank_entries(cross_entries, query, false),
            cross_limit,
        );
        if same_entries.is_empty() && cross_entries.is_empty() {
            return Ok(None);
        }

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
        Ok(Some(MemoryContextBundle {
            context_text: Self::compact_context_text(&parts.join("\n\n"), 3200),
            source: "vector".to_string(),
            retrieval_mode: "vector".to_string(),
            embedding_provider: self.embedding.provider.clone(),
            embedding_failure_reason: self.embedding.failure_reason.clone(),
            same_ticker_count: same_entries.len(),
            cross_ticker_count: cross_entries.len(),
            vector_hit_count: same_entries.len() + cross_entries.len(),
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
        }))
    }

    /// Search the vector store and post-filter by payload fields.
    ///
    /// TODO: The VectorStore trait doesn't support server-side filtering.
    /// We fetch extra results and filter client-side. Consider extending
    /// VectorStore with a filtered search method.
    pub(super) async fn vector_search_filtered(
        &self,
        store: &dyn sa_models::VectorStore,
        embedding: &[f32],
        limit: usize,
        ticker: Option<&str>,
        market: Option<&str>,
        entry_kind: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // Fetch more than needed to compensate for client-side filtering
        let fetch_limit = (limit * 4).max(limit + 20);
        let hits = store
            .search(MEMORY_VECTOR_COLLECTION, embedding, fetch_limit)
            .await?;
        let mut results = Vec::new();
        for hit in hits {
            let Some(payload_value) = hit.payload.as_object() else {
                continue;
            };
            // Client-side filtering
            if let Some(ticker) = ticker {
                let hit_ticker = payload_value
                    .get("ticker_lc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !hit_ticker.eq_ignore_ascii_case(ticker) {
                    continue;
                }
            }
            if let Some(market) = market {
                let hit_market = payload_value
                    .get("market_lc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !hit_market.eq_ignore_ascii_case(market) {
                    continue;
                }
            }
            if let Some(entry_kind) = entry_kind {
                let hit_kind = payload_value
                    .get("entry_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if hit_kind != entry_kind {
                    continue;
                }
            }
            // Skip pending entries
            let pending = payload_value
                .get("pending")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if pending {
                continue;
            }
            let payload: QdrantMemoryPayload = match serde_json::from_value(hit.payload) {
                Ok(p) => p,
                Err(_) => continue,
            };
            results.push(Self::payload_to_entry(payload));
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    pub(super) fn payload_to_entry(payload: QdrantMemoryPayload) -> MemoryEntry {
        let summary = payload.summary.clone().unwrap_or_default();
        MemoryEntry {
            ticker: payload.ticker,
            trade_date: payload.trade_date,
            rating: payload.rating,
            action: payload.action.unwrap_or_default(),
            market: payload.market.unwrap_or_default(),
            stock_name: payload.stock_name.unwrap_or_default(),
            direction_score: payload.direction_score,
            confidence_score: payload.confidence_score,
            action_score: payload.action_score,
            summary: summary.clone(),
            risk_assessment: payload.risk_assessment.clone().unwrap_or_default(),
            rationale: payload.rationale.unwrap_or_default(),
            structured_risk: payload.structured_risk.unwrap_or_else(|| {
                StructuredRiskAssessment::from_text(
                    payload.risk_assessment.as_deref().unwrap_or_default(),
                )
            }),
            structured_reflection: payload.structured_reflection.unwrap_or_else(|| {
                StructuredReflection::from_text(payload.reflection.as_deref().unwrap_or_default())
            }),
            trigger_checklist: payload.trigger_checklist.unwrap_or_default(),
            blocking_gaps: payload.blocking_gaps.unwrap_or_default(),
            setup_tags: payload.setup_tags.unwrap_or_default(),
            execution_boundary_complete: payload.execution_boundary_complete,
            final_trade_decision: payload
                .final_trade_decision
                .or_else(|| (!summary.trim().is_empty()).then_some(summary))
                .unwrap_or_default(),
            reflection: payload.reflection,
            raw_return: payload.raw_return,
            alpha_return: payload.alpha_return,
            holding_days: payload.holding_days,
            pending: payload.pending.unwrap_or(false),
            user_id: payload.user_id.unwrap_or_default(),
        }
    }

    #[tracing::instrument(skip_all, fields(ticker = %qa.ticker, qa_type = %qa.qa_type))]
    pub async fn qdrant_upsert_qa(&self, qa: &super::QaMemoryEntry) -> anyhow::Result<()> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(());
        };
        if !self.embedding.retrieval_enabled {
            return Ok(());
        }
        let text = format!(
            "question {} answer {} {} ticker {} market {}",
            qa.question_text, qa.answer_summary, qa.answer_conclusion, qa.ticker, qa.market
        );
        let point_id = format!(
            "qa:{}:{}",
            qa.task_id,
            Self::qdrant_point_id(&qa.question_text)
        );
        let payload = serde_json::json!({
            "entry_kind": "qa",
            "qa_type": qa.qa_type,
            "question_type": qa.question_type,
            "question_text": qa.question_text,
            "answer_summary": qa.answer_summary,
            "answer_conclusion": qa.answer_conclusion,
            "ticker": qa.ticker,
            "ticker_lc": qa.ticker.to_ascii_lowercase(),
            "market": qa.market,
            "market_lc": qa.market.to_ascii_lowercase(),
            "username": qa.username,
            "task_id": qa.task_id,
            "subscription_id": qa.subscription_id,
            "evidence_points": qa.evidence_points,
            "risks": qa.risks,
            "actions": qa.actions,
            "created_at": qa.created_at,
            "user_id": qa.username,
            "text": text
        });
        store
            .insert(
                MEMORY_VECTOR_COLLECTION,
                &Self::qdrant_point_id(&point_id),
                &self.embed_text(&text),
                payload,
            )
            .await
    }

    #[tracing::instrument(skip_all, fields(ticker = ticker.unwrap_or("*"), limit = limit))]
    pub async fn search_similar_qa(
        &self,
        question_embedding: &[f32],
        ticker: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<super::QaMemoryEntry>> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(Vec::new());
        };
        if !self.embedding.retrieval_enabled || limit == 0 {
            return Ok(Vec::new());
        }
        let fetch_limit = (limit * 4).max(limit + 20);
        let hits = store
            .search(MEMORY_VECTOR_COLLECTION, question_embedding, fetch_limit)
            .await?;
        let mut results = Vec::new();
        for hit in hits {
            let payload_value = &hit.payload;
            let entry_kind = payload_value
                .get("entry_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if entry_kind != "qa" {
                continue;
            }
            if let Some(ticker) = ticker {
                let hit_ticker = payload_value
                    .get("ticker_lc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !hit_ticker.eq_ignore_ascii_case(ticker) {
                    continue;
                }
            }
            let payload: QdrantMemoryPayload = match serde_json::from_value(hit.payload) {
                Ok(p) => p,
                Err(_) => continue,
            };
            results.push(super::QaMemoryEntry {
                qa_type: payload.action.unwrap_or_default(),
                question_type: String::new(),
                question_text: String::new(),
                answer_summary: payload.summary.unwrap_or_default(),
                answer_conclusion: String::new(),
                ticker: payload.ticker,
                market: payload.market.unwrap_or_default(),
                username: payload.user_id.unwrap_or_default(),
                task_id: String::new(),
                subscription_id: String::new(),
                evidence_points: Vec::new(),
                risks: Vec::new(),
                actions: Vec::new(),
                created_at: payload.trade_date,
            });
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn upsert_user_profile(
        &self,
        username: &str,
        prefs: &sa_models::UserPreferences,
    ) -> anyhow::Result<()> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(());
        };
        if !self.embedding.retrieval_enabled {
            return Ok(());
        }
        let watchlist_symbols: Vec<String> =
            prefs.watchlist.iter().map(|w| w.symbol.clone()).collect();
        let text = format!(
            "user profile risk {} horizon {} markets {} watchlist {} guidance {}",
            prefs.risk_preference,
            prefs.investment_horizon,
            prefs.preferred_markets.join(" "),
            watchlist_symbols.join(" "),
            prefs.guidance_profile
        );
        let point_id = format!("user_profile:{}", username);
        let payload = serde_json::json!({
            "entry_kind": "user_profile",
            "username": username,
            "user_id": username,
            "preferred_markets": prefs.preferred_markets,
            "risk_preference": prefs.risk_preference,
            "investment_horizon": prefs.investment_horizon,
            "watchlist_symbols": watchlist_symbols,
            "guidance_profile": prefs.guidance_profile,
            "text": text,
            "ticker": "",
            "ticker_lc": "",
            "market": "",
            "market_lc": "",
            "trade_date": chrono::Utc::now().to_rfc3339(),
            "rating": "",
            "pending": false
        });
        store
            .insert(
                MEMORY_VECTOR_COLLECTION,
                &Self::qdrant_point_id(&point_id),
                &self.embed_text(&text),
                payload,
            )
            .await
    }

    pub async fn search_personalized(
        &self,
        query_embedding: &[f32],
        username: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let Some(store) = self.vector_store.as_deref() else {
            return Ok(Vec::new());
        };
        if !self.embedding.retrieval_enabled || limit == 0 {
            return Ok(Vec::new());
        }
        let fetch_limit = (limit * 4).max(limit + 20);
        let hits = store
            .search(MEMORY_VECTOR_COLLECTION, query_embedding, fetch_limit)
            .await?;
        let mut results = Vec::new();
        for hit in hits {
            let payload_value = &hit.payload;
            let pending = payload_value
                .get("pending")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if pending {
                continue;
            }
            let user_id = payload_value
                .get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if user_id != username.trim() {
                continue;
            }
            let payload: QdrantMemoryPayload = match serde_json::from_value(hit.payload) {
                Ok(p) => p,
                Err(_) => continue,
            };
            results.push(Self::payload_to_entry(payload));
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }
}
