//! Stock pick module — candidate resolution, scoring, and LLM-based selection.

mod types;
pub(crate) use types::*;

mod history;
pub(crate) use history::{StockPickEvidencePayload, StockPickHistoryStore};

mod pipeline;
pub use pipeline::run;

mod scoring;
pub(crate) use scoring::{
    apply_portfolio_constraints, enrich_candidates, infer_theme_key, score_candidates,
};

pub(crate) mod news_cache;

mod objective;

mod llm_utils;
pub use llm_utils::llm_client_for_request;
