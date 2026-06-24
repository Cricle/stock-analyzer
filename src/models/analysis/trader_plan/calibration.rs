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

fn first_non_empty_sentence(candidates: &[&str]) -> Option<String> {
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

fn strip_redundant_prefix(text: &str, prefixes: &[&str]) -> String {
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

fn parse_risk_assessment_sections(text: &str) -> std::collections::BTreeMap<String, String> {
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

fn split_semicolon_items(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn normalize_semantic_snippet(text: &str) -> String {
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

pub(crate) fn is_semantically_similar(left: Option<&String>, right: Option<&String>) -> bool {
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

#[cfg(test)]
mod calibration_logic_tests {
    use super::super::*;

    #[test]
    fn first_non_empty_sentence_basic() {
        let result = first_non_empty_sentence(&["", "hello world", "other"]);
        assert_eq!(result, Some("hello world".into()));
    }

    #[test]
    fn first_non_empty_sentence_all_empty() {
        let result = first_non_empty_sentence(&["", "  "]);
        assert!(result.is_none());
    }

    #[test]
    fn first_non_empty_sentence_multiline() {
        let result = first_non_empty_sentence(&["line1\nline2"]);
        assert_eq!(result, Some("line1".into()));
    }

    #[test]
    fn strip_redundant_prefix_basic() {
        let result = strip_redundant_prefix("Recommendation: Buy", &["Recommendation"]);
        assert_eq!(result, "Buy");
    }

    #[test]
    fn strip_redundant_prefix_no_match() {
        let result = strip_redundant_prefix("Buy", &["Recommendation"]);
        assert_eq!(result, "Buy");
    }

    #[test]
    fn strip_redundant_prefix_multiple() {
        let result = strip_redundant_prefix("Action: Recommendation: Buy", &["Action:", "Recommendation"]);
        assert_eq!(result, "Buy");
    }

    #[test]
    fn split_semicolon_items_basic() {
        let result = split_semicolon_items("a; b; c");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn split_semicolon_items_empty() {
        let result = split_semicolon_items("");
        assert!(result.is_empty());
    }

    #[test]
    fn split_semicolon_items_whitespace() {
        let result = split_semicolon_items("  a  ;  b  ");
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn normalize_semantic_snippet_basic() {
        let result = normalize_semantic_snippet("Hello World!");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn normalize_semantic_snippet_chinese() {
        let result = normalize_semantic_snippet("你好世界");
        assert_eq!(result, "你好世界");
    }

    #[test]
    fn normalize_semantic_snippet_mixed() {
        let result = normalize_semantic_snippet("AAPL股价上涨");
        assert_eq!(result, "aapl股价上涨");
    }

    #[test]
    fn is_semantically_similar_same() {
        let left = Some(&"hello world".to_string());
        let right = Some(&"hello world".to_string());
        assert!(is_semantically_similar(left, right));
    }

    #[test]
    fn is_semantically_similar_subset() {
        let left = Some(&"hello world foo".to_string());
        let right = Some(&"hello world".to_string());
        assert!(is_semantically_similar(left, right));
    }

    #[test]
    fn is_semantically_similar_different() {
        let left = Some(&"hello".to_string());
        let right = Some(&"world".to_string());
        assert!(!is_semantically_similar(left, right));
    }

    #[test]
    fn is_semantically_similar_none() {
        assert!(!is_semantically_similar(None, None));
    }

    #[test]
    fn parse_risk_assessment_sections_empty() {
        let sections = parse_risk_assessment_sections("");
        assert!(sections.is_empty());
    }

    #[test]
    fn parse_risk_assessment_sections_basic() {
        let text = "key_risks: market volatility\noffsetting_supports: strong fundamentals";
        let sections = parse_risk_assessment_sections(text);
        assert!(sections.contains_key("key_risks"));
        assert!(sections.contains_key("offsetting_supports"));
    }

    #[test]
    fn structured_risk_assessment_from_text_empty() {
        let assessment = StructuredRiskAssessment::from_text("");
        assert!(assessment.overall_risk_framing.is_empty());
    }

    #[test]
    fn structured_risk_assessment_from_text_plain() {
        let assessment = StructuredRiskAssessment::from_text("market risk is moderate");
        assert_eq!(assessment.overall_risk_framing, "market risk is moderate");
        assert!(assessment.key_risks.is_empty());
    }

    #[test]
    fn structured_risk_assessment_from_text_structured() {
        let text = "overall_risk_framing: moderate risk\nkey_risks: volatility; liquidity";
        let assessment = StructuredRiskAssessment::from_text(text);
        assert_eq!(assessment.overall_risk_framing, "moderate risk");
        assert_eq!(assessment.key_risks.len(), 2);
    }
}

