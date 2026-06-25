use crate::{
    ActionBreakdown, AgentReportNode, AnalysisResult, ConfidenceBreakdown, ConfidenceCap,
    ConfidenceProfile, DirectionBreakdown, LocalText, Rating, ReportDiagnosticItem, ScoreDimension,
    SignedScoreDimension, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan,
};

include!("types.rs");
include!("assessment.rs");
include!("helpers.rs");

// Merged from score/ module
pub mod config;
pub mod dimensions;
pub mod history;
pub mod score_types;
/// Backward-compatibility alias: `scoring::types` → `scoring::score_types`.
pub use score_types as types;
pub mod scorer;

pub use score_types::{DimensionScore, ScoreWeights, StockScore, score_label};
pub use scorer::score_stock_pick;
