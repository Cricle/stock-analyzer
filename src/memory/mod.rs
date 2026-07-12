//! Trading memory system — RAG-based historical analysis retrieval.
//!
//! Stores past analysis results, reflections, and market highlights
//! for semantic search via vector embeddings. Supports both local
//! (fastembed) and remote embedding backends.
//!
//! # Key Types
//!
//! - [`TradingMemoryLog`] — Main memory store with vector search
//! - [`MemoryEntry`] — Individual memory record
//! - [`RagConfig`] — Embedding backend configuration

use std::path::PathBuf;

#[cfg(feature = "local-rag-embeddings")]
use fastembed::TextEmbedding;
use serde::{Deserialize, Serialize};
#[cfg(feature = "local-rag-embeddings")]
use std::sync::Arc;

use crate::HistoricalMemoryHighlight;
use crate::{StructuredReflection, StructuredRiskAssessment};

pub(crate) const ENTRY_SEPARATOR: &str = "\n\n<!-- ENTRY_END -->\n\n";
pub(crate) const WEAK_SETUP_TAGS: &[&str] = &["watchlist_only"];

/// Vector store backend for memory operations.
/// Uses the trait-based VectorStore from sa_models.
pub type VectorMemoryBackend = std::sync::Arc<dyn crate::VectorStore>;

#[derive(Clone)]
pub struct RagConfig {
    pub enabled: bool,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub top_k: usize,
    pub same_ticker_top_k: usize,
    pub cross_ticker_top_k: usize,
}

#[derive(Clone)]
pub struct EmbeddingBackend {
    #[cfg(feature = "local-rag-embeddings")]
    pub inner: Option<Arc<TextEmbedding>>,
    pub provider: String,
    pub model: String,
    pub dimension: usize,
    pub retrieval_enabled: bool,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RagRuntimeSnapshot {
    pub enabled: bool,
    pub embedding_provider: String,
    pub embedding_model: String,
}

#[derive(Clone)]
pub struct TradingMemoryLog {
    pub log_path: PathBuf,
    pub max_entries: usize,
    pub vector_store: Option<VectorMemoryBackend>,
    pub rag: RagConfig,
    pub embedding: EmbeddingBackend,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub ticker: String,
    pub trade_date: String,
    pub rating: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub market: String,
    #[serde(default)]
    pub stock_name: String,
    #[serde(default)]
    pub direction_score: Option<i32>,
    #[serde(default)]
    pub confidence_score: Option<i32>,
    #[serde(default)]
    pub action_score: Option<i32>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub risk_assessment: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub structured_risk: StructuredRiskAssessment,
    #[serde(default)]
    pub structured_reflection: StructuredReflection,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    #[serde(default)]
    pub setup_tags: Vec<String>,
    #[serde(default)]
    pub execution_boundary_complete: Option<bool>,
    pub final_trade_decision: String,
    pub reflection: Option<String>,
    pub raw_return: Option<f64>,
    pub alpha_return: Option<f64>,
    pub holding_days: Option<usize>,
    #[serde(default)]
    pub user_id: String,
    pub pending: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ResearchMemoryRecord {
    pub stock_name: String,
    pub summary: String,
    pub risk_assessment: String,
    pub rationale: String,
    pub structured_risk: StructuredRiskAssessment,
    pub structured_reflection: StructuredReflection,
    pub trigger_checklist: Vec<String>,
    pub blocking_gaps: Vec<String>,
    pub setup_tags: Vec<String>,
    pub execution_boundary_complete: bool,
    pub structured_snapshot: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct MemoryContextBundle {
    pub context_text: String,
    pub source: String,
    pub retrieval_mode: String,
    pub embedding_provider: String,
    pub embedding_failure_reason: Option<String>,
    pub same_ticker_count: usize,
    pub cross_ticker_count: usize,
    pub vector_hit_count: usize,
    pub effective_top_k: usize,
    pub same_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub cross_ticker_highlights: Vec<HistoricalMemoryHighlight>,
}

impl Default for MemoryContextBundle {
    fn default() -> Self {
        Self {
            context_text: String::new(),
            source: String::new(),
            retrieval_mode: "disabled".to_string(),
            embedding_provider: "disabled".to_string(),
            embedding_failure_reason: None,
            same_ticker_count: 0,
            cross_ticker_count: 0,
            vector_hit_count: 0,
            effective_top_k: 0,
            same_ticker_highlights: Vec::new(),
            cross_ticker_highlights: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MemoryContextBundleWithTags {
    pub context_text: String,
    pub source: String,
    pub retrieval_mode: String,
    pub embedding_provider: String,
    pub embedding_failure_reason: Option<String>,
    pub same_ticker_count: usize,
    pub cross_ticker_count: usize,
    pub vector_hit_count: usize,
    pub effective_top_k: usize,
    pub same_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub cross_ticker_highlights: Vec<HistoricalMemoryHighlight>,
    pub setup_tags: Vec<String>,
    pub used_setup_filtered_retrieval: bool,
    pub used_setup_fallback_calibration: bool,
    pub setup_calibration_sample_count: usize,
    pub setup_match_count: usize,
    pub setup_pending_match_count: usize,
    pub setup_resolved_match_count: usize,
    pub setup_match_hit_rate: f64,
    pub setup_match_avg_alpha_return: f64,
    pub setup_long_match_count: usize,
    pub setup_short_match_count: usize,
    pub setup_neutral_match_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryQuery {
    pub ticker: String,
    pub market: String,
    pub setup_tags: Vec<String>,
    pub user_id: String,
}

mod core;
pub mod cross_collection;
mod embedding;
pub mod format;
pub mod stats;
mod vector_store;

/// Format memory context parts from same-ticker and cross-ticker entries.
pub fn format_memory_parts(
    same_entries: &[MemoryEntry],
    cross_entries: &[MemoryEntry],
    format_full: impl Fn(&MemoryEntry) -> String,
    format_reflection: impl Fn(&MemoryEntry) -> String,
) -> Vec<String> {
    let mut parts = Vec::new();
    if !same_entries.is_empty() {
        parts.push(format!(
            "Past analyses of {} (most relevant first):",
            same_entries[0].ticker.trim().to_uppercase()
        ));
        parts.extend(same_entries.iter().map(format_full));
    }
    if !cross_entries.is_empty() {
        parts.push("Recent cross-ticker lessons:".to_string());
        parts.extend(cross_entries.iter().map(format_reflection));
    }
    parts
}

/// Build highlights from memory entries (top 3 from each group).
pub fn build_highlights(
    same_entries: &[MemoryEntry],
    cross_entries: &[MemoryEntry],
    highlight_from_entry: impl Fn(&MemoryEntry, bool) -> HistoricalMemoryHighlight,
) -> (
    Vec<HistoricalMemoryHighlight>,
    Vec<HistoricalMemoryHighlight>,
) {
    let same_highlights = same_entries
        .iter()
        .take(3)
        .map(|entry| highlight_from_entry(entry, true))
        .collect();
    let cross_highlights = cross_entries
        .iter()
        .take(3)
        .map(|entry| highlight_from_entry(entry, false))
        .collect();
    (same_highlights, cross_highlights)
}

/// Generate a hash-based embedding vector from text.
/// Uses SHA-256 to deterministically map tokens to vector indices.
pub fn hash_embed_text(text: &str, dimension: usize) -> Vec<f32> {
    use sha2::{Digest, Sha256};
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

pub use stats::*;
