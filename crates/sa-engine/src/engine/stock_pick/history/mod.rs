//! Stock pick history storage using trait-based CacheStore and VectorStore.
//!
//! TODO: This module previously used direct Redis and Qdrant access.
//! The migration uses CacheStore for key-value operations and VectorStore
//! for vector search. Some complex Redis operations (LPUSH, LRANGE, SCAN)
//! are simplified or stubbed since CacheStore doesn't support lists.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{
    StockPickEvidenceCoverageSummary, StockPickHistoryMatchSnapshot,
    StockPickResponse, StockPickStorageWriteSummary,
};

const STOCK_PICK_CACHE_PREFIX: &str = "tradingagents:stock_pick";
const STOCK_PICK_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const STOCK_PICK_VECTOR_COLLECTION: &str = "tradingagents_stock_pick";

#[derive(Clone)]
pub(crate) struct StockPickHistoryStore {
    cache: std::sync::Arc<dyn crate::models::CacheStore>,
    vector_store: std::sync::Arc<dyn crate::models::VectorStore>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StockPickEvidencePayload {
    pub symbol: String,
    pub market: String,
    pub theme_key: String,
    pub analysis_date: String,
    pub query: String,
    pub published_at: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub evidence_type: String,
    pub sentiment_hint: String,
    pub hard_negative_flag: bool,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StockPickHistoryPayload {
    pub run_id: String,
    pub symbol: String,
    pub market: String,
    pub analysis_date: String,
    pub strategy: String,
    pub theme_key: String,
    pub industry: String,
    pub score: f64,
    pub confidence: f64,
    pub final_rank: i32,
    pub selected: bool,
    pub grade: String,
    pub ready: bool,
    pub summary: String,
    pub evidence_points: Vec<String>,
    pub risk_flags: Vec<String>,
    pub alpha_return: Option<f64>,
    pub pick_price: Option<f64>,
    pub pick_date: Option<String>,
}

impl StockPickHistoryStore {
    #[allow(dead_code)]
    pub(crate) fn new(
        cache: std::sync::Arc<dyn crate::models::CacheStore>,
        vector_store: std::sync::Arc<dyn crate::models::VectorStore>,
    ) -> Self {
        Self { cache, vector_store }
    }

    /// Create from environment variables (legacy compatibility).
    ///
    /// TODO: Replace with explicit dependency injection.
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        tracing::warn!("StockPickHistoryStore::from_env() called without injected stores; using no-op fallback");
        Ok(Self {
            cache: std::sync::Arc::new(NoopCacheStore),
            vector_store: std::sync::Arc::new(NoopVectorStore),
        })
    }

    pub(crate) async fn write_run(
        &self,
        run_id: &str,
        request_market: &str,
        response: &StockPickResponse,
        theme_keys: &[(String, String)],
        evidence_payloads: &[StockPickEvidencePayload],
    ) -> anyhow::Result<StockPickStorageWriteSummary> {
        let mut cache_keys_written = 0usize;
        let mut vector_points_written = 0usize;

        // Write run data to cache
        let run_key = format!("{STOCK_PICK_CACHE_PREFIX}:run:{run_id}");
        let run_payload = serde_json::to_vec(response)?;
        self.cache.set(&run_key, &run_payload, Some(STOCK_PICK_CACHE_TTL_SECS)).await?;
        cache_keys_written += 1;

        // Write summary to cache
        let summary_key = format!(
            "{STOCK_PICK_CACHE_PREFIX}:summary:{}:{}:{}",
            request_market.trim().to_ascii_lowercase(),
            response.analysis_date.trim(),
            response.strategy.trim().to_ascii_lowercase().replace(' ', "_")
        );
        let summary_payload = serde_json::to_vec(&response.summary)?;
        self.cache.set(&summary_key, &summary_payload, Some(STOCK_PICK_CACHE_TTL_SECS)).await?;
        cache_keys_written += 1;

        // Write picks to vector store
        for pick in &response.picks {
            let theme_key = theme_keys
                .iter()
                .find(|(symbol, _)| symbol.eq_ignore_ascii_case(&pick.symbol))
                .map(|(_, theme)| theme.clone())
                .unwrap_or_default();
            self.upsert_history_payload(StockPickHistoryPayload {
                run_id: run_id.to_string(),
                symbol: pick.symbol.clone(),
                market: pick.market.clone(),
                analysis_date: response.analysis_date.clone(),
                strategy: response.strategy.clone(),
                theme_key,
                industry: pick.fundamental_snapshot.industry.clone(),
                score: pick.score,
                confidence: pick.confidence,
                final_rank: pick.priority_rank,
                selected: true,
                grade: pick.objective_assessment.grade.clone(),
                ready: pick.objective_assessment.ready,
                summary: pick.thesis.clone(),
                evidence_points: pick.evidence_points.clone(),
                risk_flags: pick.rejection_risk_flags.clone(),
                alpha_return: None,
                pick_price: pick.price.or(pick.market_snapshot.current_price),
                pick_date: Some(response.analysis_date.clone()),
            })
            .await?;
            vector_points_written += 1;
        }

        for payload in evidence_payloads {
            self.upsert_evidence_payload(payload).await?;
            vector_points_written += 1;
        }

        Ok(StockPickStorageWriteSummary {
            redis_keys_written: cache_keys_written,
            qdrant_points_written: vector_points_written,
        })
    }

    pub(crate) async fn read_history(
        &self,
        symbol: &str,
        market: &str,
        theme_key: &str,
        current_price: Option<f64>,
    ) -> anyhow::Result<StockPickHistoryMatchSnapshot> {
        let query_text = format!(
            "symbol {} market {} theme {} stock pick history evidence",
            symbol.trim().to_uppercase(),
            market.trim(),
            theme_key.trim()
        );
        let embedding = hash_embed_text(&query_text, 384);
        let hits = self
            .vector_store
            .search(STOCK_PICK_VECTOR_COLLECTION, &embedding, 6)
            .await?;

        if hits.is_empty() {
            return Ok(StockPickHistoryMatchSnapshot {
                enabled: true,
                sample_count: 0,
                vector_hit_count: 0,
                average_score: None,
                hit_rate: None,
                average_alpha_return: None,
                top_matches: Vec::new(),
            });
        }

        let mut sample_count = 0usize;
        let mut score_sum = 0.0;
        let mut alpha_sum = 0.0;
        let mut alpha_count = 0usize;
        let mut hit_count = 0usize;
        let mut top_matches = Vec::new();
        for hit in &hits {
            let payload = &hit.payload;
            sample_count += 1;
            score_sum += payload.get("score").and_then(|v| v.as_f64()).unwrap_or_default();

            let alpha = payload
                .get("alpha_return")
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    let stored_price = payload.get("pick_price").and_then(|v| v.as_f64())?;
                    let cur = current_price?;
                    if stored_price > 0.0 {
                        Some(cur / stored_price - 1.0)
                    } else {
                        None
                    }
                });

            if let Some(alpha) = alpha {
                alpha_sum += alpha;
                alpha_count += 1;
                if alpha > 0.0 {
                    hit_count += 1;
                }
            }
            if let Some(match_symbol) = payload.get("symbol").and_then(|v| v.as_str()) {
                top_matches.push(match_symbol.to_string());
            }
        }

