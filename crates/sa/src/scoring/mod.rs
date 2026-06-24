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
pub mod scorer;

pub use scorer::score_stock_pick;
pub use score_types::{DimensionScore, ScoreWeights, StockScore, score_label};
