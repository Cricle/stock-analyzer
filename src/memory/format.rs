use super::{
    MemoryEntry, StructuredReflection, StructuredRiskAssessment, TradingMemoryLog,
    extract_labeled_block,
};
impl TradingMemoryLog {
    pub fn apply_rotation(&self, blocks: Vec<String>) -> Vec<String> {
        if self.max_entries == 0 {
            return blocks;
        }

        let resolved_count = blocks
            .iter()
            .filter(|block| {
                let line = block.lines().next().unwrap_or_default().trim();
                line.starts_with('[') && line.ends_with(']') && !line.ends_with("| pending]")
            })
            .count();
        if resolved_count <= self.max_entries {
            return blocks;
        }

        let mut to_drop = resolved_count - self.max_entries;
        let mut kept = Vec::new();
        for block in blocks {
            let line = block.lines().next().unwrap_or_default().trim();
            let is_resolved =
                line.starts_with('[') && line.ends_with(']') && !line.ends_with("| pending]");
            if is_resolved && to_drop > 0 {
                to_drop -= 1;
                continue;
            }
            kept.push(block);
        }
        kept
    }

    pub fn parse_entry(raw: &str) -> Option<MemoryEntry> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lines = trimmed.lines().collect::<Vec<_>>();
        let tag_line = lines.first()?.trim();
        if !tag_line.starts_with('[') || !tag_line.ends_with(']') {
            return None;
        }
        let fields = tag_line
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split('|')
            .map(|item| item.trim().to_string())
            .collect::<Vec<_>>();
        if fields.len() < 4 {
            return None;
        }

