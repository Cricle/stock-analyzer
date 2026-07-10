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
//! Data fetching is delegated to [akshare-rs](https://github.com/Cricle/akshare-rs); this crate focuses on
//! analysis, scoring, LLM-powered reports, and stock picking.
//!
//! # Features
//!
//! - `report` — Full analysis report generation
//! - `guide` — Daily market guidance
//! - `pick` — Stock picking and screening
//! - `score` — Stock scoring system
//! - `local-rag-embeddings` — Local vector embeddings via `fastembed`
//!
//! # Key Types
//!
//! ```rust
//! use sa::{Rating, LocalText};
//! use sa::store::InMemoryAnalysisStore;
//!
//! // Rating enum for buy/hold/sell recommendations
//! let rating = Rating::parse("buy");
//! assert!(rating.is_bullish());
//! assert!(!rating.is_bearish());
//!
//! // LocalText for i18n key storage
//! let text = LocalText::new("report.buy_signal");
//! assert_eq!(text.as_str(), "report.buy_signal");
//! assert!(!text.is_empty());
//!
//! // In-memory store for testing
//! let _store = InMemoryAnalysisStore::new();
//! ```

// ── Base modules ──

pub mod config;
pub mod env_config;
pub mod llm_config;
pub mod shared;
pub mod task_manager;
pub mod telemetry;

pub mod checkpoint;
pub mod llm;
pub mod memory;

// ── Data module ──

pub mod data;

// ── Type re-exports (from akshare) ──

pub mod types {
    pub use akshare::provider::market_client::DataFetchDiagnosis;
    pub use akshare::provider::market_client::normalized_news_date;
    pub use akshare::provider::market_client::tools::{
        PendingToolCall, ScenarioData, ToolObservation, TradingToolbox,
    };
    pub use akshare::provider::market_client::{GeneralSearchIntent, MarketDataClient};
    pub use akshare::stock::feature::*;
    pub use akshare::types::*;
}

// ── Storage (stub for PgStore) ──

pub mod storage;

// ── Models ──

pub mod analysis;
pub mod indicators;
mod noop_stores;
pub mod store;
pub mod task;
pub mod value_utils;

pub mod scoring;
/// Backward-compatibility alias: `sa::score` → `sa::scoring`.
pub use scoring as score;

// ── Feature modules ──

pub mod guide;
pub mod pick;
pub mod report;

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
    PriceContext, ProbabilityDriver, ProbabilityView, ProfitRiskView, Rating, ReferenceFactItem,
    ReportActionGuides, ReportCandle, ReportDiagnosticItem, ReportDiagnostics, ReportEvidenceCard,
    ReportFlavor, ReportMarketChart, ReportReferenceSnapshot, ReportSection, ReportStageState,
    ResumeAnalysisRequest, ReviewChecklist, ReviewItem, RiskControl, RiskDebateState,
    RuntimeNodeTrace, ScoreDimension, SetupMatchExplanation, SignedScoreDimension,
    SingleAnalysisRequest, StockPickDataQualitySnapshot, StockPickEvidenceCoverageSummary,
    StockPickFactorBreakdown, StockPickFailureInfo, StockPickFundamentalSnapshot,
    StockPickHistoryMatchSnapshot, StockPickItem, StockPickMarketSnapshot, StockPickNewsSnapshot,
    StockPickObjectiveAssessment, StockPickObjectiveBreakdown, StockPickObjectiveBucket,
    StockPickObjectiveOverview, StockPickRequest, StockPickResponse, StockPickRiskSnapshot,
    StockPickRunRecord, StockPickSelectionDiagnostics, StockPickStorageWriteSummary,
    StockPickTechnicalSnapshot, StructuredPortfolioDecision, StructuredReflection,
    StructuredReport, StructuredResearchPlan, StructuredRiskAssessment, StructuredTraderPlan,
    TechnicalIndicatorCategory, TechnicalIndicatorConclusion, TechnicalIndicatorItem,
    TechnicalIndicatorView, TechnicalValues, ThesisState, TradeSetupQuality, TrendLine,
    TrendLinePoint, adx_report, atr_report, bollinger_report, derive_action_guides,
    derive_memory_reference_facts, derive_news_diagnostics, derive_news_insights,
    derive_report_diagnostics, derive_setup_match_explanation, derive_setup_tags,
    derive_technical_conclusions, detect_disclosure_sequence_complexity, ema_report,
    is_publishable_summary_reference, is_semantically_similar, kdj_report, macd_report, obv_report,
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

pub use config::SaConfig;
pub use env_config::env_flag;
pub use llm_config::LlmProviderConfig;
pub use types::ToolObservation;

pub use scoring::{
    ActionAssessment, CalibrationProfile, ConfidenceAssessment, DirectionAssessment,
    RecommendationCalibration, calibrate_recommendation_with_profile, evaluate_action_score,
    evaluate_confidence_score, evaluate_direction_score, has_execution_boundary,
    history_requires_caution, score_setup_direction_alignment,
};

pub use task_manager::TASK_STEPS;
pub use task_manager::{TaskManager, TaskRunParams};
pub use telemetry::{SharedTelemetry, TelemetryState};

// ── Convenience re-exports from types (used by tests) ──

pub use types::{
    BatchFundamentalsResult, BatchQuoteResult, CandlePoint, DataFetchDiagnosis,
    FundamentalsSnapshot, MarketDataClient, MarketKind, NewsItem, QuoteSnapshot,
    normalized_news_date,
};

// ── Backward-compatibility: `engine` module ──
//
// TAR references `stock_analyzer::engine::*` which maps to the top-level modules.

pub mod engine {
    pub use crate::checkpoint;
    pub use crate::data;
    pub use crate::env_config as config;
    pub use crate::guide as guidance;
    pub use crate::llm;
    pub use crate::memory;
    pub use crate::pick as stock_pick;
    pub use crate::task_manager::{self, TASK_STEPS, TaskManager, TaskRunParams};
    pub use crate::telemetry::{self, SharedTelemetry, TelemetryState};

    /// Stub for qlib_import (not present in this version).
    pub mod qlib_import {
        pub async fn run_init_from_env(_data_dir: &str) -> anyhow::Result<serde_json::Value> {
            anyhow::bail!("qlib_import is not available in this version of stock-analyzer")
        }
    }
}

// ── Backward-compatibility: `models` module ──
//
// TAR references `stock_analyzer::models::*` which maps to the crate root re-exports.

pub mod models {
    pub use crate::*;
}
