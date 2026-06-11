//! Daily market guidance report system.
//!
//! Aggregates news (via searxng), market data, and historical memory
//! to produce structured daily guidance reports. Reports are cached in Redis
//! and summaries are stored in Qdrant for cross-day semantic retrieval.

pub mod embedding;
mod models;
mod prewarm;
mod report;
pub mod store;

pub use embedding::{hash_embed, semantic_embed};
pub use models::*;
pub use prewarm::{PrewarmTask, generate_prewarm_tasks};
pub use report::DailyGuidanceGenerator;
pub use store::GuidanceStore;
pub use store::PreparedData;

/// Minimal memory interface needed by the guidance system.
/// The backend implements this for `TradingMemoryLog`.
#[async_trait::async_trait]
pub trait GuidanceMemory: Send + Sync {
    async fn past_context_bundle(
        &self,
        query: &str,
        same_ticker_limit: usize,
        cross_ticker_limit: usize,
    ) -> GuidanceMemoryBundle;
}

/// Lightweight memory context used by guidance reports.
#[derive(Clone, Debug, Default)]
pub struct GuidanceMemoryBundle {
    pub context_text: String,
    pub source: String,
    pub vector_hit_count: usize,
    pub same_ticker_count: usize,
    pub cross_ticker_count: usize,
    pub same_ticker_highlights: Vec<GuidanceMemoryHighlight>,
}

/// Simplified memory highlight for guidance reports.
#[derive(Clone, Debug, Default)]
pub struct GuidanceMemoryHighlight {
    pub key_risk: String,
    pub lesson: String,
}