        let meta = extract_labeled_block(trimmed, "META")
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text.trim()).ok());
        let final_trade_decision = extract_labeled_block(trimmed, "DECISION")
            .map(|text| text.trim().to_string())
            .unwrap_or_default();
        let reflection =
            extract_labeled_block(trimmed, "REFLECTION").map(|text| text.trim().to_string());

        let pending = fields
            .last()
            .map(|value| value == "pending")
            .unwrap_or(false);
        let (raw_return, alpha_return, holding_days) = if pending || fields.len() < 6 {
            (None, None, None)
        } else {
            (
                fields
                    .get(3)
                    .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
                    .map(|value| value / 100.0),
                fields
                    .get(4)
                    .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
                    .map(|value| value / 100.0),
                fields
                    .get(5)
                    .and_then(|value| value.trim_end_matches('d').parse::<usize>().ok()),
            )
        };

        Some(MemoryEntry {
            ticker: fields.get(1)?.to_string(),
            trade_date: fields.first()?.to_string(),
            rating: meta
                .as_ref()
                .and_then(|value| value["rating"].as_str())
                .unwrap_or_else(|| fields.get(2).map(|item| item.as_str()).unwrap_or("Hold"))
                .to_string(),
            action: meta
                .as_ref()
                .and_then(|value| value["action"].as_str())
                .unwrap_or_default()
                .to_string(),
            market: meta
                .as_ref()
                .and_then(|value| value["market"].as_str())
                .unwrap_or_default()
                .to_string(),
            stock_name: meta
                .as_ref()
                .and_then(|value| value["stock_name"].as_str())
                .unwrap_or_default()
                .to_string(),
            direction_score: meta
                .as_ref()
                .and_then(|value| value["direction_score"].as_i64())
                .map(|value| value as i32),
            confidence_score: meta
                .as_ref()
                .and_then(|value| value["confidence_score"].as_i64())
                .map(|value| value as i32),
            action_score: meta
                .as_ref()
                .and_then(|value| value["action_score"].as_i64())
                .map(|value| value as i32),
            summary: meta
                .as_ref()
                .and_then(|value| value["summary"].as_str())
                .unwrap_or_default()
                .to_string(),
            risk_assessment: meta
                .as_ref()
                .and_then(|value| value["risk_assessment"].as_str())
                .unwrap_or_default()
                .to_string(),
            rationale: meta
                .as_ref()
                .and_then(|value| value["rationale"].as_str())
                .unwrap_or_default()
                .to_string(),
            structured_risk: meta
                .as_ref()
                .and_then(|value| value.get("structured_risk").cloned())
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_else(|| {
                    StructuredRiskAssessment::from_text(
                        meta.as_ref()
                            .and_then(|value| value["risk_assessment"].as_str())
                            .unwrap_or_default(),
                    )
                }),
            structured_reflection: meta
                .as_ref()
                .and_then(|value| value.get("structured_reflection").cloned())
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_else(|| {
                    StructuredReflection::from_text(reflection.as_deref().unwrap_or_default())
                }),
            trigger_checklist: meta
                .as_ref()
                .and_then(|value| value["trigger_checklist"].as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            blocking_gaps: meta
                .as_ref()
                .and_then(|value| value["blocking_gaps"].as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            setup_tags: meta
                .as_ref()
                .and_then(|value| value["setup_tags"].as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            execution_boundary_complete: meta
                .as_ref()
                .and_then(|value| value["execution_boundary_complete"].as_bool()),
            final_trade_decision,
            reflection,
            raw_return,
            alpha_return,
            holding_days,
            pending,
            user_id: meta
                .as_ref()
                .and_then(|v| v["user_id"].as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    pub fn format_full_entry(entry: &MemoryEntry) -> String {
        let decision_body = if !entry.summary.trim().is_empty() {
            let mut lines = vec![format!(
                "Summary:\n{}",
                Self::sanitize_memory_text(&entry.summary)
            )];
            if !entry.structured_risk.key_risks.is_empty()
                || !entry.structured_risk.decision_blocking_gaps.is_empty()
                || !entry.structured_risk.offsetting_supports.is_empty()
            {
                lines.push(format!(
                    "Key Risks:\n{}",
                    Self::sanitize_memory_text(&Self::format_structured_risk_snapshot(
                        &entry.structured_risk
                    ))
                ));
            } else if !entry.risk_assessment.trim().is_empty() {
                lines.push(format!(
                    "Key Risks:\n{}",
                    Self::humanize_memory_risk(&entry.risk_assessment)
                ));
            }
            if !entry.rationale.trim().is_empty() {
                lines.push(format!(
                    "Decision Basis:\n{}",
                    Self::sanitize_memory_text(&entry.rationale)
                ));
            }
            if !entry.trigger_checklist.is_empty() {
                lines.push(format!(
                    "Review Triggers:\n- {}",
                    entry
                        .trigger_checklist
                        .iter()
                        .map(|item| Self::sanitize_memory_text(item))
                        .collect::<Vec<_>>()
                        .join("\n- ")
                ));
            }
            lines.join("\n\n")
        } else {
            Self::sanitize_memory_text(&entry.final_trade_decision)
        };
        format!(
            "- [{} | {} | {} | {}]\n{}\n\nReview:\n{}",
            entry.trade_date,
            entry.ticker,
            entry.rating,
            if entry.action.is_empty() {
                "na"
            } else {
                &entry.action
            },
            decision_body,
            Self::format_structured_reflection_snapshot(&entry.structured_reflection)
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    entry
                        .reflection
                        .clone()
                        .map(|value| Self::sanitize_memory_text(&value))
                })
                .unwrap_or_else(|| "No reflections yet".to_string())
        )
    }

    pub fn format_reflection_only(entry: &MemoryEntry) -> String {
        let lesson = Self::format_structured_reflection_snapshot(&entry.structured_reflection)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| entry.reflection.clone())
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                if !entry.summary.trim().is_empty() {
                    Some(Self::sanitize_memory_text(&entry.summary))
                } else if !entry.rationale.trim().is_empty() {
                    Some(Self::sanitize_memory_text(&entry.rationale))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "No reflections yet".to_string());
        format!(
            "- [{} | {} | {} | {}]\n{}",
            entry.trade_date,
            entry.ticker,
            entry.rating,
            if entry.action.is_empty() {
                "na"
            } else {
                &entry.action
            },
            lesson
        )
    }

    pub fn highlight_from_entry(
        entry: &MemoryEntry,
        same_ticker: bool,
    ) -> crate::HistoricalMemoryHighlight {
        crate::HistoricalMemoryHighlight {
            trade_date: entry.trade_date.clone(),
            ticker: entry.ticker.clone(),
            rating: entry.rating.clone(),
            action: entry.action.clone(),
            summary: Self::sanitize_memory_text(&entry.summary)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
            key_risk: entry
                .structured_risk
                .key_risks
                .first()
                .cloned()
                .or_else(|| {
                    entry
                        .structured_risk
                        .decision_blocking_gaps
                        .first()
                        .cloned()
                })
                .unwrap_or_default(),
            lesson: Self::format_structured_reflection_snapshot(&entry.structured_reflection)
                .and_then(|text| text.lines().next().map(str::trim).map(ToString::to_string))
                .or_else(|| {
                    entry.reflection.as_ref().and_then(|text| {
                        text.lines().next().map(str::trim).map(ToString::to_string)
                    })
                })
                .unwrap_or_default(),
            same_ticker,
        }
    }
}
impl TradingMemoryLog {
    pub fn format_structured_risk_snapshot(risk: &StructuredRiskAssessment) -> String {
        let mut parts = Vec::new();
        if !risk.decision_blocking_gaps.is_empty() {
            parts.push(format!(
                "Blockers: {}",
                risk.decision_blocking_gaps.join("; ")
            ));
        }
        if !risk.key_risks.is_empty() {
            parts.push(format!("Key Risks: {}", risk.key_risks.join("; ")));
        }
        if !risk.offsetting_supports.is_empty() {
            parts.push(format!("Supports: {}", risk.offsetting_supports.join("; ")));
        }
        if !risk.overall_risk_framing.trim().is_empty() {
            parts.push(risk.overall_risk_framing.trim().to_string());
        }
        parts.join("\n")
    }

    pub fn format_structured_reflection_snapshot(
        reflection: &StructuredReflection,
    ) -> Option<String> {
        let mut parts = Vec::new();
        if !reflection.strengths.trim().is_empty() {
            parts.push(format!("What went right: {}", reflection.strengths.trim()));
        }
        if !reflection.uncertainties.trim().is_empty() {
            parts.push(format!(
                "Greatest uncertainty: {}",
                reflection.uncertainties.trim()
            ));
        }
        if !reflection.next_lessons.trim().is_empty() {
            parts.push(format!(
                "What to improve next time: {}",
                reflection.next_lessons.trim()
            ));
        }
        (!parts.is_empty()).then(|| parts.join("\n"))
    }

    pub fn sanitize_memory_text(text: &str) -> String {
        text.lines()
            .map(str::trim)
            .filter(|line| {
                !line.is_empty()
                    && !line.starts_with("decision_blocking_gaps:")
                    && !line.starts_with("invalidation_conditions:")
                    && !line.starts_with("key_risks:")
                    && !line.starts_with("offsetting_supports:")
                    && !line.starts_with("overall_risk_framing:")
                    && !line.starts_with("serious_but_manageable_gaps:")
                    && !line.starts_with("tolerable_context_gaps:")
                    && !line.starts_with("Final stance:")
                    && !line.starts_with("Primary risk:")
                    && !line.starts_with("RISK:")
                    && !line.starts_with("SUMMARY:")
                    && !line.starts_with("RATIONALE:")
                    && !line.starts_with("TRIGGERS:")
                    && !line.starts_with("DECISION:")
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    }

    pub fn humanize_memory_risk(text: &str) -> String {
        let mut parts = Vec::new();
        let normalized = text.replace('\n', " ");
        for key in [
            "decision_blocking_gaps:",
            "key_risks:",
            "offsetting_supports:",
        ] {
            if let Some(index) = normalized.find(key) {
                let tail = &normalized[index + key.len()..];
                let item = tail.split(['\n']).next().unwrap_or(tail).trim();
                if !item.is_empty() {
                    parts.push(item.to_string());
                }
            }
        }
        if parts.is_empty() {
            Self::sanitize_memory_text(text)
        } else {
            parts.join("\n- ").replacen("", "- ", 1)
        }
    }
}
