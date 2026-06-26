impl GeneratedPortfolioDecision {
    fn price_target_number(&self) -> Option<f64> {
        self.price_target
            .as_ref()
            .and_then(parse::normalize_numeric)
    }

    pub fn rendered_decision(&self) -> String {
        let mut parts = vec![
            "# Portfolio Manager Decision".to_string(),
            String::new(),
            "## Final Rating".to_string(),
            format!("**Rating**: {}", self.rating),
            format!("**Confidence**: {}", self.confidence_string()),
            format!("**Risk Assessment**: {}", self.risk_assessment),
            String::new(),
            "## Executive Summary".to_string(),
            format!("**Executive Summary**: {}", self.executive_summary),
            String::new(),
            "## Investment Thesis".to_string(),
            format!("**Investment Thesis**: {}", self.investment_thesis),
        ];
        if !self.rationale.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Why This Call Won".to_string(),
                format!("**Rationale**: {}", self.rationale),
            ]);
        }
        if let Some(value) = self.price_target_number() {
            parts.extend([
                String::new(),
                "## Price Target".to_string(),
                format!("**Price Target**: {value}"),
            ]);
        }
        if let Some(value) = self
            .confirmation_level
            .as_ref()
            .map(parse::normalize_value)
            .filter(|value| !value.trim().is_empty())
        {
            parts.extend([
                String::new(),
                "## Confirmation Level".to_string(),
                format!("**Confirmation Level**: {value}"),
            ]);
        }
        if let Some(value) = self
            .invalidation_level
            .as_ref()
            .map(parse::normalize_value)
            .filter(|value| !value.trim().is_empty())
        {
            parts.extend([
                String::new(),
                "## Invalidation Level".to_string(),
                format!("**Invalidation Level**: {value}"),
            ]);
        }
        if let Some(value) = self
            .target_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.extend([
                String::new(),
                "## Target Reference".to_string(),
                format!("**Target Reference**: {value}"),
            ]);
        }
        if let Some(value) = self
            .target_condition
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.extend([
                String::new(),
                "## Target Condition".to_string(),
                format!("**Target Condition**: {value}"),
            ]);
        }
        if let Some(value) = self
            .time_horizon
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            parts.extend([
                String::new(),
                "## Time Horizon".to_string(),
                format!("**Time Horizon**: {value}"),
            ]);
        }
        parts.join("\n")
    }

    pub fn rendered_reflection(&self) -> String {
        self.reflection
            .as_ref()
            .map(GeneratedReflection::rendered)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "strongest_part": "Final decision was generated from the portfolio manager evidence packet.",
                    "weakest_uncertainty_or_missing_evidence": "No separate reflection was provided by the model.",
                    "next_lesson_for_next_run": "Keep the next run focused on the highest-impact missing confirmation."
                })
                .to_string()
            })
    }

    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let risk_assessment_raw = meaningful_value(field("risk_assessment"));
        let rating = parse::first_non_empty(
            &[field("rating").as_ref(), field("recommendation").as_ref()],
            "Hold",
        );
        let executive_summary = parse::first_non_empty(
            &[
                field("executive_summary").as_ref(),
                field("summary").as_ref(),
            ],
            "模型未返回组合经理执行摘要。",
        );
        let investment_thesis = parse::text_or_default(
            field("investment_thesis").or_else(|| field("portfolio_decision")),
            "模型未返回组合经理投资逻辑。",
        );
        let rationale = parse::text_or_default(field("rationale"), investment_thesis.as_str());
        let risk_assessment =
            parse::text_or_default(risk_assessment_raw.clone(), "模型未返回风险评估。");
        let trigger_checklist = parse::string_list_or_default(field("trigger_checklist"), &[]);
        let trigger_checklist = if trigger_checklist.is_empty() {
            extract_object_string_list(
                risk_assessment_raw.as_ref(),
                &[
                    "trigger_checklist",
                    "upgrade_trigger_checklist",
                    "action_trigger_checklist",
                ],
            )
        } else {
            trigger_checklist
        };
        let inferred_price_target = meaningful_value(field("price_target"));
        let inferred_confirmation_level = meaningful_value(field("confirmation_level"))
            .or_else(|| {
                extract_object_value(
                    risk_assessment_raw.as_ref(),
                    &["confirmation_level", "confirmation", "trigger_level"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    meaningful_value(field("trade_levels")).as_ref(),
                    &["confirmation_level", "confirmation", "breakout_level"],
                )
            });
        let inferred_invalidation_level = meaningful_value(field("invalidation_level"))
            .or_else(|| {
                extract_object_value(
                    risk_assessment_raw.as_ref(),
                    &["invalidation_level", "invalidation_price", "stop_loss"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    meaningful_value(field("trade_levels")).as_ref(),
                    &["invalidation_level", "invalidation_price", "stop_loss"],
                )
            });
        let inferred_target_reference = meaningful_value(field("target_reference"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                meaningful_value(field("price_target")).map(|value| parse::normalize_value(&value))
            })
            .or_else(|| {
                inferred_price_target
                    .as_ref()
                    .map(parse::normalize_value)
                    .filter(|value| !value.trim().is_empty())
            })
            .filter(|value| !value.trim().is_empty());
        let inferred_target_condition = meaningful_value(field("target_condition"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                extract_object_value(
                    risk_assessment_raw.as_ref(),
                    &[
                        "target_condition",
                        "target_trigger",
                        "target_validity_condition",
                    ],
                )
                .map(|value| parse::normalize_value(&value))
            })
            .filter(|value| !value.trim().is_empty());
        let inferred_time_horizon = meaningful_value(field("time_horizon"))
            .map(|value| parse::normalize_value(&value))
            .map(|value| {
                let first_line = value.lines().next().unwrap_or("").trim();
                let compact = first_line
                    .trim_start_matches("time_horizon:")
                    .trim_start_matches("Time Horizon:")
                    .trim();
                compact.to_string()
            })
            .filter(|value| !value.is_empty());
        Self {
            rating,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            risk_assessment,
            summary: executive_summary.clone(),
            rationale,
            executive_summary,
            investment_thesis,
            price_target: inferred_price_target,
            confirmation_level: inferred_confirmation_level,
            invalidation_level: inferred_invalidation_level,
            target_reference: inferred_target_reference,
            target_condition: inferred_target_condition,
            time_horizon: inferred_time_horizon,
            missing_evidence_ladder: GeneratedMissingEvidenceLadder::from_value(
                meaningful_value(field("missing_evidence_ladder")).or_else(|| {
                    extract_object_value(
                        risk_assessment_raw.as_ref(),
                        &[
                            "missing_evidence_ladder",
                            "missing_evidence",
                            "missing_evidence_classification",
                            "missing_evidence_severity_ladder",
                        ],
                    )
                }),
            ),
            trigger_checklist,
            scenario_paths: {
                let raw = field("scenario_paths")
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                raw.into_iter()
                    .filter_map(|item| {
                        let obj = item.as_object()?;
                        Some(GeneratedScenarioPath {
                            key: obj.get("key").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            name: obj.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            trigger: obj.get("trigger").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            action: obj.get("action").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            risk_boundary: obj.get("risk_boundary").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            position_sizing: obj.get("position_sizing").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            stop_level: obj.get("stop_level").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        })
                    })
                    .collect()
            },
            time_stop_deadline: field("time_stop_deadline").and_then(|v| v.as_str().map(String::from)),
            time_stop_reason: field("time_stop_reason").and_then(|v| v.as_str().map(String::from)),
            reflection: meaningful_value(field("reflection")).map(GeneratedReflection::from_value),
        }
    }
}
