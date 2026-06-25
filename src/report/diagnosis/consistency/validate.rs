use crate::{AnalysisResult, DiagnosisIssue, Rating};

use super::check::{IssueSeverity, extract_pct, make_issue};

/// Validates `AnalysisResult` for internal consistency and auto-fixes common
/// LLM output problems (probability normalization, entry/stop parity, R:R,
/// position sizing caps, recommendation-probability mismatch, missing fields).
///
/// Runs after all LLM stages complete and before the result is persisted.
pub struct ConsistencyValidator;

impl ConsistencyValidator {
    /// Run all checks, mutate `result` in-place, and return a list of issues
    /// describing what was found and fixed.
    pub fn validate_and_fix(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let mut issues = Vec::new();
        issues.extend(super::check::fix_probabilities(result));
        issues.extend(super::check::fix_entry_stop(result));
        issues.extend(super::check::fix_risk_reward(result));
        issues.extend(Self::fix_position_sizing(result));
        issues.extend(Self::fix_recommendation_consistency(result));
        issues.extend(Self::fill_missing_fields(result));
        issues
    }

    // ------------------------------------------------------------------
    // Check 4: Position sizing cap
    // ------------------------------------------------------------------

    /// If position_sizing mentions a percentage > 25%, cap at 20%.
    fn fix_position_sizing(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let sizing = result.report.trader_plan.position_sizing.clone();
        if sizing.is_empty() {
            return Vec::new();
        }

        let pct = extract_pct(&sizing);
        let Some(pct) = pct else {
            return Vec::new();
        };

        if pct <= 25.0 {
            return Vec::new();
        }

        let new_sizing = sizing.replace(&format!("{:.0}%", pct), "20%");
        // If the simple replacement didn't work (e.g. "25.5%"), try with decimals.
        let new_sizing = if new_sizing == sizing {
            sizing.replace(&format!("{:.1}%", pct), "20%")
        } else {
            new_sizing
        };
        let new_sizing = if new_sizing == sizing {
            // Last resort: replace any >25% pattern
            sizing.replacen(
                sizing
                    .find('%')
                    .map(|i| {
                        let start = sizing[..i]
                            .rfind(|c: char| !c.is_ascii_digit() && c != '.')
                            .map(|s| s + 1)
                            .unwrap_or(0);
                        &sizing[start..i + 1]
                    })
                    .unwrap_or(""),
                "20%",
                1,
            )
        } else {
            new_sizing
        };

        result.report.trader_plan.position_sizing = new_sizing.clone();

        tracing::warn!(
            check = "fix_position_sizing",
            original_pct = pct,
            "position sizing capped at 20%"
        );

        vec![make_issue(
            IssueSeverity::Warning,
            "fix_position_sizing",
            "trader_plan.position_sizing",
            &sizing,
            &new_sizing,
            &format!("Position sizing {:.0}% exceeds 25%, capped at 20%", pct),
        )]
    }

    // ------------------------------------------------------------------
    // Check 5: Recommendation-probability consistency
    // ------------------------------------------------------------------

    /// If recommendation=Buy but downside_probability > 60%, downgrade to
    /// Hold. Similarly for Sell with upside > 60%.
    fn fix_recommendation_consistency(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let rec = result.report.recommendation.trim().to_string();
        if rec.is_empty() {
            return Vec::new();
        }

        let rating = Rating::parse(&rec);
        let up = result.report.probability_view.upside_probability_pct;
        let down = result.report.probability_view.downside_probability_pct;

        let (should_downgrade, reason) = if rating.is_bullish() && down > 60.0 {
            (
                true,
                format!("Buy recommendation but downside probability {:.1}%", down),
            )
        } else if rating.is_bearish() && up > 60.0 {
            (
                true,
                format!("Sell recommendation but upside probability {:.1}%", up),
            )
        } else {
            (false, String::new())
        };

        if !should_downgrade {
            return Vec::new();
        }

        let original = rec.clone();
        result.report.recommendation = "Hold".into();
        // Also update the portfolio decision rating.
        result.report.portfolio_decision.rating = Rating::Hold;

        tracing::warn!(
            check = "fix_recommendation_consistency",
            original = %original,
            reason = %reason,
            "downgraded recommendation to Hold"
        );

        vec![make_issue(
            IssueSeverity::Warning,
            "fix_recommendation_consistency",
            "recommendation",
            &original,
            "Hold",
            &reason,
        )]
    }

    // ------------------------------------------------------------------
    // Check 6: Fill missing fields
    // ------------------------------------------------------------------

