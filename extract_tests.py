#!/usr/bin/env python3
"""Extract #[cfg(test)] blocks from source files to tests/ directory."""
import re
import os

BASE = "/root/github/stock-analyzer"
SRC = os.path.join(BASE, "src")
TESTS = os.path.join(BASE, "tests")

def extract_test_block(content):
    """Extract the #[cfg(test)] mod ... { ... } block from source content."""
    pattern = r'#\[cfg\(test\)\]\s*\nmod\s+(\w+)\s*\{'
    match = re.search(pattern, content)
    if not match:
        return None, None, None
    mod_name = match.group(1)
    start = match.start()
    brace_count = 0
    i = match.end() - 1
    while i < len(content):
        if content[i] == '{':
            brace_count += 1
        elif content[i] == '}':
            brace_count -= 1
            if brace_count == 0:
                end = i + 1
                break
        i += 1
    test_block = content[start:end]
    inner_start = test_block.index('{') + 1
    inner_content = test_block[inner_start:-1].strip()
    return test_block, inner_content, mod_name

# --- FILES CONFIG ---
# Each entry: (src_path, test_file_name, imports, private_fns_to_make_pub)
# private_fns_to_make_pub: list of function names to change from fn to pub fn

FILES = [
    # report_types
    ("src/analysis/report_types/action_guides.rs", "tests/analysis_report_types_action_guides.rs",
     """use sa::analysis::{
    DirectionBreakdown, SignedScoreDimension, ActionBreakdown, ReportSection,
    ReportActionGuides, AudienceActionGuide, ActionScenarioPath,
    LocalText, ScoreDimension,
};
""",
     []),
    ("src/analysis/report_types/chart.rs", "tests/analysis_report_types_chart.rs",
     """use sa::analysis::{
    ReportMarketChart, ReportCandle, ChartOverlay, TrendLine, TrendLinePoint,
};
""",
     []),
    ("src/analysis/report_types/confidence.rs", "tests/analysis_report_types_confidence.rs",
     """use sa::analysis::{
    CalibrationBias, ReportDiagnostics, ReportDiagnosticItem, ReportReferenceSnapshot,
    ReferenceFactItem, HistoricalCalibrationStats, ConfidenceBreakdown, ScoreDimension,
    ConfidenceCap, ResearchReliability, LocalText,
};
""",
     []),
    ("src/analysis/report_types/context.rs", "tests/analysis_report_types_context.rs",
     """use sa::analysis::{
    AnalysisUserContext, LlmTokenUsageSummary, LlmTokenUsageByModel,
    MemoryContextSnapshot, HistoricalMemoryHighlight,
};
""",
     []),
    ("src/analysis/report_types/graph.rs", "tests/analysis_report_types_graph.rs",
     """use sa::analysis::{
    AnalysisGraph, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts,
    DiagnosisSummary, DiagnosisIssue, MemoryContextSnapshot, LlmTokenUsageSummary,
    ReportMarketChart, AnalysisUserContext, AnalysisScenarioContext, AnalysisScenarioData,
    InvestmentDebateState, RiskDebateState, ReflectionState,
    StructuredResearchPlan, StructuredTraderPlan, StructuredPortfolioDecision,
    AnalysisCheckpoint, RuntimeNodeTrace, AnalystRuntimeState,
};
""",
     []),
    ("src/analysis/report_types/plans.rs", "tests/analysis_report_types_plans.rs",
     """use sa::analysis::{
    TradeSetupQuality, CalibrationSummary, SetupMatchExplanation,
    HistoricalCalibrationStats, ScoreDimension, CalibrationBias,
    LocalText,
};
""",
     []),
    ("src/analysis/report_types/reflection.rs", "tests/analysis_report_types_reflection.rs",
     """use sa::analysis::{
    ActionScenarioPath, StructuredResearchPlan, StructuredTraderPlan,
    StructuredPortfolioDecision, MissingEvidenceLadder, CatalystScoreCard,
    CatalystScoreItem, ReviewChecklist, ReviewItem, Rating, LocalText,
};
""",
     []),
    ("src/analysis/report_types/risk_assessment.rs", "tests/analysis_report_types_risk_assessment.rs",
     """use sa::analysis::{
    StructuredReflection, StructuredRiskAssessment, ReportStageState,
    AnalystRuntimeState, AgentReportNode, DebateTurn, InvestmentDebateState,
    RiskDebateState, ReflectionState, AnalysisCheckpoint, AnalysisTaskSummary,
    RuntimeNodeTrace, LlmTokenUsageSummary, LocalText, TaskStatus,
};
""",
     []),
    ("src/analysis/report_types/views.rs", "tests/analysis_report_types_views.rs",
     """use sa::analysis::{
    PriceContext, ProbabilityView, ProbabilityDriver, ProfitRiskView,
    IcNavigatorView, IcDisciplineView, TechnicalIndicatorView,
    TechnicalIndicatorCategory, TechnicalIndicatorItem, TechnicalIndicatorConclusion,
    ReportEvidenceCard, NewsInsight, RiskControl, LocalText,
};
""",
     []),
    # derived + scenario_types
    ("src/analysis/derived.rs", "tests/analysis_derived.rs",
     """use sa::{AnalysisResult, LocalText, Rating, ReportStageState};
""",
     []),
    ("src/analysis/scenario_types.rs", "tests/analysis_scenario_types.rs",
     """use sa::analysis::{
    AnalysisScenarioMarket, AnalysisScenarioContext, AnalysisScenarioData,
    AnalysisScenarioIssue,
};
""",
     []),
    # report_logic files
    ("src/analysis/report_logic/calibration.rs", "tests/analysis_report_logic_calibration.rs",
     """use sa::analysis::{
    CalibrationBias, MemoryContextSnapshot, LocalText, Rating,
};
""",
     ["derive_calibration_bias", "fallback_sizing_reference"]),
    ("src/analysis/report_logic/catalyst_review.rs", "tests/analysis_report_logic_catalyst_review.rs",
     "",
     ["priority_rank"]),
    ("src/analysis/report_logic/chart.rs", "tests/analysis_report_logic_chart.rs",
     """use sa::analysis::{
    ReportMarketChart, ReportCandle, ChartOverlay, TrendLine, TrendLinePoint,
    PriceContext, LocalText,
};
""",
     ["compute_trend_lines", "add_overlay", "derive_price_context"]),
    ("src/analysis/report_logic/core/postlude.rs", "tests/analysis_report_logic_core_postlude.rs",
     """use sa::analysis::{
    StructuredTraderPlan, StructuredPortfolioDecision,
};
""",
     ["compute_reward_risk_hint", "extract_first_price"]),
    ("src/analysis/report_logic/news_insights.rs", "tests/analysis_report_logic_news_insights.rs",
     """use sa::analysis::{
    ReportReferenceSnapshot, ReferenceFactItem, ReportDiagnosticItem,
    DecisionView, DecisionAction, ReportEvidenceCard, LocalText,
};
""",
     ["derive_evidence_cards", "news_watch_next_summary", "has_report_diagnostic"]),
    ("src/analysis/report_logic/probability.rs", "tests/analysis_report_logic_probability.rs",
     "",
     ["round_to_100"]),
    ("src/analysis/report_logic/setup_quality.rs", "tests/analysis_report_logic_setup_quality.rs",
     """use sa::analysis::{
    StructuredResearchPlan, StructuredTraderPlan, StructuredPortfolioDecision,
    ReportDiagnostics, ReportDiagnosticItem, LocalText,
};
""",
     ["normalize_gap_to_i18n_key", "normalize_gap_match_text", "tokenize_gap_match_text",
      "score_related_gap_match", "related_gap_items", "collect_execution_blocking_gaps",
      "scenario_gap_messages"]),
    ("src/analysis/report_logic/setup_tags.rs", "tests/analysis_report_logic_setup_tags.rs",
     """use sa::analysis::{
    ConfidenceBreakdown, DirectionBreakdown, SignedScoreDimension, ExecutionReadiness,
    StructuredResearchPlan, StructuredTraderPlan, StructuredPortfolioDecision,
    ScoreDimension, LocalText,
};
""",
     []),  # derive_setup_tags is already pub
    ("src/analysis/report_logic/technical_indicators/calculations.rs",
     "tests/analysis_report_logic_technical_indicators_calculations.rs",
     """use sa::analysis::ReportCandle;
""",
     ["sma_report", "ema_report", "rsi_report", "atr_report", "bollinger_report",
      "macd_report", "kdj_report", "adx_report", "obv_report", "obv_signal"]),
    ("src/analysis/report_logic/trader_plan/calibration.rs",
     "tests/analysis_report_logic_trader_plan_calibration.rs",
     """use sa::analysis::{
    StructuredRiskAssessment,
};
""",
     ["first_non_empty_sentence", "strip_redundant_prefix", "split_semicolon_items",
      "normalize_semantic_snippet", "is_semantically_similar",
      "parse_risk_assessment_sections"]),
    # trader_plan/tests.rs - special case (includes other files)
    ("src/analysis/report_logic/trader_plan/tests.rs",
     "tests/analysis_report_logic_trader_plan_tests.rs",
     """use sa::{AnalysisResult, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph};
use sa::scoring::{calibrate_recommendation, evaluate_confidence_score, evaluate_direction_score, CalibrationProfile, calibrate_recommendation_with_profile, RecommendationCalibration, DirectionAssessment, ConfidenceAssessment};
use sa::analysis::{StructuredTraderPlan, StructuredPortfolioDecision};
""",
     []),  # calibrate_recommendation will need special handling
    # scoring files
    ("src/scoring/assessment/core.rs", "tests/scoring_assessment_core.rs",
     """use sa::analysis::{
    AnalysisResult, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts,
    AnalysisGraph, DebateTurn, StructuredTraderPlan, StructuredPortfolioDecision,
    StructuredResearchPlan, AnalystRuntimeState, LocalText,
};
use sa::scoring::{
    score_data_quality, score_trend_confirmation, score_fundamentals,
    score_catalyst_quality, score_historical_transferability,
    score_setup_direction_alignment, score_cross_agent_consistency, score_risk_clarity,
    DATA_QUALITY_MAX, TREND_CONFIRMATION_MAX,
};
""",
     []),
    ("src/scoring/dimensions/fundamental.rs", "tests/scoring_dimensions_fundamental.rs",
     """use sa::scoring::{FundamentalInput, score_fundamental, DimensionScore};
""",
     []),
    ("src/scoring/dimensions/llm_analysis.rs", "tests/scoring_dimensions_llm_analysis.rs",
     """use sa::scoring::{LlmAnalysisInput, score_llm_analysis, DimensionScore};
""",
     []),
    ("src/scoring/dimensions/sentiment.rs", "tests/scoring_dimensions_sentiment.rs",
     """use sa::scoring::DimensionScore;
""",
     ["parse_sentiment_response"]),
    ("src/scoring/dimensions/technical.rs", "tests/scoring_dimensions_technical.rs",
     """use sa::scoring::{TechnicalInput, score_technical, DimensionScore};
""",
     []),
    ("src/scoring/helpers/format.rs", "tests/scoring_helpers_format.rs",
     "",
     []),
    ("src/scoring/helpers/fundamental.rs", "tests/scoring_helpers_fundamental.rs",
     """use sa::analysis::{
    StructuredTraderPlan, StructuredPortfolioDecision,
};
""",
     ["numeric_tokens", "count_numeric_levels", "count_numeric_dates",
      "parse_first_number", "parse_position_percentage", "looks_like_ymd_date",
      "bool_text"]),
    ("src/scoring/helpers/technical.rs", "tests/scoring_helpers_technical.rs",
     """use sa::analysis::{
    AgentReportNode, StructuredTraderPlan, StructuredPortfolioDecision, Rating,
};
use sa::scoring::{
    normalized_key, is_cjk, analyst_matches, matches_semantic_alias,
    analyst_probability_quality, analyst_net_probability, score_analyst_net,
    rating_bias, map_direction_score_to_rating, direction_score_to_evidence_score,
    score_to_rating, has_execution_boundary, average_evidence_density,
};
""",
     []),
    ("src/scoring/history.rs", "tests/scoring_history.rs",
     """use sa::scoring::{
    compute_performance_report, StoredRecommendation, PriceSnapshot,
};
""",
     []),
    ("src/scoring/scorer.rs", "tests/scoring_scorer.rs",
     """use sa::scoring::{weighted_total, ScoreWeights, DimensionScore};
""",
     []),
    ("src/scoring/score_types.rs", "tests/scoring_score_types.rs",
     """use sa::scoring::{ScoreWeights, DimensionScore, score_label};
""",
     []),
    ("src/scoring/types/assessment/execution.rs", "tests/scoring_types_assessment_execution.rs",
     """use sa::scoring::{
    calibrate_recommendation, calibrate_recommendation_with_profile,
    evaluate_confidence_score, evaluate_direction_score, RecommendationCalibration,
    CalibrationProfile, DirectionAssessment, ConfidenceAssessment,
};
use sa::{AnalysisResult, AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph};
use sa::analysis::{StructuredTraderPlan, StructuredPortfolioDecision, AnalystRuntimeState, LocalText};
""",
     ["calibrate_recommendation"]),  # Remove #[cfg(test)] and make pub
]


