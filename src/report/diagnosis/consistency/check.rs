use crate::{
    AnalysisResult, DecisionViewDirection, DiagnosisIssue, analysis::ExecutionPrerequisite,
};

/// Severity of a consistency issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Info,
}

impl IssueSeverity {
    /// Compute As_str.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

// ======================================================================
// Helpers
// ======================================================================

/// Compute Make_issue.
pub fn make_issue(
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
pub fn parse_price(s: &str) -> Option<f64> {
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
pub fn round_price(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Extract the first percentage number from a string like "30%" or "20% of portfolio".
pub fn extract_pct(s: &str) -> Option<f64> {
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

// ======================================================================
// Check 1: Probability normalization
// ======================================================================

/// Validate and fix probability fields. The directional trio
/// (up+down+sideways) is normalized independently to ~100%.
/// Risk probability is treated as an independent overlay metric
/// and validated separately: clamped to [5, 95] and enforced above the
/// probability of the adverse price direction for the active trade.
pub fn fix_probabilities(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    let is_bearish = result.report.decision_view.view == DecisionViewDirection::Bearish;
    let pv = &mut result.report.probability_view;

    // --- Check A: Directional trio sums to ~100% ---
    let directional_sum =
        pv.upside_probability_pct + pv.downside_probability_pct + pv.sideways_probability_pct;

    if directional_sum > 0.0 && (directional_sum - 100.0).abs() > 5.0 {
        let original = format!(
            "up={:.1}%, down={:.1}%, sideways={:.1}% (sum={:.1}%)",
            pv.upside_probability_pct,
            pv.downside_probability_pct,
            pv.sideways_probability_pct,
            directional_sum,
        );

        let scale = 100.0 / directional_sum;
        pv.upside_probability_pct = (pv.upside_probability_pct * scale * 10.0).round() / 10.0;
        pv.downside_probability_pct = (pv.downside_probability_pct * scale * 10.0).round() / 10.0;
        pv.sideways_probability_pct =
            (100.0 - pv.upside_probability_pct - pv.downside_probability_pct).max(0.0);

        let fixed = format!(
            "up={:.1}%, down={:.1}%, sideways={:.1}%",
            pv.upside_probability_pct, pv.downside_probability_pct, pv.sideways_probability_pct,
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

    // --- Check C: Risk >= adverse direction (logical invariant) ---
    let adverse_probability = if is_bearish {
        pv.upside_probability_pct
    } else {
        pv.downside_probability_pct
    };
    if pv.risk_probability_pct > 0.0
        && adverse_probability > 0.0
        && pv.risk_probability_pct < adverse_probability
    {
        let original_risk = pv.risk_probability_pct;
        pv.risk_probability_pct = (adverse_probability + 5.0).min(95.0);

        tracing::warn!(
            check = "fix_risk_invariant",
            original = original_risk,
            fixed = pv.risk_probability_pct,
            adverse_probability,
            "risk probability below the adverse direction, adjusted to adverse + 5"
        );

        issues.push(make_issue(
            IssueSeverity::Warning,
            "fix_risk_invariant",
            "probability_view.risk_probability_pct",
            &format!("{:.1}%", original_risk),
            &format!("{:.1}%", pv.risk_probability_pct),
            "Risk probability was below the adverse direction, adjusted to adverse + 5%",
        ));
    }

    issues
}

// ======================================================================
// Check 2: Entry == stop-loss guard
// ======================================================================

/// If the trader plan's entry_price equals stop_loss, the stop is
/// meaningless. Set stop = entry * 0.98 as a conservative default.
pub fn fix_entry_stop(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
    let is_bearish = result.report.decision_view.view == DecisionViewDirection::Bearish;
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

    let new_stop = if is_bearish {
        round_price(entry * 1.02)
    } else {
        round_price(entry * 0.98)
    };
    result.report.trader_plan.stop_loss = format!("{:.2}", new_stop);

    // Do NOT modify decision_view.invalidation_level here.
    // The DecisionView derives its invalidation from portfolio_decision.invalidation_level,
    // which is the authoritative source. Modifying it here creates dual invalidation values
    // (decision_view.invalidation_level vs decision_view.invalidation_price) that confuse users.

    tracing::warn!(
        check = "fix_entry_stop",
        entry = entry,
        original_stop = stop,
        new_stop = new_stop,
        "entry == stop-loss, adjusted stop in the adverse price direction"
    );

    vec![make_issue(
        IssueSeverity::Warning,
        "fix_entry_stop",
        "trader_plan.stop_loss",
        &format!("{:.2}", entry),
        &format!("{:.2}", new_stop),
        "Entry price equals stop-loss; adjusted stop in the adverse price direction",
    )]
}

// ======================================================================
// Check 2b: Entry < invalidation guard
// ======================================================================

/// If entry_price < invalidation_level, the risk control logic is inverted
/// (buying below the stop). Lower invalidation to stop_loss or entry * 0.95.
pub fn fix_entry_invalidation(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
    // For a short execution plan the valid structure is target < entry < stop.
    // `invalidation_level` is therefore intentionally above entry and must not
    // be rewritten by this long-only consistency repair after report assembly.
    if result.report.decision_view.view == DecisionViewDirection::Bearish {
        return Vec::new();
    }

    let entry_str = result
        .report
        .decision_view
        .entry_reference
        .trim()
        .to_string();
    let inval_str = result
        .report
        .decision_view
        .invalidation_level
        .trim()
        .to_string();

    if entry_str.is_empty() || inval_str.is_empty() {
        return Vec::new();
    }

    let entry = parse_price(&entry_str);
    let inval = parse_price(&inval_str);

    let (Some(entry), Some(inval)) = (entry, inval) else {
        return Vec::new();
    };

    if entry <= 0.0 || inval <= 0.0 || entry >= inval {
        return Vec::new();
    }

    // entry < invalidation — derive a corrected invalidation
    let stop = parse_price(result.report.trader_plan.stop_loss.trim());
    let new_inval = if let Some(s) = stop.filter(|&s| s > 0.0 && s < entry) {
        round_price(s)
    } else {
        round_price(entry * 0.95)
    };

    let original = format!("entry={:.2}, invalidation={:.2}", entry, inval);
    let fixed = format!("{:.2}", new_inval);

    result.report.decision_view.invalidation_level = fixed.clone();
    result.report.portfolio_decision.invalidation_level = fixed.clone();

    tracing::warn!(
        check = "fix_entry_invalidation",
        entry = entry,
        original_invalidation = inval,
        new_invalidation = new_inval,
        "entry < invalidation_level, lowered invalidation"
    );

    vec![make_issue(
        IssueSeverity::Warning,
        "fix_entry_invalidation",
        "decision_view.invalidation_level",
        &original,
        &fixed,
        "Entry price below invalidation level; lowered invalidation to stop-loss or entry * 0.95",
    )]
}

// ======================================================================
// Check 3: Risk:Reward ratio
// ======================================================================

/// If R:R is below 1.5, extend the profit target in the direction of the
/// established trade while preserving one canonical target across all views.
pub fn fix_risk_reward(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
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

    const MIN_ACTIVE_REWARD_RISK: f64 = 2.0;
    if rr >= MIN_ACTIVE_REWARD_RISK {
        return Vec::new();
    }

    let boundary = &mut result.report.execution_boundary;
    boundary.minimum_reward_risk = MIN_ACTIVE_REWARD_RISK;
    boundary.actual_reward_risk = Some(rr);
    boundary.active_execution_allowed = false;
    if !boundary
        .prerequisites
        .contains(&ExecutionPrerequisite::MinimumRewardRisk)
    {
        boundary
            .prerequisites
            .push(ExecutionPrerequisite::MinimumRewardRisk);
    }
    result.report.profit_risk.reward_risk_ratio = Some(rr);
    result.report.ic_discipline.reward_risk_ratio = Some(rr);
    let target_text = format!("{target:.2}");
    result.report.decision_view.target_reference =
        crate::LocalText::new("target_reference_value").with_str("value", target_text.clone());
    result.report.decision_view.first_target = target_text.clone();
    result.report.trader_plan.target_reference = target_text.clone();
    result.report.portfolio_decision.price_target = target_text.clone();
    result.report.portfolio_decision.target_reference = target_text;
    result.report.profit_risk.calc_target = Some(target);
    result.report.profit_risk.calc_stop = Some(stop);
    result.report.probability_view.profit_target = Some(target);
    result.report.probability_view.stop_loss = Some(stop);

    vec![make_issue(
        IssueSeverity::Warning,
        "block_low_reward_risk",
        "execution_boundary.active_execution_allowed",
        &format!("R:R={rr:.2}"),
        "false",
        "Active execution blocked because the real target does not meet the 2.0 reward/risk minimum",
    )]
}
