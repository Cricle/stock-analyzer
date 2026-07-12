//! Stock scoring system — multi-dimensional assessment of stock candidates.
//!
//! Combines technical indicators, fundamental metrics, sentiment signals,
//! and LLM analysis into a unified score with confidence calibration.
//!
//! # Key Types
//!
//! - `StockScore` — Final composite score with breakdown
//! - `ScoreWeights` — Configurable weights per dimension
//! - `DimensionScore` — Individual dimension scoring result

use crate::{
    ActionBreakdown, AgentReportNode, AnalysisResult, ConfidenceBreakdown, ConfidenceCap,
    ConfidenceProfile, DirectionBreakdown, LocalText, Rating, ReportDiagnosticItem, ScoreDimension,
    SignedScoreDimension, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan, TechnicalIndicatorView,
};

include!("types.rs");
include!("assessment.rs");
include!("helpers.rs");

// Merged from score/ module
pub mod config;
pub mod dimensions;
pub mod score_types;
/// Backward-compatibility alias: `scoring::types` → `scoring::score_types`.
pub use score_types as types;
pub mod scorer;

pub use score_types::{DimensionScore, ScoreReliability, ScoreWeights, StockScore, score_label};
pub use scorer::score_stock_pick;
