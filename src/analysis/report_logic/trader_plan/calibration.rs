/// Compute Render_calibration_discipline_markdown.
pub fn render_calibration_discipline_markdown(
    report: &StructuredReport,
    memory_context: &MemoryContextSnapshot,
    calibration_memo: &str,
) -> String {
    let mut parts = vec![
        "## Execution Discipline".to_string(),
        format!("- Objective confidence: {}/100", report.confidence_score),
        format!("- Direction score: {}", report.direction_score),
        format!("- Action score: {}", report.action_score),
        format!(
            "- Execution boundary complete: {}",
            if report.execution_readiness.execution_boundary_complete {
                "Yes"
            } else {
                "No"
            }
        ),
        format!(
            "- Threshold tightened: {}",
            if report.calibration_summary.threshold_tightened {
                "Yes"
            } else {
                "No"
            }
        ),
        format!(
            "- Memory threshold tightened: {}",
            if report.calibration_summary.memory_threshold_tightened {
                "Yes"
            } else {
                "No"
            }
        ),
        format!(
            "- Direction threshold penalty: {}",
            report.calibration_summary.direction_threshold_penalty
        ),
        format!(
            "- Setup match quality: {}/{}",
            report.calibration_summary.setup_match_quality.score,
            report.calibration_summary.setup_match_quality.max_score
        ),
        format!(
            "- Setup direction alignment: {}/{}",
            report.calibration_summary.setup_direction_alignment.score,
            report
                .calibration_summary
                .setup_direction_alignment
                .max_score
        ),
        String::new(),
        "## Calibration Constraints".to_string(),
        report.recommendation_calibration_reason.clone(),
    ];

    if !memory_context.setup_tags.is_empty() {
        parts.push(String::new());
        parts.push("## Historical Setup Context".to_string());
        parts.push(format!(
            "- Setup tags: {}",
            memory_context.setup_tags.join(", ")
        ));
        parts.push(format!(
            "- Pending setup samples: {}",
            memory_context.setup_pending_match_count
        ));
        parts.push(format!(
            "- Resolved setup samples: {}",
            memory_context.setup_resolved_match_count
        ));
        parts.push(format!(
            "- Setup hit rate: {:.0}%",
            memory_context.setup_match_hit_rate * 100.0
        ));
        parts.push(format!(
            "- Setup average alpha: {:.1}%",
            memory_context.setup_match_avg_alpha_return * 100.0
        ));
        parts.push(format!(
            "- Direction mix: bullish={}, bearish={}, neutral={}",
            memory_context.setup_long_match_count,
            memory_context.setup_short_match_count,
            memory_context.setup_neutral_match_count
        ));
    }

    if !calibration_memo.trim().is_empty() {
        parts.push(String::new());
        parts.push("## Calibration Memo".to_string());
        parts.push(calibration_memo.trim().to_string());
    }

    parts.join("\n")
}

/// Compute First_non_empty_sentence.
pub fn first_non_empty_sentence(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(|value| {
            value
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or(value)
                .trim()
                .to_string()
        })
}

/// Compute Strip_redundant_prefix.
pub fn strip_redundant_prefix(text: &str, prefixes: &[&str]) -> String {
    let mut current = text.trim();
    for prefix in prefixes {
        if let Some(stripped) = current.strip_prefix(prefix.trim()) {
            current = stripped.trim_start_matches([' ', ':', ';', '。']);
        }
    }
    current.trim().to_string()
}

pub(crate) fn humanize_risk_assessment(text: &str) -> String {
    humanize_structured_risk_assessment(&StructuredRiskAssessment::from_text(text))
}

fn render_risk_assessment_sections(text: &str) -> String {
    render_structured_risk_assessment_sections(&StructuredRiskAssessment::from_text(text))
}