        Ok(StockPickHistoryMatchSnapshot {
            enabled: true,
            sample_count,
            vector_hit_count: hits.len(),
            average_score: (sample_count > 0).then_some(score_sum / sample_count as f64),
            hit_rate: (alpha_count > 0).then_some(hit_count as f64 / alpha_count as f64),
            average_alpha_return: (alpha_count > 0).then_some(alpha_sum / alpha_count as f64),
            top_matches,
        })
    }

    pub(crate) fn build_evidence_coverage_summary(
        &self,
        light_search_symbols: usize,
        deep_search_symbols: usize,
        evidence_records_indexed: usize,
        history_records_matched: usize,
    ) -> StockPickEvidenceCoverageSummary {
        StockPickEvidenceCoverageSummary {
            light_search_symbols,
            deep_search_symbols,
            evidence_records_indexed,
            history_records_matched,
        }
    }

    async fn upsert_history_payload(&self, payload: StockPickHistoryPayload) -> anyhow::Result<()> {
        let point_id = qdrant_point_id(&format!(
            "stock-pick-history:{}:{}:{}",
            payload.symbol, payload.analysis_date, payload.run_id
        ));
        let text = format!(
            "{} {} {} {}",
            payload.symbol, payload.theme_key, payload.industry, payload.summary
        );
        let vector = hash_embed_text(&text, 384);
        let json_payload = serde_json::json!({
            "entry_kind": "stock_pick_history",
            "run_id": payload.run_id,
            "symbol": payload.symbol,
            "market": payload.market,
            "analysis_date": payload.analysis_date,
            "strategy": payload.strategy,
            "theme_key": payload.theme_key,
            "industry": payload.industry,
            "score": payload.score,
            "confidence": payload.confidence,
            "final_rank": payload.final_rank,
            "selected": payload.selected,
            "grade": payload.grade,
            "ready": payload.ready,
            "summary": payload.summary,
            "evidence_points": payload.evidence_points,
            "risk_flags": payload.risk_flags,
            "alpha_return": payload.alpha_return,
            "pick_price": payload.pick_price,
            "pick_date": payload.pick_date
        });
        self.vector_store
            .insert(STOCK_PICK_VECTOR_COLLECTION, &point_id, &vector, json_payload)
            .await
    }

    async fn upsert_evidence_payload(
        &self,
        payload: &StockPickEvidencePayload,
    ) -> anyhow::Result<()> {
        let point_id = qdrant_point_id(&format!(
            "stock-pick-evidence:{}:{}:{}",
            payload.symbol, payload.analysis_date, payload.dedupe_key
        ));
        let text = format!(
            "{} {} {} {}",
            payload.symbol, payload.title, payload.summary, payload.evidence_type
        );
        let vector = hash_embed_text(&text, 384);
        let json_payload = serde_json::json!({
            "entry_kind": "stock_pick_evidence",
            "symbol": payload.symbol,
            "market": payload.market,
            "theme_key": payload.theme_key,
            "analysis_date": payload.analysis_date,
            "query": payload.query,
            "published_at": payload.published_at,
            "title": payload.title,
            "summary": payload.summary,
            "source": payload.source,
            "url": payload.url,
            "evidence_type": payload.evidence_type,
            "sentiment_hint": payload.sentiment_hint,
            "hard_negative_flag": payload.hard_negative_flag,
            "dedupe_key": payload.dedupe_key
        });
        self.vector_store
            .insert(STOCK_PICK_VECTOR_COLLECTION, &point_id, &vector, json_payload)
            .await
    }
}

