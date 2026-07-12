/// Compute Derive_evidence_cards.
pub fn derive_evidence_cards(references: &ReportReferenceSnapshot) -> Vec<ReportEvidenceCard> {
    let mut cards = Vec::new();
    for (category, items) in [
        ("market", &references.market),
        ("fundamentals", &references.fundamentals),
        ("news", &references.news),
        ("memory", &references.memory),
    ] {
        for item in items.iter().take(8) {
            let claim = if !item.value.trim().is_empty() {
                item.value.clone()
            } else if !item.summary.trim().is_empty() {
                item.summary.clone()
            } else {
                format!("{}: {}", item.key, item.emphasis)
            };
            cards.push(ReportEvidenceCard {
                key: item.key.clone(),
                category: category.to_string(),
                value: item.value.clone(),
                unit: String::new(),
                direction: match item.emphasis.as_str() {
                    "success" => "positive",
                    "warning" => "caution",
                    "primary" => "primary",
                    _ => "neutral",
                }
                .to_string(),
                strength: item.emphasis.clone(),
                source: category.to_string(),
                claim: claim.into(),
            });
        }
    }
    cards
}

/// Compute Derive_news_insights.
pub fn derive_news_insights(
    references: &ReportReferenceSnapshot,
    decision: &DecisionView,
    price_context: &PriceContext,
    diagnostics: &ReportDiagnostics,
    analysis_date: &str,
) -> Vec<NewsInsight> {
    references
        .news
        .iter()
        .filter(|item| item.key == "news_item")
        .take(10)
        .map(|item| {
            let title = item.value.clone();
            let classification = classify_news_insight(item, decision, price_context, diagnostics);
            // Determine if this news item was published on or before the analysis date.
            // This prevents the LLM from describing already-published catalysts as
            // "upcoming" or "about to be released".
            let published_before_analysis = !item.label.trim().is_empty()
                && item.label.trim() <= analysis_date;
            let timing_key = if published_before_analysis {
                "news_timing_published"
            } else {
                "news_timing_pending"
            };
            let raw_summary = if item.summary.trim().is_empty() {
                title.clone()
            } else {
                item.summary.clone()
            };
            NewsInsight {
                title: title.clone(),
                published_at: item.label.clone(),
                source: item.emphasis.clone(),
                url: item.url.clone(),
                fact_summary: LocalText::new(timing_key).with_str("summary", &raw_summary),
                interpretation: classification.interpretation,
                impact_direction: classification.impact_direction.into(),
                impact_strength: classification.impact_strength.into(),
                what_it_confirms: classification.what_it_confirms,
                what_to_watch_next: classification.what_to_watch_next,
                published_before_analysis,
            }
        })
        .collect()
}

struct NewsInsightClassification {
    interpretation: LocalText,
    impact_direction: String,
    impact_strength: String,
    what_it_confirms: LocalText,
    what_to_watch_next: LocalText,
}

fn classify_news_insight(
    item: &ReferenceFactItem,
    decision: &DecisionView,
    price_context: &PriceContext,
    diagnostics: &ReportDiagnostics,
) -> NewsInsightClassification {
    let is_regulatory_source = is_regulatory_reference_source(item);
    let has_complex_disclosure_sequence =
        has_report_diagnostic(&diagnostics.news, "disclosure_sequence_complexity");
    let is_reference_like = is_regulatory_source;

    let interpretation = if has_complex_disclosure_sequence && is_reference_like {
        LocalText::new("news_disclosure_sequence_needs_context")
    } else if is_reference_like {
        LocalText::new("news_reference_only")
    } else if decision.early_probe_allowed {
        LocalText::new("news_supports_active_monitoring")
    } else if !decision.next_upgrade_condition.key.is_empty() {
        LocalText::new("news_needs_price_confirmation_after_catalyst")
    } else if price_context.distance_to_high_pct.is_some()
        || price_context.distance_to_low_pct.is_some()
    {
        LocalText::new("news_requires_follow_up_confirmation")
    } else {
        LocalText::new("news_changes_attention_but_not_thesis")
    };

    let impact_direction = if has_complex_disclosure_sequence && is_reference_like {
        "caution"
    } else if is_reference_like {
        "neutral"
    } else if decision.early_probe_allowed {
        match decision.view {
            DecisionViewDirection::Bearish => "caution",
            DecisionViewDirection::Bullish => "positive",
            DecisionViewDirection::Neutral => "neutral",
        }
    } else {
        "neutral"
    }
    .to_string();

    let impact_strength = if has_complex_disclosure_sequence && is_reference_like {
        "medium"
    } else if decision.early_probe_allowed {
        match decision.confidence_band {
            DecisionConfidenceBand::High => "medium",
            DecisionConfidenceBand::Medium => "medium",
            DecisionConfidenceBand::Low => "low",
        }
    } else {
        "low"
    }
    .to_string();

    let what_it_confirms = news_confirmation_summary(
        decision,
        price_context,
        diagnostics,
        false,
        !is_reference_like,
        is_reference_like,
        has_complex_disclosure_sequence,
    );

    let what_to_watch_next = news_follow_through_summary(
        decision,
        price_context,
        diagnostics,
        false,
        !is_reference_like,
        is_reference_like,
        has_complex_disclosure_sequence,
    );

    NewsInsightClassification {
        interpretation,
        impact_direction,
        impact_strength,
        what_it_confirms,
        what_to_watch_next,
    }
}

