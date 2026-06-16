use crate::models::{AnalysisResult, DiagnosisIssue, Rating};

/// Severity of a consistency issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    #[allow(dead_code)] // Used via matches! macro
    Error,
    Warning,
    Info,
}

impl IssueSeverity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

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
        issues.extend(Self::fix_probabilities(result));
        issues.extend(Self::fix_entry_stop(result));
        issues.extend(Self::fix_risk_reward(result));
        issues.extend(Self::fix_position_sizing(result));
        issues.extend(Self::fix_recommendation_consistency(result));
        issues.extend(Self::fill_missing_fields(result));
        issues
    }

    // ------------------------------------------------------------------
    // Check 1: Probability normalization
    // ------------------------------------------------------------------

    /// Validate and fix probability fields. The directional trio
    /// (up+down+sideways) is normalized independently to ~100%.
    /// Risk probability is treated as an independent overlay metric
    /// and validated separately: clamped to [5, 95] and enforced >= downside.
    fn fix_probabilities(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let mut issues = Vec::new();
        let pv = &mut result.report.probability_view;

        // --- Check A: Directional trio sums to ~100% ---
        let directional_sum = pv.upside_probability_pct
            + pv.downside_probability_pct
            + pv.sideways_probability_pct;

        if directional_sum > 0.0 && (directional_sum - 100.0).abs() > 5.0 {
            let original = format!(
                "up={:.1}%, down={:.1}%, sideways={:.1}% (sum={:.1}%)",
                pv.upside_probability_pct,
                pv.downside_probability_pct,
                pv.sideways_probability_pct,
                directional_sum,
            );

            let scale = 100.0 / directional_sum;
            pv.upside_probability_pct =
                (pv.upside_probability_pct * scale * 10.0).round() / 10.0;
            pv.downside_probability_pct =
                (pv.downside_probability_pct * scale * 10.0).round() / 10.0;
            pv.sideways_probability_pct = (100.0
                - pv.upside_probability_pct
                - pv.downside_probability_pct)
                .max(0.0);

            let fixed = format!(
                "up={:.1}%, down={:.1}%, sideways={:.1}%",
                pv.upside_probability_pct,
                pv.downside_probability_pct,
                pv.sideways_probability_pct,
            );

            tracing::warn!(
                check = "fix_probabilities",
                original = %original,
                fixed = %fixed,
                "directional trio did not sum to ~100%, normalized proportionally"
            );

            issues.push(make_issue(
                IssueSeverity::Warning,
                "fix_probabilities",
                "probability_view",
                &original,
                &fixed,
                "Directional probabilities did not sum to ~100%, normalized proportionally",
            ));
        }

        // --- Check B: Risk probability range [5, 95] ---
        if pv.risk_probability_pct > 0.0 {
            let original_risk = pv.risk_probability_pct;
            pv.risk_probability_pct = pv.risk_probability_pct.clamp(5.0, 95.0);
            if (pv.risk_probability_pct - original_risk).abs() > f64::EPSILON {
                tracing::warn!(
                    check = "fix_risk_range",
                    original = original_risk,
                    fixed = pv.risk_probability_pct,
                    "risk probability outside [5, 95], clamped"
                );
                issues.push(make_issue(
                    IssueSeverity::Warning,
                    "fix_risk_range",
                    "probability_view.risk_probability_pct",
                    &format!("{:.1}%", original_risk),
                    &format!("{:.1}%", pv.risk_probability_pct),
                    "Risk probability outside [5, 95] range, clamped",
                ));
            }
        }

        // --- Check C: Risk >= downside (logical invariant) ---
        if pv.risk_probability_pct > 0.0
            && pv.downside_probability_pct > 0.0
            && pv.risk_probability_pct < pv.downside_probability_pct
        {
            let original_risk = pv.risk_probability_pct;
            pv.risk_probability_pct = (pv.downside_probability_pct + 5.0).min(95.0);

            tracing::warn!(
                check = "fix_risk_invariant",
                original = original_risk,
                fixed = pv.risk_probability_pct,
                downside = pv.downside_probability_pct,
                "risk probability < downside, adjusted to downside + 5"
            );

            issues.push(make_issue(
                IssueSeverity::Warning,
                "fix_risk_invariant",
                "probability_view.risk_probability_pct",
                &format!("{:.1}%", original_risk),
                &format!("{:.1}%", pv.risk_probability_pct),
                "Risk probability was less than downside probability, adjusted to downside + 5%",
            ));
        }

        issues
    }

    // ------------------------------------------------------------------
    // Check 2: Entry == stop-loss guard
    // ------------------------------------------------------------------

    /// If the trader plan's entry_price equals stop_loss, the stop is
    /// meaningless. Set stop = entry * 0.98 as a conservative default.
    fn fix_entry_stop(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let entry_str = result.report.trader_plan.entry_price.trim().to_string();
        let stop_str = result.report.trader_plan.stop_loss.trim().to_string();

        if entry_str.is_empty() || stop_str.is_empty() {
            return Vec::new();
        }

        let entry = parse_price(&entry_str);
        let stop = parse_price(&stop_str);

        let (Some(entry), Some(stop)) = (entry, stop) else {
            return Vec::new();
        };

        if entry <= 0.0 || stop <= 0.0 {
            return Vec::new();
        }

        // Check if entry and stop are effectively the same (within 0.1%).
        if (entry - stop).abs() / entry > 0.001 {
            return Vec::new();
        }

        let new_stop = round_price(entry * 0.98);
        result.report.trader_plan.stop_loss = format!("{:.2}", new_stop);

        // Also update the decision view invalidation level if it matched.
        if result.report.decision_view.invalidation_level.trim() == entry_str
            || parse_price(result.report.decision_view.invalidation_level.trim()) == Some(entry)
        {
            result.report.decision_view.invalidation_level = format!("{:.2}", new_stop);
        }

        tracing::warn!(
            check = "fix_entry_stop",
            entry = entry,
            original_stop = stop,
            new_stop = new_stop,
            "entry == stop-loss, adjusted stop to entry * 0.98"
        );

        vec![make_issue(
            IssueSeverity::Warning,
            "fix_entry_stop",
            "trader_plan.stop_loss",
            &format!("{:.2}", entry),
            &format!("{:.2}", new_stop),
            "Entry price equals stop-loss; adjusted stop to entry * 0.98",
        )]
    }

    // ------------------------------------------------------------------
    // Check 3: Risk:Reward ratio
    // ------------------------------------------------------------------

    /// R:R = (target - entry) / (entry - stop). If R:R < 1.5, widen the
    /// target to entry + 1.5 * (entry - stop).
    fn fix_risk_reward(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
        let entry = parse_price(result.report.trader_plan.entry_price.trim());
        let stop = parse_price(result.report.trader_plan.stop_loss.trim());

        // Try target from decision_view first, then trader_plan.
        let target = parse_price(result.report.decision_view.target_reference.trim())
            .or_else(|| parse_price(result.report.trader_plan.target_reference.trim()));

        let (Some(entry), Some(stop), Some(target)) = (entry, stop, target) else {
            return Vec::new();
        };

        if entry <= 0.0 || stop <= 0.0 || target <= 0.0 {
            return Vec::new();
        }

        let risk = (entry - stop).abs();
        if risk <= 0.0 {
            return Vec::new();
        }

        let reward = (target - entry).abs();
        let rr = reward / risk;

        if rr >= 1.5 {
            return Vec::new();
        }

        let new_target = round_price(entry + 1.5 * risk);
        let new_rr = 1.5;

        result.report.decision_view.target_reference =
            crate::models::LocalText::new(format!("{:.2}", new_target));
        result.report.trader_plan.target_reference = format!("{:.2}", new_target);

        tracing::warn!(
            check = "fix_risk_reward",
            entry = entry,
            stop = stop,
            original_target = target,
            new_target = new_target,
            original_rr = format!("{:.2}", rr),
            new_rr = new_rr,
            "R:R below 1.5, widened target"
        );

        vec![make_issue(
            IssueSeverity::Warning,
            "fix_risk_reward",
            "target_reference",
            &format!("target={:.2}, R:R={:.2}", target, rr),
            &format!("target={:.2}, R:R={:.2}", new_target, new_rr),
            "Risk:reward ratio below 1.5, widened target to entry + 1.5 * risk",
        )]
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
            use crate::models::{ActionScenarioPath, LocalText};

            result
                .report
                .action_guides
                .buyers
                .scenario_paths
                .push(ActionScenarioPath {
                    key: "default_hold".to_string(),
                    name: LocalText::new("Observe and Wait"),
                    trigger: LocalText::new("Wait for clear technical signal or catalyst confirmation"),
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

// ======================================================================
// Helpers
// ======================================================================

fn make_issue(
    severity: IssueSeverity,
    check_name: &str,
    field: &str,
    original_value: &str,
    fixed_value: &str,
    message: &str,
) -> DiagnosisIssue {
    DiagnosisIssue {
        severity: severity.as_str().to_string(),
        check_name: check_name.to_string(),
        field: field.to_string(),
        original_value: original_value.to_string(),
        fixed_value: fixed_value.to_string(),
        message: message.to_string(),
    }
}

/// Parse a price string like "12.50", "$12.50", "12.50 CNY", or "12.50元".
fn parse_price(s: &str) -> Option<f64> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Round a price to two decimal places.
fn round_price(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Extract the first percentage number from a string like "30%" or "20% of portfolio".
fn extract_pct(s: &str) -> Option<f64> {
    let mut num = String::new();
    let mut found_digit = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() || (ch == '.' && found_digit) {
            num.push(ch);
            found_digit = true;
        } else if found_digit {
            break;
        }
    }
    num.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

#[cfg(test)]
mod consistency_tests;
