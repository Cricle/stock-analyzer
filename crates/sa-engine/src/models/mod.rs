//! Analysis result models and storage trait interfaces.

pub mod analysis;
pub mod config;
pub mod market;
pub mod scoring;
pub mod store;
pub mod task;
pub mod user_preferences;
pub mod value_utils;

// Re-export scoring types at crate root for convenience.
pub use scoring::{
    ActionAssessment, CalibrationProfile, ConfidenceAssessment, DirectionAssessment,
    RecommendationCalibration, calibrate_recommendation_with_profile, evaluate_action_score,
    evaluate_confidence_score, evaluate_direction_score, has_execution_boundary,
    history_requires_caution, score_setup_direction_alignment,
};

pub use analysis::{
    ActionBreakdown, ActionScenarioPath, CatalystScoreCard, CatalystScoreItem, Rating,
    ReviewChecklist, ReviewItem, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts,
    AnalysisCheckpoint, AnalysisGraph, AnalysisOutcomeRequest, AnalysisParameters, AnalysisResult,
    AnalysisTaskSummary, AnalysisUserContext,
    AnalysisReuseCandidate, AnalysisReuseCheckRequest, AnalysisReuseSemanticMatch,
    AnalysisScenarioContext, AnalysisScenarioData, AnalysisScenarioIssue, AnalysisScenarioMarket,
    AnalystRuntimeState, AudienceActionGuide,
    CalibrationBias, CalibrationSummary, ConfidenceBreakdown, ConfidenceCap, ConfidenceProfile,
    CoreResearchCall, DebateTurn, DecisionAction, DecisionActionBias, DecisionConfidenceBand,
    DecisionExecutionState, DecisionMode, DecisionTargetType, DecisionTimeframe, DecisionView,
    DecisionViewDirection, DiagnosisIssue, DiagnosisSummary, DirectionBreakdown,
    ExecutionReadiness, HistoricalMemoryHighlight, LocalText, IcDisciplineView, IcNavigatorView,
    InvestmentDebateState, LlmTokenUsageByModel, LlmTokenUsageSummary, MemoryContextSnapshot,
    MissingEvidenceLadder, NewsInsight, PendingToolCall, PriceContext, ProbabilityDriver,
    ProbabilityView, ProfitRiskView, ReferenceFactItem, ReportActionGuides, ReportCandle,
    ReportDiagnosticItem, ReportDiagnostics, ReportEvidenceCard, ReportFlavor, ReportMarketChart,
    ReportReferenceSnapshot, ReportSection, ReportStageState, ResumeAnalysisRequest, RiskControl,
    RiskDebateState, RuntimeNodeTrace, ScoreDimension, SetupMatchExplanation,
    SignedScoreDimension, SingleAnalysisRequest, StockPickDataQualitySnapshot,
    StockPickEvidenceCoverageSummary, StockPickFactorBreakdown, StockPickFailureInfo,
    StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot, StockPickItem,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickObjectiveAssessment,
    StockPickObjectiveBreakdown, StockPickObjectiveBucket, StockPickObjectiveOverview,
    StockPickRequest, StockPickResponse, StockPickRiskSnapshot, StockPickRunRecord,
    StockPickSelectionDiagnostics, StockPickStorageWriteSummary, StockPickTechnicalSnapshot,
    StructuredPortfolioDecision, StructuredReflection, StructuredReport, StructuredResearchPlan,
    TrendLine, TrendLinePoint,
    StructuredRiskAssessment, StructuredTraderPlan, TechnicalIndicatorCategory,
    TechnicalIndicatorConclusion, TechnicalIndicatorItem, TechnicalIndicatorView, ThesisState,
    adx_report, atr_report, bollinger_report, ema_report, kdj_report, macd_report, obv_report,
    rsi_report, sma_report,
    ToolObservation, TradeSetupQuality, derive_report_diagnostics, derive_setup_tags,
    render_action_guides_markdown, render_calibration_discipline_markdown,
};

pub use market::{
    AnnouncementDetailResponse, BillboardEntryItem, BillboardResponse, BillboardSeatItem,
    BillboardSeatsResponse, CandleItem, CandlesResponse, CapitalFlowItem, CapitalFlowResponse,
    FundamentalsResponse, NewsItemResponse, NewsResponse, QuoteResponse, SectorCapitalFlowResponse,
    SectorConstituentItem, SectorConstituentsResponse, SectorRankingItem, SectorRankingsResponse,
    StockSearchItem, StockSearchResponse,
};

pub use store::{
    AnalysisStore, CacheStore, CheckpointStore, GuidanceStore, VectorStore,
    CacheEntry, CheckpointInfo, GuidanceRule, StoredAnalysisSummary, StoredCheckpoint,
    VectorSearchHit,
};

pub use task::{
    AnalysisStep, PersistedTask, ResultStage, StepStatus, TaskEvent, TaskStatus,
    TaskStatusResponse,
};

pub use config::LlmProviderConfig;
pub use user_preferences::{UserPreferences, WatchlistItem};