fn news_confirmation_summary(
    decision: &DecisionView,
    price_context: &PriceContext,
    diagnostics: &ReportDiagnostics,
    is_risk: bool,
    is_catalyst: bool,
    is_reference_only: bool,
    has_complex_disclosure_sequence: bool,
) -> LocalText {
    if is_risk {
        return if decision.next_downgrade_condition.key.is_empty() {
            LocalText::new("news_confirm_risk_gate")
        } else {
            LocalText::new(&decision.next_downgrade_condition.key)
        };
    }

    if is_reference_only {
        if has_complex_disclosure_sequence {
            return LocalText::new("news_confirm_disclosure_complexity");
        }
        if diagnostics
            .fundamentals
            .iter()
            .any(|item| item.code == "fundamentals_sparse" || item.code == "fundamentals_period_mixed")
        {
            return LocalText::new("news_confirm_fundamentals_sparse");
        }
        return LocalText::new("news_confirm_background_validation");
    }

    if is_catalyst {
        if !decision.next_upgrade_condition.key.is_empty() {
            return LocalText::new(&decision.next_upgrade_condition.key);
        }
        if !decision.primary_path.trim().is_empty() {
            return LocalText::new("news_confirm_primary_path").with_str("path", decision.primary_path.trim());
        }
    }

    if let Some(distance) = price_context.distance_to_high_pct
        && distance <= 3.0
    {
        return LocalText::new("news_confirm_near_high")
            .with_str("high", format_price_reference(price_context.high_price.unwrap_or_default()));
    }

    if !decision.primary_path.trim().is_empty() {
        return LocalText::new("news_confirm_primary_path").with_str("path", decision.primary_path.trim());
    }

    LocalText::new("news_confirm_needs_price_structure")
}

fn news_follow_through_summary(
    decision: &DecisionView,
    price_context: &PriceContext,
    diagnostics: &ReportDiagnostics,
    is_risk: bool,
    is_catalyst: bool,
    is_reference_only: bool,
    has_complex_disclosure_sequence: bool,
) -> LocalText {
    if is_risk {
        return LocalText::new("watch_risk_resolution");
    }

    if has_complex_disclosure_sequence && is_reference_only {
        return LocalText::new("watch_disclosure_overhang_resolution");
    }

    if diagnostics
        .fundamentals
        .iter()
        .any(|item| item.code == "fundamentals_sparse" || item.code == "fundamentals_period_mixed")
    {
        return LocalText::new("watch_fundamental_follow_through");
    }

    if is_reference_only {
        return LocalText::new("watch_fundamental_follow_through");
    }

    if is_catalyst {
        if let Some(distance) = price_context.distance_to_high_pct
            && distance <= 3.0
        {
            return LocalText::new("watch_confirmation_breakout");
        }
        if let Some(distance) = price_context.distance_to_low_pct
            && distance <= 3.0
        {
            return LocalText::new("watch_retest_acceptance");
        }
    }

    if price_context.volume_change_pct.unwrap_or_default() > 10.0 {
        return LocalText::new("watch_price_volume_follow_through");
    }

    news_watch_next_summary(decision)
}

/// Compute News_watch_next_summary.
pub fn news_watch_next_summary(decision: &DecisionView) -> LocalText {
    if decision.early_probe_allowed {
        return LocalText::new("watch_price_volume_follow_through");
    }
    if matches!(decision.action, DecisionAction::WaitRetest) {
        return LocalText::new("watch_retest_acceptance");
    }
    LocalText::new("watch_confirmation_breakout")
}

/// Compute Has_report_diagnostic.
pub fn has_report_diagnostic(items: &[ReportDiagnosticItem], code: &str) -> bool {
    items.iter().any(|item| item.code == code)
}
