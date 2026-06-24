//! Analysis result models and storage trait interfaces.

pub(crate) mod analysis;
pub(crate) mod config;
pub(crate) mod market;
pub(crate) mod scoring;
pub(crate) mod store;
pub(crate) mod task;
pub(crate) mod user_preferences;
pub(crate) mod value_utils;

// Re-export scoring types at crate root for convenience.
pub(crate) use scoring::{
    ActionAssessment, CalibrationProfile, ConfidenceAssessment, DirectionAssessment,
    RecommendationCalibration, calibrate_recommendation_with_profile, evaluate_action_score,
    evaluate_confidence_score, evaluate_direction_score, has_execution_boundary,
    history_requires_caution, score_setup_direction_alignment,
};

pub(crate) use analysis::{
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

pub(crate) use market::{
    AnnouncementDetailResponse, BillboardEntryItem, BillboardResponse, BillboardSeatItem,
    BillboardSeatsResponse, CandleItem, CandlesResponse, CapitalFlowItem, CapitalFlowResponse,
    FundamentalsResponse, NewsItemResponse, NewsResponse, QuoteResponse, SectorCapitalFlowResponse,
    SectorConstituentItem, SectorConstituentsResponse, SectorRankingItem, SectorRankingsResponse,
    StockSearchItem, StockSearchResponse,
};

pub(crate) use store::{
    AnalysisStore, CacheEntry, CacheStore, CheckpointInfo, CheckpointStore, GuidanceRule,
    GuidanceStore, StoredAnalysisSummary, StoredCheckpoint, VectorSearchHit, VectorStore,
};

pub(crate) use task::{
    AnalysisStep, PersistedTask, ResultStage, StepStatus, TaskEvent, TaskStatus, TaskStatusResponse,
};

pub(crate) use config::LlmProviderConfig;
pub(crate) use user_preferences::{UserPreferences, WatchlistItem};
