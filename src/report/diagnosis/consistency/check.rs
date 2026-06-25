use crate::{AnalysisResult, DiagnosisIssue};

/// Severity of a consistency issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Info,
}

impl IssueSeverity {
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
/// and validated separately: clamped to [5, 95] and enforced >= downside.
pub fn fix_probabilities(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
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

// ======================================================================
// Check 2: Entry == stop-loss guard
// ======================================================================

/// If the trader plan's entry_price equals stop_loss, the stop is
/// meaningless. Set stop = entry * 0.98 as a conservative default.
pub fn fix_entry_stop(result: &mut AnalysisResult) -> Vec<DiagnosisIssue> {
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

// ======================================================================
// Check 3: Risk:Reward ratio
// ======================================================================

/// R:R = (target - entry) / (entry - stop). If R:R < 1.5, widen the
/// target to entry + 1.5 * (entry - stop).
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

    if rr >= 1.5 {
        return Vec::new();
    }

    let new_target = round_price(entry + 1.5 * risk);
    let new_rr = 1.5;

    result.report.decision_view.target_reference =
        crate::LocalText::new(format!("{:.2}", new_target));
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
