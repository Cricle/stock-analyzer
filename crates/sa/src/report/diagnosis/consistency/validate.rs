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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnalysisResult;

    fn default_result() -> AnalysisResult {
        AnalysisResult {
            task_id: "test".to_string(),
            report_id: "report-test".to_string(),
            symbol: "TEST".to_string(),
            stock_name: "Test Corp".to_string(),
            analysis_date: "2026-06-05".to_string(),
            market_type: "US".to_string(),
            graph: Default::default(),
            agent_state: Default::default(),
            artifacts: Default::default(),
            report: Default::default(),
            ic_report: Default::default(),
            created_at: "2026-06-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_probability_normalization() {
        let mut result = default_result();
        result.report.probability_view.upside_probability_pct = 60.0;
        result.report.probability_view.downside_probability_pct = 50.0;
        result.report.probability_view.sideways_probability_pct = 30.0;
        result.report.probability_view.risk_probability_pct = 10.0;
        // Directional sum = 140%, should be normalized to ~100%.

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let prob_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_probabilities")
            .collect();
        assert_eq!(prob_issues.len(), 1);

        let pv = &result.report.probability_view;
        let directional_sum =
            pv.upside_probability_pct + pv.downside_probability_pct + pv.sideways_probability_pct;
        assert!(
            (directional_sum - 100.0).abs() < 2.0,
            "directional trio should sum to ~100, got {}",
            directional_sum
        );
    }

    #[test]
    fn test_probabilities_within_tolerance_are_unchanged() {
        let mut result = default_result();
        result.report.probability_view.upside_probability_pct = 45.0;
        result.report.probability_view.downside_probability_pct = 30.0;
        result.report.probability_view.sideways_probability_pct = 28.0;
        result.report.probability_view.risk_probability_pct = 35.0;
        // Directional sum = 103%, within 5% tolerance of 100. Risk >= downside.

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let prob_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_probabilities")
            .collect();
        assert!(prob_issues.is_empty());
    }

    #[test]
    fn test_entry_stop_guard() {
        let mut result = default_result();
        result.report.trader_plan.entry_price = "100.00".to_string();
        result.report.trader_plan.stop_loss = "100.00".to_string();

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let stop_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_entry_stop")
            .collect();
        assert_eq!(stop_issues.len(), 1);

        let stop = super::super::check::parse_price(&result.report.trader_plan.stop_loss).unwrap();
        assert!(
            (stop - 98.0).abs() < 0.1,
            "stop should be ~98.0, got {}",
            stop
        );
    }

    #[test]
    fn test_risk_reward_widening() {
        let mut result = default_result();
        result.report.trader_plan.entry_price = "100.00".to_string();
        result.report.trader_plan.stop_loss = "95.00".to_string();
        result.report.decision_view.target_reference = crate::LocalText::new("103.00");
        // R:R = (103-100)/(100-95) = 0.6 < 1.5

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let rr_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_risk_reward")
            .collect();
        assert_eq!(rr_issues.len(), 1);

        let target = super::super::check::parse_price(result.report.decision_view.target_reference.as_str()).unwrap();
        // Expected: 100 + 1.5 * 5 = 107.50
        assert!(
            (target - 107.5).abs() < 0.1,
            "target should be ~107.5, got {}",
            target
        );
    }

    #[test]
    fn test_position_sizing_cap() {
        let mut result = default_result();
        result.report.trader_plan.position_sizing = "30% of portfolio".to_string();

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let sizing_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_position_sizing")
            .collect();
        assert_eq!(sizing_issues.len(), 1);
        assert!(
            result.report.trader_plan.position_sizing.contains("20%"),
            "should be capped at 20%, got: {}",
            result.report.trader_plan.position_sizing
        );
    }

    #[test]
    fn test_recommendation_downgrade_buy_with_high_downside() {
        let mut result = default_result();
        result.report.recommendation = "Buy".into();
        result.report.probability_view.downside_probability_pct = 70.0;

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let rec_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_recommendation_consistency")
            .collect();
        assert_eq!(rec_issues.len(), 1);
        assert_eq!(result.report.recommendation, "Hold".into());
    }

    #[test]
    fn test_fill_missing_recommendation() {
        let mut result = default_result();
        result.report.recommendation = "".into();
        result.report.direction_score = 10;

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let fill_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fill_missing_fields" && i.field == "recommendation")
            .collect();
        assert_eq!(fill_issues.len(), 1);
        assert_eq!(result.report.recommendation, "Buy".into());
    }

    #[test]
    fn test_fill_missing_confidence() {
        let mut result = default_result();
        result.report.confidence = "".into();

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let fill_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fill_missing_fields" && i.field == "confidence")
            .collect();
        assert_eq!(fill_issues.len(), 1);
        assert_eq!(result.report.confidence, "Low".into());
    }

    #[test]
    fn test_fill_missing_scenario_paths() {
        let mut result = default_result();
        // Both buyer and holder paths are empty by default.

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let path_issues: Vec<_> = issues
            .iter()
            .filter(|i| {
                i.check_name == "fill_missing_fields"
                    && i.field == "action_guides.buyers.scenario_paths"
            })
            .collect();
        assert_eq!(path_issues.len(), 1);
        assert!(!result.report.action_guides.buyers.scenario_paths.is_empty());
    }

    #[test]
    fn test_no_issues_on_clean_result() {
        let mut result = default_result();
        result.report.recommendation = "Hold".into();
        result.report.confidence = "Medium".into();
        result.report.probability_view.upside_probability_pct = 30.0;
        result.report.probability_view.downside_probability_pct = 25.0;
        result.report.probability_view.sideways_probability_pct = 45.0;
        result.report.probability_view.risk_probability_pct = 30.0;
        result.report.trader_plan.entry_price = "100.00".to_string();
        result.report.trader_plan.stop_loss = "95.00".to_string();
        result.report.trader_plan.position_sizing = "10%".to_string();
        result.report.decision_view.target_reference = crate::LocalText::new("120.00");

        use crate::{ActionScenarioPath, LocalText};
        result
            .report
            .action_guides
            .buyers
            .scenario_paths
            .push(ActionScenarioPath {
                key: "test".to_string(),
                name: LocalText::new("Test"),
                trigger: LocalText::new("Test"),
                action: LocalText::new("Test"),
                risk_boundary: LocalText::new("Test"),
                position_sizing: LocalText::new("10%"),
                stop_level: LocalText::new("95"),
                sizing_blocked: false,
            });

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        assert!(
            issues.is_empty(),
            "clean result should have no issues, got {}",
            issues.len()
        );
    }

    #[test]
    fn test_probability_normalization_directional_only() {
        let mut result = default_result();
        result.report.probability_view.upside_probability_pct = 60.0;
        result.report.probability_view.downside_probability_pct = 50.0;
        result.report.probability_view.sideways_probability_pct = 30.0;
        result.report.probability_view.risk_probability_pct = 40.0;
        // Directional sum = 140%, risk = 40% (independent, stays unchanged).

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let prob_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_probabilities")
            .collect();
        assert_eq!(
            prob_issues.len(),
            1,
            "should trigger directional normalization"
        );

        let pv = &result.report.probability_view;
        let directional_sum =
            pv.upside_probability_pct + pv.downside_probability_pct + pv.sideways_probability_pct;
        assert!(
            (directional_sum - 100.0).abs() < 2.0,
            "directional trio should sum to ~100, got {}",
            directional_sum
        );
        assert!(
            (pv.risk_probability_pct - 40.0).abs() < 1.0,
            "risk should stay at ~40, got {}",
            pv.risk_probability_pct
        );
    }

    #[test]
    fn test_risk_at_least_downside() {
        let mut result = default_result();
        result.report.probability_view.upside_probability_pct = 30.0;
        result.report.probability_view.downside_probability_pct = 30.0;
        result.report.probability_view.sideways_probability_pct = 30.0;
        result.report.probability_view.risk_probability_pct = 20.0;
        // Directional sum = 90%, within tolerance. But risk (20) < downside (30).

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let risk_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_risk_invariant")
            .collect();
        assert_eq!(risk_issues.len(), 1, "should trigger risk >= downside fix");

        let pv = &result.report.probability_view;
        assert!(
            pv.risk_probability_pct >= pv.downside_probability_pct,
            "risk ({}) should be >= downside ({})",
            pv.risk_probability_pct,
            pv.downside_probability_pct
        );
    }

    #[test]
    fn test_risk_clamped_to_valid_range() {
        let mut result = default_result();
        result.report.probability_view.upside_probability_pct = 30.0;
        result.report.probability_view.downside_probability_pct = 25.0;
        result.report.probability_view.sideways_probability_pct = 35.0;
        result.report.probability_view.risk_probability_pct = 98.0;
        // Directional sum = 90%, within tolerance. Risk = 98, should clamp to 95.

        let issues = ConsistencyValidator::validate_and_fix(&mut result);
        let risk_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.check_name == "fix_risk_range")
            .collect();
        assert_eq!(risk_issues.len(), 1, "should trigger risk range clamp");

        let pv = &result.report.probability_view;
        assert!(
            pv.risk_probability_pct <= 95.0,
            "risk should be clamped to <= 95, got {}",
            pv.risk_probability_pct
        );
    }
}