/// Compute Parse_risk_assessment_sections.
pub fn parse_risk_assessment_sections(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut sections = std::collections::BTreeMap::new();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return sections;
    }

    let known_keys = [
        "decision_blocking_gaps",
        "invalidation_conditions",
        "key_risks",
        "offsetting_supports",
        "overall_risk_framing",
        "serious_but_manageable_gaps",
        "tolerable_context_gaps",
    ];
    let mut current_key: Option<String> = None;

    for line in trimmed.lines() {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }
        if let Some((key, value)) = raw.split_once(':')
            && known_keys.contains(&key.trim())
        {
            current_key = Some(key.trim().to_string());
            sections.insert(key.trim().to_string(), value.trim().to_string());
            continue;
        }
        if let Some(key) = current_key.as_ref() {
            let entry = sections.entry(key.clone()).or_default();
            if !entry.is_empty() {
                entry.push(' ');
            }
            entry.push_str(raw);
        }
    }

    sections
}

/// Compute Split_semicolon_items.
pub fn split_semicolon_items(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Compute Normalize_semantic_snippet.
pub fn normalize_semantic_snippet(text: &str) -> String {
    text.to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '\u{4e00}'..='\u{9fff}' => ch,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Compute Is_semantically_similar.
pub fn is_semantically_similar(left: Option<&String>, right: Option<&String>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let left = normalize_semantic_snippet(left);
    let right = normalize_semantic_snippet(right);
    if left.is_empty() || right.is_empty() {
        return false;
    }
    left == right || left.contains(&right) || right.contains(&left)
}

fn normalize_reflection_text(markdown: &str) -> String {
    let trimmed = markdown.trim();
    if trimmed.starts_with('{')
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        if let Some(reflection) = value.get("reflection").and_then(serde_json::Value::as_str) {
            return reflection.trim().to_string();
        }

        let strongest = value
            .get("strongest_part")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let weakest = value
            .get("weakest_uncertainty")
            .or_else(|| value.get("weakest_uncertainty_or_missing_evidence"))
            .or_else(|| value.get("main_uncertainty"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let lesson = value
            .get("next_lesson")
            .or_else(|| value.get("next_lessons"))
            .or_else(|| value.get("next_lesson_for_next_run"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let parts = [strongest, weakest, lesson]
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !parts.is_empty() {
            return parts.join("\n\n");
        }
    }
    trimmed.to_string()
}

impl StructuredReflection {
    /// Compute From_text.
    pub fn from_text(markdown: &str) -> Self {
        let normalized = normalize_reflection_text(markdown);
        let paragraphs = normalized
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let strengths = paragraphs.first().copied().unwrap_or_default().to_string();
        let uncertainties = paragraphs.get(1).copied().unwrap_or_default().to_string();
        let next_lessons = paragraphs.get(2).copied().unwrap_or_default().to_string();
        Self {
            strengths: strengths.into(),
            uncertainties: uncertainties.into(),
            next_lessons: next_lessons.into(),
            raw_reflection: markdown.trim().to_string(),
            markdown: normalized,
        }
    }
}

impl StructuredRiskAssessment {
    /// Compute From_text.
    pub fn from_text(text: &str) -> Self {
        let trimmed = text.trim();
        let sections = parse_risk_assessment_sections(trimmed);
        if sections.is_empty() {
            return Self {
                overall_risk_framing: trimmed.to_string(),
                raw_text: trimmed.to_string(),
                ..Default::default()
            };
        }

        Self {
            decision_blocking_gaps: split_section_items(&sections, "decision_blocking_gaps"),
            key_risks: split_section_items(&sections, "key_risks"),
            offsetting_supports: split_section_items(&sections, "offsetting_supports"),
            invalidation_conditions: split_section_items(&sections, "invalidation_conditions"),
            overall_risk_framing: sections
                .get("overall_risk_framing")
                .map(|value| value.trim().to_string())
                .unwrap_or_default(),
            serious_but_manageable_gaps: split_section_items(&sections, "serious_but_manageable_gaps"),
            tolerable_context_gaps: split_section_items(&sections, "tolerable_context_gaps"),
            raw_text: trimmed.to_string(),
        }
    }
}