fn qdrant_point_id(entry_id: &str) -> String {
    let digest = Sha256::digest(entry_id.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn hash_embed_text(text: &str, dimension: usize) -> Vec<f32> {
    let mut vector = vec![0.0f32; dimension];
    for token in text
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let normalized = token.to_ascii_lowercase();
        let digest = Sha256::digest(normalized.as_bytes());
        let index = (u16::from_le_bytes([digest[0], digest[1]]) as usize) % dimension.max(1);
        let sign = if digest[2] % 2 == 0 { 1.0 } else { -1.0 };
        let magnitude = 1.0 + (digest[3] as f32 / 255.0);
        vector[index] += sign * magnitude;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

// No-op fallback implementations

struct NoopCacheStore;

#[async_trait::async_trait]
impl crate::models::CacheStore for NoopCacheStore {
    async fn get(&self, _key: &str) -> anyhow::Result<Option<Vec<u8>>> { Ok(None) }
    async fn set(&self, _key: &str, _value: &[u8], _ttl_seconds: Option<u64>) -> anyhow::Result<()> { Ok(()) }
    async fn delete(&self, _key: &str) -> anyhow::Result<()> { Ok(()) }
    async fn exists(&self, _key: &str) -> anyhow::Result<bool> { Ok(false) }
    async fn list_entries(&self, _prefix: &str) -> anyhow::Result<Vec<crate::models::CacheEntry>> { Ok(vec![]) }
}

struct NoopVectorStore;

#[async_trait::async_trait]
impl crate::models::VectorStore for NoopVectorStore {
    async fn insert(&self, _collection: &str, _id: &str, _embedding: &[f32], _payload: serde_json::Value) -> anyhow::Result<()> { Ok(()) }
    async fn search(&self, _collection: &str, _query_embedding: &[f32], _top_k: usize) -> anyhow::Result<Vec<crate::models::VectorSearchHit>> { Ok(vec![]) }
    async fn delete(&self, _collection: &str, _id: &str) -> anyhow::Result<()> { Ok(()) }
}