def make_private_fn_pub(content, fn_name):
    """Change a private fn to pub fn. Handles both 'fn name(' and 'fn name ('."""
    # Match 'fn name(' at the start of a line (possibly with leading whitespace)
    patterns = [
        (r'(\n\s+)fn ' + re.escape(fn_name) + r'\(', r'\1pub fn ' + fn_name + '('),
    ]
    for old_pat, new_pat in patterns:
        new_content = re.sub(old_pat, new_pat, content)
        if new_content != content:
            return new_content
    return content


def process_file(src_rel, test_file, imports, fns_to_pub):
    src_path = os.path.join(BASE, src_rel)
    test_path = os.path.join(BASE, test_file)

    with open(src_path) as f:
        content = f.read()

    test_block, inner, mod_name = extract_test_block(content)
    if not test_block:
        print(f"SKIP: {src_rel} (no test block)")
        return

    # Make private functions public
    for fn_name in fns_to_pub:
        content = make_private_fn_pub(content, fn_name)

    # Special handling for calibrate_recommendation #[cfg(test)] function
    if "calibrate_recommendation" in fns_to_pub and src_rel == "src/scoring/types/assessment/execution.rs":
        content = content.replace('#[cfg(test)]\npub fn calibrate_recommendation', 'pub fn calibrate_recommendation')

    # Remove test block from source
    content = content.replace(test_block, '')
    content = content.rstrip() + '\n'

    with open(src_path, 'w') as f:
        f.write(content)

    # Create test file
    test_content = ""
    if imports.strip():
        test_content += imports.strip() + "\n"

    # Remove 'use super::*;' from inner content and replace with nothing
    inner = inner.replace('use super::*;', '')
    # Remove use crate::... lines that are already in imports
    # (we'll handle these manually)

    test_content += "\n" + inner + "\n"

    with open(test_path, 'w') as f:
        f.write(test_content)

    print(f"OK: {src_rel} -> {test_file} (removed {len(test_block)} chars, {len(fns_to_pub)} fns made pub)")


if __name__ == "__main__":
    for entry in FILES:
        process_file(*entry)
