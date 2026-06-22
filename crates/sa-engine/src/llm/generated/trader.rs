use serde_json::Value;

use super::super::parse;
use super::helpers::{
    extract_entry_price_from_texts, extract_numbered_trigger_lines, extract_object_string_list,
    extract_object_value, extract_position_sizing_from_texts, extract_price_target_from_texts,
    extract_stop_loss_from_texts, extract_time_horizon_from_texts, format_price_like_text,
    meaningful_value, object_value,
};
use super::types::GeneratedTraderDecision;

impl GeneratedTraderDecision {
    fn entry_price_number(&self) -> Option<f64> {
        self.entry_price.as_ref().and_then(parse::normalize_numeric)
    }

    fn stop_loss_number(&self) -> Option<f64> {
        self.stop_loss.as_ref().and_then(parse::normalize_numeric)
    }

    pub fn rendered_proposal(&self) -> String {
        let mut parts = vec![
            "# Trader Execution Plan".to_string(),
            String::new(),
            "## Proposed Action".to_string(),
            format!("**Action**: {}", self.action),
            String::new(),
            "## Execution Logic".to_string(),
            format!("**Reasoning**: {}", self.reasoning),
        ];

        let mut level_lines = Vec::new();
        if let Some(value) = self.entry_price_number() {
            level_lines.push(format!("**Entry Price**: {value}"));
        }
        if let Some(value) = self.stop_loss_number() {
            level_lines.push(format!("**Stop Loss**: {value}"));
        }
        if let Some(value) = self
            .position_sizing
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            level_lines.push(format!("**Position Sizing**: {value}"));
        }
        if let Some(value) = self
            .confirmation_level
            .as_ref()
            .map(parse::normalize_value)
            .filter(|value| !value.trim().is_empty())
        {
            level_lines.push(format!("**Confirmation Level**: {value}"));
        }
        if let Some(value) = self
            .target_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            level_lines.push(format!("**Target Reference**: {value}"));
        }
        if let Some(value) = self
            .target_condition
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            level_lines.push(format!("**Target Condition**: {value}"));
        }
        if let Some(value) = self
            .time_horizon
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            level_lines.push(format!("**Time Horizon**: {value}"));
        }
        if !level_lines.is_empty() {
            parts.push(String::new());
            parts.push("## Trade Levels".to_string());
            parts.extend(level_lines);
        }
        parts.extend([
            String::new(),
            "## Final Transaction Proposal".to_string(),
            format!("**Action**: {}", self.action),
        ]);
        parts.join("\n")
    }

    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let action = parse::text_or_default(field("action"), "Hold");
        let reasoning = parse::text_or_default(field("reasoning"), "模型未返回交易推理。");
        let trader_plan = parse::text_or_default(field("trader_plan"), "");
        let entry_price = meaningful_value(field("entry_price"))
            .or_else(|| {
                extract_object_value(
                    object_value(field("trade_levels")).as_ref(),
                    &[
                        "entry_price",
                        "entry",
                        "entry_zone",
                        "entry_band",
                        "entry_level",
                    ],
                )
            })
            .or_else(|| {
                extract_object_value(
                    object_value(field("execution_plan")).as_ref(),
                    &[
                        "entry_price",
                        "entry",
                        "entry_zone",
                        "entry_band",
                        "entry_level",
                    ],
                )
            })
            .or_else(|| {
                extract_entry_price_from_texts(&[&reasoning, &trader_plan]).map(Value::from)
            });
        let stop_loss = meaningful_value(field("stop_loss"))
            .or_else(|| {
                extract_object_value(
                    object_value(field("trade_levels")).as_ref(),
                    &["stop_loss", "stop", "invalidation_price"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    object_value(field("execution_plan")).as_ref(),
                    &["stop_loss", "stop", "invalidation_price"],
                )
            })
            .or_else(|| extract_stop_loss_from_texts(&[&reasoning, &trader_plan]).map(Value::from));
        let confirmation_level = meaningful_value(field("confirmation_level"))
            .or_else(|| {
                extract_object_value(
                    object_value(field("trade_levels")).as_ref(),
                    &[
                        "confirmation_level",
                        "confirmation",
                        "breakout_level",
                        "conditional_breakout",
                    ],
                )
            })
            .or_else(|| {
                extract_object_value(
                    object_value(field("execution_plan")).as_ref(),
                    &[
                        "confirmation_level",
                        "confirmation",
                        "breakout_level",
                        "conditional_breakout",
                    ],
                )
            });
        let entry_price = entry_price
            .or_else(|| {
                extract_object_value(
                    object_value(field("trade_levels")).as_ref(),
                    &["conditional_pullback_zone", "pullback_zone", "retest_zone"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    object_value(field("execution_plan")).as_ref(),
                    &["conditional_pullback_zone", "pullback_zone", "retest_zone"],
                )
            });
        let target_reference = meaningful_value(field("target_reference"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                extract_object_value(
                    object_value(field("trade_levels")).as_ref(),
                    &[
                        "target_reference",
                        "target_zone",
                        "target",
                        "take_profit_zone",
                    ],
                )
                .map(|value| parse::normalize_value(&value))
            })
            .or_else(|| {
                extract_price_target_from_texts(&[&reasoning, &trader_plan])
                    .map(format_price_like_text)
            })
            .filter(|value| !value.is_empty());
        let target_condition = meaningful_value(field("target_condition"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                extract_object_value(
                    object_value(field("execution_plan")).as_ref(),
                    &[
                        "target_condition",
                        "target_trigger",
                        "target_validity_condition",
                    ],
                )
                .map(|value| parse::normalize_value(&value))
            })
            .filter(|value| !value.is_empty());
        let time_horizon = meaningful_value(field("time_horizon"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| extract_time_horizon_from_texts(&[&reasoning, &trader_plan]))
            .filter(|value| !value.is_empty());
        let position_sizing = meaningful_value(field("position_sizing"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| extract_position_sizing_from_texts(&[&reasoning, &trader_plan]))
            .filter(|value| !value.is_empty());
        let execution_trigger_checklist =
            parse::string_list_or_default(field("execution_trigger_checklist"), &[]);
        let execution_trigger_checklist = if execution_trigger_checklist.is_empty() {
            let object_triggers = extract_object_string_list(
                object_value(field("execution_plan")).as_ref(),
                &[
                    "execution_trigger_checklist",
                    "trigger_checklist",
                    "upgrade_triggers",
                    "entry_triggers",
                ],
            );
            if object_triggers.is_empty() {
                extract_numbered_trigger_lines(&reasoning)
            } else {
                object_triggers
            }
        } else {
            execution_trigger_checklist
        };

        let mut result = Self {
            action,
            reasoning,
            trader_plan,
            entry_price,
            stop_loss,
            confirmation_level,
            target_reference,
            target_condition,
            time_horizon,
            position_sizing,
            execution_trigger_checklist,
            blocking_gaps: {
                let blocking_gaps = parse::string_list_or_default(field("blocking_gaps"), &[]);
                if blocking_gaps.is_empty() {
                    extract_object_string_list(
                        object_value(field("execution_plan")).as_ref(),
                        &[
                            "blocking_gaps",
                            "blocking_conditions",
                            "missing_proof_points",
                        ],
                    )
                } else {
                    blocking_gaps
                }
            },
            time_stop_deadline: field("time_stop_deadline")
                .and_then(|v| v.as_str().map(String::from)),
            time_stop_reason: field("time_stop_reason").and_then(|v| v.as_str().map(String::from)),
        };
        if result.trader_plan.trim().is_empty() {
            result.trader_plan = result.rendered_proposal();
        }
        // Dedup: if entry_price and confirmation_level are the same numeric value,
        // clear confirmation_level so rebuild_confirmation_level() derives a
        // different value from other sources (thesis text, anchors, etc.).
        if let (Some(entry), Some(confirm)) = (
            result
                .entry_price
                .as_ref()
                .and_then(parse::normalize_numeric),
            result
                .confirmation_level
                .as_ref()
                .and_then(parse::normalize_numeric),
        ) && entry > 0.0
            && confirm > 0.0
            && (entry - confirm).abs() / entry.max(confirm) < 0.005
        {
            result.confirmation_level = None;
        }
        result
    }
}
