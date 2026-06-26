use serde_json::Value;

use super::super::parse;
use super::helpers::{
    extract_object_string_list, extract_object_value, meaningful_value, FieldExtractor,
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
        let ex = FieldExtractor::from_raw(&raw);
        let action = ex.text("action", "Hold");
        let reasoning = ex.text("reasoning", "模型未返回交易推理。");
        let trader_plan = ex.text("trader_plan", "");
        let entry_price = meaningful_value(ex.field("entry_price"))
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("trade_levels")).as_ref(),
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
                    meaningful_value(ex.field("execution_plan")).as_ref(),
                    &[
                        "entry_price",
                        "entry",
                        "entry_zone",
                        "entry_band",
                        "entry_level",
                    ],
                )
            });
        let stop_loss = meaningful_value(ex.field("stop_loss"))
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("trade_levels")).as_ref(),
                    &["stop_loss", "stop", "invalidation_price"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("execution_plan")).as_ref(),
                    &["stop_loss", "stop", "invalidation_price"],
                )
            });
        let confirmation_level = meaningful_value(ex.field("confirmation_level"))
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("trade_levels")).as_ref(),
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
                    meaningful_value(ex.field("execution_plan")).as_ref(),
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
                    meaningful_value(ex.field("trade_levels")).as_ref(),
                    &["conditional_pullback_zone", "pullback_zone", "retest_zone"],
                )
            })
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("execution_plan")).as_ref(),
                    &["conditional_pullback_zone", "pullback_zone", "retest_zone"],
                )
            });
        let target_reference = meaningful_value(ex.field("target_reference"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("trade_levels")).as_ref(),
                    &[
                        "target_reference",
                        "target_zone",
                        "target",
                        "take_profit_zone",
                    ],
                )
                .map(|value| parse::normalize_value(&value))
            })
            .filter(|value| !value.is_empty());
        let target_condition = meaningful_value(ex.field("target_condition"))
            .map(|value| parse::normalize_value(&value))
            .or_else(|| {
                extract_object_value(
                    meaningful_value(ex.field("execution_plan")).as_ref(),
                    &[
                        "target_condition",
                        "target_trigger",
                        "target_validity_condition",
                    ],
                )
                .map(|value| parse::normalize_value(&value))
            })
            .filter(|value| !value.is_empty());
        let time_horizon = meaningful_value(ex.field("time_horizon"))
            .map(|value| parse::normalize_value(&value))
            .filter(|value| !value.is_empty());
        let position_sizing = meaningful_value(ex.field("position_sizing"))
            .map(|value| parse::normalize_value(&value))
            .filter(|value| !value.is_empty());
        let execution_trigger_checklist =
            parse::string_list_or_default(ex.field("execution_trigger_checklist"), &[]);
        let execution_trigger_checklist = if execution_trigger_checklist.is_empty() {
            extract_object_string_list(
                meaningful_value(ex.field("execution_plan")).as_ref(),
                &[
                    "execution_trigger_checklist",
                    "trigger_checklist",
                    "upgrade_triggers",
                    "entry_triggers",
                ],
            )
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
                let blocking_gaps = parse::string_list_or_default(ex.field("blocking_gaps"), &[]);
                if blocking_gaps.is_empty() {
                    extract_object_string_list(
                        meaningful_value(ex.field("execution_plan")).as_ref(),
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
            time_stop_deadline: ex.field("time_stop_deadline")
                .and_then(|v| v.as_str().map(String::from)),
            time_stop_reason: ex.field("time_stop_reason").and_then(|v| v.as_str().map(String::from)),
        };
        if result.trader_plan.trim().is_empty() {
            result.trader_plan = result.rendered_proposal();
        }
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