    /// Derive empty recommendation from direction_score, empty confidence
    /// from evidence count, and empty scenario paths from defaults.
    fn fill_missing_fields(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let mut issues = Vec::new();

        // 6a: Empty recommendation -> derive from direction_score
        if result.report.recommendation.trim().is_empty() {
            let score = result.report.direction_score;
            let derived = if score >= 8 {
                "Buy"
            } else if score >= 4 {
                "Overweight"
            } else if score >= -3 {
                "Hold"
            } else if score >= -7 {
                "Underweight"
            } else {
                "Sell"
            };
            result.report.recommendation = derived.into();
            tracing::info!(
                check = "fill_missing_fields",
                field = "recommendation",
                direction_score = score,
                derived = derived,
                "recommendation was empty, derived from direction_score"
            );
            issues.push(make_issue(
                IssueSeverity::Info,
                "fill_missing_fields",
                "recommendation",
                "",
                derived,
                &format!(
                    "Recommendation was empty; derived '{}' from direction_score={}",
                    derived, score
                ),
            ));
        }

        // 6b: Empty confidence -> derive from evidence count
        if result.report.confidence.trim().is_empty() {
            let evidence_count = result.report.evidence_cards.len()
                + result.report.news_insights.len()
                + result
                    .report
                    .technical_indicators
                    .categories
                    .iter()
                    .map(|c| c.indicators.len())
                    .sum::<usize>();
            let derived = if evidence_count >= 20 {
                "High"
            } else if evidence_count >= 8 {
                "Medium"
            } else {
                "Low"
            };
            result.report.confidence = derived.into();
            tracing::info!(
                check = "fill_missing_fields",
                field = "confidence",
                evidence_count = evidence_count,
                derived = derived,
                "confidence was empty, derived from evidence count"
            );
            issues.push(make_issue(
                IssueSeverity::Info,
                "fill_missing_fields",
                "confidence",
                "",
                derived,
                &format!(
                    "Confidence was empty; derived '{}' from evidence_count={}",
                    derived, evidence_count
                ),
            ));
        }

        // 6c: All-zero probabilities -> populate with a default Hold view
        let pv = &result.report.probability_view;
        let prob_sum = pv.upside_probability_pct
            + pv.downside_probability_pct
            + pv.sideways_probability_pct
            + pv.risk_probability_pct;
        if prob_sum <= 0.0 {
            result.report.probability_view.upside_probability_pct = 30.0;
            result.report.probability_view.downside_probability_pct = 25.0;
            result.report.probability_view.sideways_probability_pct = 35.0;
            result.report.probability_view.risk_probability_pct = 10.0;
            tracing::info!(
                check = "fill_missing_fields",
                field = "probability_view",
                "all probabilities were zero, set default Hold distribution"
            );
            issues.push(make_issue(
                IssueSeverity::Info,
                "fill_missing_fields",
                "probability_view",
                "all zero",
                "up=30%, down=25%, sideways=35%, risk=10%",
                "All probabilities were zero; set default Hold/Observe distribution",
            ));
        }

        // 6d: Empty scenario paths -> generate default Hold paths
        let has_buy_paths = !result.report.action_guides.buyers.scenario_paths.is_empty();
        let has_sell_paths = !result
            .report
            .action_guides
            .holders
            .scenario_paths
            .is_empty();
        if !has_buy_paths && !has_sell_paths {
            use crate::{ActionScenarioPath, LocalText};

            result
                .report
                .action_guides
                .buyers
                .scenario_paths
                .push(ActionScenarioPath {
                    key: "default_hold".to_string(),
                    name: LocalText::new("Observe and Wait"),
                    trigger: LocalText::new(
                        "Wait for clear technical signal or catalyst confirmation",
                    ),
                    action: LocalText::new("Hold observation, no new positions"),
                    risk_boundary: LocalText::new("None"),
                    position_sizing: LocalText::new("0% - Wait for signal confirmation"),
                    stop_level: LocalText::new(""),
                    sizing_blocked: true,
                });

            tracing::info!(
                check = "fill_missing_fields",
                field = "action_guides.scenario_paths",
                "scenario paths were empty, generated default Hold/Observe path"
            );
            issues.push(make_issue(
                IssueSeverity::Info,
                "fill_missing_fields",
                "action_guides.buyers.scenario_paths",
                "empty",
                "default Hold/Observe path",
                "Scenario paths were empty; generated a default Hold/Observe path",
            ));
        }

        issues
    }
}
