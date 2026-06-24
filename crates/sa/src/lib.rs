#![allow(
    clippy::collapsible_if,
    clippy::let_and_return,
    clippy::type_complexity,
    clippy::redundant_closure,
    clippy::needless_question_mark,
    clippy::unnecessary_lazy_evaluations,
    clippy::manual_contains,
    clippy::unnecessary_map_or,
    clippy::manual_clamp,
    clippy::too_many_arguments,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion,
    clippy::manual_is_ascii_check,
    clippy::derivable_impls,
    clippy::redundant_field_names,
    clippy::bool_comparison,
    clippy::needless_borrow,
    clippy::if_same_then_else,
    clippy::manual_range_contains,
    clippy::should_implement_trait,
    clippy::redundant_pattern_matching
)]

//! sa — Unified stock analysis crate.
//!
//! # Features
//!
//! - `report` — Full analysis report generation
//! - `guide` — Daily market guidance
//! - `pick` — Stock picking and screening
//! - `score` — Stock scoring system

// ── Base modules ──

pub mod env_config;
pub mod llm_config;
pub mod shared;
pub mod task_manager;
pub mod telemetry;

pub mod checkpoint;
pub mod llm;
pub mod memory;
pub mod tools;

// ── Data module ──

pub mod data;

// ── Type re-exports (from akshare) ──

pub mod types {
    pub use akshare::types::*;
    pub use akshare::provider::market_client::{GeneralSearchIntent, MarketDataClient};
    pub use akshare::provider::market_client::DataFetchDiagnosis;
    pub use akshare::provider::market_client::normalized_news_date;
    pub use akshare::stock::feature::*;
}

// ── Models ──

pub mod analysis;
pub mod store;
pub mod task;
pub mod user_preferences;
pub mod value_utils;

pub mod scoring;

// ── Feature modules ──

pub mod report;
pub mod guide;
pub mod pick;

// ── Re-exports ──

pub use analysis::{
    ActionBreakdown, ActionScenarioPath, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts,
    AnalysisCheckpoint, AnalysisGraph, AnalysisOutcomeRequest, AnalysisParameters, AnalysisResult,
    AnalysisReuseCandidate, AnalysisReuseCheckRequest, AnalysisReuseSemanticMatch,
    AnalysisScenarioContext, AnalysisScenarioData, AnalysisScenarioIssue, AnalysisScenarioMarket,
    AnalysisTaskSummary, AnalysisUserContext, AnalystRuntimeState, AudienceActionGuide,
    CalibrationBias, CalibrationSummary, CatalystScoreCard, CatalystScoreItem, ConfidenceBreakdown,
    ConfidenceCap, ConfidenceProfile, CoreResearchCall, DebateTurn, DecisionAction,
    DecisionActionBias, DecisionConfidenceBand, DecisionExecutionState, DecisionMode,
    DecisionTargetType, DecisionTimeframe, DecisionView, DecisionViewDirection, DiagnosisIssue,
    DiagnosisSummary, DirectionBreakdown, ExecutionReadiness, HistoricalMemoryHighlight,
    IcDisciplineView, IcNavigatorView, InvestmentDebateState, LlmTokenUsageByModel,
    LlmTokenUsageSummary, LocalText, MemoryContextSnapshot, MissingEvidenceLadder, NewsInsight,
    PendingToolCall, PriceContext, ProbabilityDriver, ProbabilityView, ProfitRiskView, Rating,
    ReferenceFactItem, ReportActionGuides, ReportCandle, ReportDiagnosticItem, ReportDiagnostics,
    ReportEvidenceCard, ReportFlavor, ReportMarketChart, ReportReferenceSnapshot, ReportSection,
    ReportStageState, ResumeAnalysisRequest, ReviewChecklist, ReviewItem, RiskControl,
    RiskDebateState, RuntimeNodeTrace, ScoreDimension, SetupMatchExplanation, SignedScoreDimension,
    SingleAnalysisRequest, StockPickDataQualitySnapshot, StockPickEvidenceCoverageSummary,
    StockPickFactorBreakdown, StockPickFailureInfo, StockPickFundamentalSnapshot,
    StockPickHistoryMatchSnapshot, StockPickItem, StockPickMarketSnapshot, StockPickNewsSnapshot,
    StockPickObjectiveAssessment, StockPickObjectiveBreakdown, StockPickObjectiveBucket,
    StockPickObjectiveOverview, StockPickRequest, StockPickResponse, StockPickRiskSnapshot,
    StockPickRunRecord, StockPickSelectionDiagnostics, StockPickStorageWriteSummary,
    StockPickTechnicalSnapshot, StructuredPortfolioDecision, StructuredReflection,
    StructuredReport, StructuredResearchPlan, StructuredRiskAssessment, StructuredTraderPlan,
    TechnicalIndicatorCategory, TechnicalIndicatorConclusion, TechnicalIndicatorItem,
    TechnicalIndicatorView, ThesisState, ToolObservation, TradeSetupQuality, TrendLine,
    TrendLinePoint, adx_report, atr_report, bollinger_report, derive_report_diagnostics,
    derive_setup_tags, ema_report, kdj_report, macd_report, obv_report,
    render_action_guides_markdown, render_calibration_discipline_markdown, rsi_report, sma_report,
};

pub use store::{
    AnalysisStore, CacheEntry, CacheStore, CheckpointInfo, CheckpointStore, GuidanceRule,
    GuidanceStore, InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore,
    InMemoryGuidanceStore, StoredAnalysisSummary, StoredCheckpoint, VectorSearchHit, VectorStore,
};

pub use task::{
    AnalysisStep, PersistedTask, ResultStage, StepStatus, TaskEvent, TaskStatus, TaskStatusResponse,
};

pub use llm_config::LlmProviderConfig;
pub use user_preferences::{UserPreferences, WatchlistItem};

pub use scoring::{
    ActionAssessment, CalibrationProfile, ConfidenceAssessment, DirectionAssessment,
    RecommendationCalibration, calibrate_recommendation_with_profile, evaluate_action_score,
    evaluate_confidence_score, evaluate_direction_score, has_execution_boundary,
    history_requires_caution, score_setup_direction_alignment,
};

pub use task_manager::TASK_STEPS;
pub use task_manager::{TaskManager, TaskRunParams};
pub use telemetry::{SharedTelemetry, TelemetryState};
