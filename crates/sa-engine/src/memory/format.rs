use super::{
    MemoryEntry, StructuredReflection, StructuredRiskAssessment, TradingMemoryLog,
    extract_labeled_block,
};
impl TradingMemoryLog {
    pub(super) fn apply_rotation(&self, blocks: Vec<String>) -> Vec<String> {
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

    pub(super) fn parse_entry(raw: &str) -> Option<MemoryEntry> {
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

    pub(super) fn format_full_entry(entry: &MemoryEntry) -> String {
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

    pub(super) fn format_reflection_only(entry: &MemoryEntry) -> String {
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

    pub(super) fn highlight_from_entry(
        entry: &MemoryEntry,
        same_ticker: bool,
    ) -> sa_models::HistoricalMemoryHighlight {
        sa_models::HistoricalMemoryHighlight {
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
    pub(super) fn format_structured_risk_snapshot(risk: &StructuredRiskAssessment) -> String {
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

    pub(super) fn format_structured_reflection_snapshot(
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

    pub(super) fn sanitize_memory_text(text: &str) -> String {
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

    pub(super) fn humanize_memory_risk(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EmbeddingBackend, RagConfig};
    use sa_models::LocalText;

    fn make_entry(ticker: &str, rating: &str) -> MemoryEntry {
        MemoryEntry {
            ticker: ticker.to_string(),
            trade_date: "2025-01-15".to_string(),
            rating: rating.to_string(),
            action: rating.to_string(),
            summary: "Test summary".to_string(),
            final_trade_decision: "Buy AAPL".to_string(),
            ..Default::default()
        }
    }

    // --- sanitize_memory_text ---

    #[test]
    fn sanitize_removes_known_prefixes() {
        let text = "Good line\nkey_risks: some risk\nAnother good line\nRISK: bad";
        let result = TradingMemoryLog::sanitize_memory_text(text);
        assert!(result.contains("Good line"));
        assert!(result.contains("Another good line"));
        assert!(!result.contains("key_risks"));
        assert!(!result.contains("RISK:"));
    }

    #[test]
    fn sanitize_removes_decision_blocking_gaps() {
        let text = "decision_blocking_gaps: gap1\ndecision_blocking_gaps: gap2";
        let result = TradingMemoryLog::sanitize_memory_text(text);
        assert!(result.is_empty());
    }

    #[test]
    fn sanitize_empty_input() {
        assert_eq!(TradingMemoryLog::sanitize_memory_text(""), "");
    }

    #[test]
    fn sanitize_preserves_normal_text() {
        let text = "Line one\nLine two\nLine three";
        let result = TradingMemoryLog::sanitize_memory_text(text);
        assert_eq!(result, "Line one\nLine two\nLine three");
    }

    #[test]
    fn sanitize_removes_all_prefix_variants() {
        let prefixes = [
            "invalidation_conditions:",
            "key_risks:",
            "offsetting_supports:",
            "overall_risk_framing:",
            "serious_but_manageable_gaps:",
            "tolerable_context_gaps:",
            "Final stance:",
            "Primary risk:",
            "RISK:",
            "SUMMARY:",
            "RATIONALE:",
            "TRIGGERS:",
            "DECISION:",
        ];
        for prefix in &prefixes {
            let text = format!("{} value", prefix);
            let result = TradingMemoryLog::sanitize_memory_text(&text);
            assert!(result.is_empty(), "should remove prefix: {}", prefix);
        }
    }

    // --- humanize_memory_risk ---

    #[test]
    fn humanize_risk_with_known_keys() {
        let text = "key_risks: market volatility\noffsetting_supports: strong fundamentals";
        let result = TradingMemoryLog::humanize_memory_risk(text);
        assert!(result.contains("market volatility"));
        assert!(result.contains("strong fundamentals"));
    }

    #[test]
    fn humanize_risk_no_known_keys() {
        let text = "Some plain risk text";
        let result = TradingMemoryLog::humanize_memory_risk(text);
        assert_eq!(result, "Some plain risk text");
    }

    #[test]
    fn humanize_risk_decision_blocking_gaps() {
        let text = "decision_blocking_gaps: missing earnings data";
        let result = TradingMemoryLog::humanize_memory_risk(text);
        assert!(result.contains("missing earnings data"));
    }

    // --- format_structured_risk_snapshot ---

    #[test]
    fn format_risk_snapshot_all_fields() {
        let risk = StructuredRiskAssessment {
            key_risks: vec!["risk1".into(), "risk2".into()],
            decision_blocking_gaps: vec!["blocker1".into()],
            offsetting_supports: vec!["support1".into()],
            overall_risk_framing: "moderate risk".into(),
            ..Default::default()
        };
        let result = TradingMemoryLog::format_structured_risk_snapshot(&risk);
        assert!(result.contains("Blockers: blocker1"));
        assert!(result.contains("Key Risks: risk1; risk2"));
        assert!(result.contains("Supports: support1"));
        assert!(result.contains("moderate risk"));
    }

    #[test]
    fn format_risk_snapshot_empty() {
        let risk = StructuredRiskAssessment::default();
        let result = TradingMemoryLog::format_structured_risk_snapshot(&risk);
        assert!(result.is_empty());
    }

    // --- format_structured_reflection_snapshot ---

    #[test]
    fn format_reflection_snapshot_all_fields() {
        let reflection = StructuredReflection {
            strengths: LocalText::new("good entry timing"),
            uncertainties: LocalText::new("market direction"),
            next_lessons: LocalText::new("be more patient"),
            ..Default::default()
        };
        let result = TradingMemoryLog::format_structured_reflection_snapshot(&reflection);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("What went right: good entry timing"));
        assert!(text.contains("Greatest uncertainty: market direction"));
        assert!(text.contains("What to improve next time: be more patient"));
    }

    #[test]
    fn format_reflection_snapshot_empty() {
        let reflection = StructuredReflection::default();
        let result = TradingMemoryLog::format_structured_reflection_snapshot(&reflection);
        assert!(result.is_none());
    }

    #[test]
    fn format_reflection_snapshot_partial() {
        let reflection = StructuredReflection {
            strengths: LocalText::new("good"),
            ..Default::default()
        };
        let result = TradingMemoryLog::format_structured_reflection_snapshot(&reflection);
        assert!(result.is_some());
        assert!(result.unwrap().contains("What went right"));
    }

    // --- format_reflection_only ---

    #[test]
    fn format_reflection_only_with_reflection() {
        let mut entry = make_entry("AAPL", "Buy");
        entry.reflection = Some("Learned to wait".into());
        let result = TradingMemoryLog::format_reflection_only(&entry);
        assert!(result.contains("2025-01-15"));
        assert!(result.contains("AAPL"));
        assert!(result.contains("Buy"));
        assert!(result.contains("Learned to wait"));
    }

    #[test]
    fn format_reflection_only_no_reflection_uses_summary() {
        let entry = make_entry("AAPL", "Buy");
        let result = TradingMemoryLog::format_reflection_only(&entry);
        assert!(result.contains("Test summary"));
    }

    #[test]
    fn format_reflection_only_empty_action() {
        let mut entry = make_entry("AAPL", "Buy");
        entry.action = String::new();
        let result = TradingMemoryLog::format_reflection_only(&entry);
        assert!(result.contains("na"));
    }

    // --- apply_rotation ---

    #[test]
    fn apply_rotation_no_rotation_needed() {
        let log = TradingMemoryLog {
            log_path: std::path::PathBuf::new(),
            max_entries: 10,
            vector_store: None,
            rag: RagConfig {
                enabled: false,
                embedding_provider: String::new(),
                embedding_model: String::new(),
                top_k: 0,
                same_ticker_top_k: 0,
                cross_ticker_top_k: 0,
            },
            embedding: EmbeddingBackend {
                provider: String::new(),
                model: String::new(),
                dimension: 0,
                retrieval_enabled: false,
                failure_reason: None,
            },
        };
        let blocks = vec![
            "[2025-01-01 | AAPL | Buy | 5.0%]".to_string(),
            "[2025-01-02 | MSFT | Hold | pending]".to_string(),
        ];
        let result = log.apply_rotation(blocks);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn apply_rotation_drops_old_resolved() {
        let log = TradingMemoryLog {
            log_path: std::path::PathBuf::new(),
            max_entries: 1,
            vector_store: None,
            rag: RagConfig {
                enabled: false,
                embedding_provider: String::new(),
                embedding_model: String::new(),
                top_k: 0,
                same_ticker_top_k: 0,
                cross_ticker_top_k: 0,
            },
            embedding: EmbeddingBackend {
                provider: String::new(),
                model: String::new(),
                dimension: 0,
                retrieval_enabled: false,
                failure_reason: None,
            },
        };
        let blocks = vec![
            "[2025-01-01 | AAPL | Buy | 5.0%]".to_string(),
            "[2025-01-02 | MSFT | Hold | 2.0%]".to_string(),
            "[2025-01-03 | TSLA | Sell | pending]".to_string(),
        ];
        let result = log.apply_rotation(blocks);
        // max_entries=1, 2 resolved entries, so 1 should be dropped
        assert_eq!(result.len(), 2);
        // Pending entry should always be kept
        assert!(result.iter().any(|b| b.contains("pending")));
    }

    #[test]
    fn apply_rotation_zero_max_entries() {
        let log = TradingMemoryLog {
            log_path: std::path::PathBuf::new(),
            max_entries: 0,
            vector_store: None,
            rag: RagConfig {
                enabled: false,
                embedding_provider: String::new(),
                embedding_model: String::new(),
                top_k: 0,
                same_ticker_top_k: 0,
                cross_ticker_top_k: 0,
            },
            embedding: EmbeddingBackend {
                provider: String::new(),
                model: String::new(),
                dimension: 0,
                retrieval_enabled: false,
                failure_reason: None,
            },
        };
        let blocks = vec!["[2025-01-01 | AAPL | Buy | 5.0%]".to_string()];
        let result = log.apply_rotation(blocks);
        assert_eq!(result.len(), 1);
    }

    // --- highlight_from_entry ---

    #[test]
    fn highlight_from_entry_basic() {
        let entry = make_entry("AAPL", "Buy");
        let highlight = TradingMemoryLog::highlight_from_entry(&entry, true);
        assert_eq!(highlight.ticker, "AAPL");
        assert_eq!(highlight.rating, "Buy");
        assert!(highlight.same_ticker);
        assert_eq!(highlight.trade_date, "2025-01-15");
    }

    #[test]
    fn highlight_from_entry_with_risk() {
        let mut entry = make_entry("AAPL", "Buy");
        entry.structured_risk.key_risks = vec!["volatility".into()];
        let highlight = TradingMemoryLog::highlight_from_entry(&entry, false);
        assert_eq!(highlight.key_risk, "volatility");
        assert!(!highlight.same_ticker);
    }

    #[test]
    fn highlight_from_entry_with_reflection() {
        let mut entry = make_entry("AAPL", "Buy");
        entry.structured_reflection = StructuredReflection {
            strengths: LocalText::new("Good timing"),
            ..Default::default()
        };
        let highlight = TradingMemoryLog::highlight_from_entry(&entry, true);
        assert!(highlight.lesson.contains("Good timing"));
    }

    // --- parse_entry ---

    #[test]
    fn parse_entry_resolved() {
        let raw = "[2025-01-15 | AAPL | Buy | 5.0% | 3.0% | 10d]\nMETA:\n{\"rating\":\"Buy\"}\n\nDECISION:\nBuy AAPL\n\nREFLECTION:\nGood trade\n";
        let entry = TradingMemoryLog::parse_entry(raw).unwrap();
        assert_eq!(entry.ticker, "AAPL");
        assert_eq!(entry.rating, "Buy");
        assert!(!entry.pending);
        assert!(entry.raw_return.is_some());
        assert!(entry.alpha_return.is_some());
    }

    #[test]
    fn parse_entry_pending() {
        let raw = "[2025-01-15 | MSFT | Hold | pending]\nDECISION:\nWait\n";
        let entry = TradingMemoryLog::parse_entry(raw).unwrap();
        assert_eq!(entry.ticker, "MSFT");
        assert!(entry.pending);
        assert!(entry.raw_return.is_none());
    }

    #[test]
    fn parse_entry_empty() {
        assert!(TradingMemoryLog::parse_entry("").is_none());
        assert!(TradingMemoryLog::parse_entry("   ").is_none());
    }

    #[test]
    fn parse_entry_invalid_format() {
        assert!(TradingMemoryLog::parse_entry("not a valid entry").is_none());
    }

    #[test]
    fn parse_entry_too_few_fields() {
        assert!(TradingMemoryLog::parse_entry("[2025-01-15 | AAPL]").is_none());
    }
}
