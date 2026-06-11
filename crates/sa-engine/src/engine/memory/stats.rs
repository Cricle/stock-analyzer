//! Memory statistics and calibration. Inlined from backend/src/engine/memory/stats/.

use serde::Deserialize;
use serde_json::json;

use super::MemoryEntry;
use crate::models::{StructuredReflection, StructuredRiskAssessment, CalibrationProfile};

#[derive(Clone, Debug, Default)]
pub struct SetupMatchStats {
    pub total_match_count: usize,
    pub pending_match_count: usize,
    pub resolved_match_count: usize,
    pub used_fallback: bool,
    pub calibration_sample_count: usize,
    pub hit_rate: f64,
    pub avg_alpha_return: f64,
    pub long_match_count: usize,
    pub short_match_count: usize,
    pub neutral_match_count: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QdrantMemoryPayload {
    pub(crate) ticker: String,
    pub(crate) trade_date: String,
    pub(crate) rating: String,
    pub(crate) action: Option<String>,
    pub(crate) market: Option<String>,
    pub(crate) stock_name: Option<String>,
    pub(crate) direction_score: Option<i32>,
    pub(crate) confidence_score: Option<i32>,
    pub(crate) action_score: Option<i32>,
    pub(crate) summary: Option<String>,
    pub(crate) risk_assessment: Option<String>,
    pub(crate) rationale: Option<String>,
    pub(crate) structured_risk: Option<StructuredRiskAssessment>,
    pub(crate) structured_reflection: Option<StructuredReflection>,
    pub(crate) trigger_checklist: Option<Vec<String>>,
    pub(crate) blocking_gaps: Option<Vec<String>>,
    pub(crate) setup_tags: Option<Vec<String>>,
    pub(crate) execution_boundary_complete: Option<bool>,
    pub(crate) final_trade_decision: Option<String>,
    pub(crate) reflection: Option<String>,
    pub(crate) raw_return: Option<f64>,
    pub(crate) alpha_return: Option<f64>,
    pub(crate) holding_days: Option<usize>,
    pub(crate) pending: Option<bool>,
    #[serde(default)]
    pub(crate) user_id: Option<String>,
}

pub(crate) fn extract_labeled_block<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let marker = format!("{label}:\n");
    let start = text.find(&marker)? + marker.len();
    let rest = &text[start..];
    let mut end = rest.len();
    for next_label in ["META", "DECISION", "REFLECTION"] {
        if next_label == label {
            continue;
        }
        let next_marker = format!("\n{next_label}:\n");
        if let Some(index) = rest.find(&next_marker) {
            end = end.min(index);
        }
    }
    Some(&rest[..end])
}

pub(crate) fn summarize_entries(entries: &[MemoryEntry]) -> serde_json::Value {
    if entries.is_empty() {
        return json!({
            "count": 0,
            "avg_raw_return": 0.0,
            "avg_alpha_return": 0.0,
            "hit_rate": 0.0,
            "positive_rate": 0.0,
            "alpha_positive_rate": 0.0,
        });
    }

    let mut raw_sum = 0.0;
    let mut alpha_sum = 0.0;
    let mut hits = 0usize;
    let mut positive = 0usize;
    let mut alpha_positive = 0usize;

    for entry in entries {
        let raw = entry.raw_return.unwrap_or_default();
        let alpha = entry.alpha_return.unwrap_or_default();
        raw_sum += raw;
        alpha_sum += alpha;
        if raw > 0.0 {
            positive += 1;
        }
        if alpha > 0.0 {
            alpha_positive += 1;
        }
        if realized_call_hit(entry.rating.as_str(), raw, alpha) {
            hits += 1;
        }
    }

    let count = entries.len() as f64;
    json!({
        "count": entries.len(),
        "avg_raw_return": raw_sum / count,
        "avg_alpha_return": alpha_sum / count,
        "hit_rate": hits as f64 / count,
        "positive_rate": positive as f64 / count,
        "alpha_positive_rate": alpha_positive as f64 / count,
    })
}

pub(crate) fn group_summary<F>(entries: &[MemoryEntry], key_fn: F) -> serde_json::Value
where
    F: Fn(&MemoryEntry) -> String,
{
    let mut grouped = std::collections::BTreeMap::<String, Vec<MemoryEntry>>::new();
    for entry in entries {
        grouped
            .entry(key_fn(entry))
            .or_default()
            .push(entry.clone());
    }

    let mut output = serde_json::Map::new();
    for (key, value) in grouped {
        output.insert(key, summarize_entries(&value));
    }
    serde_json::Value::Object(output)
}

pub(crate) fn realized_call_hit(rating: &str, raw_return: f64, alpha_return: f64) -> bool {
    match rating {
        "Buy" => raw_return > 0.0 && alpha_return > 0.0,
        "Overweight" => alpha_return > 0.0,
        "Hold" => raw_return.abs() <= 0.05 || alpha_return.abs() <= 0.03,
        "Underweight" => alpha_return < 0.0,
        "Sell" => raw_return < 0.0 && alpha_return < 0.0,
        _ => false,
    }
}

pub(crate) fn bucket_score(score: Option<i32>, cutoffs: &[i32]) -> String {
    let Some(score) = score else {
        return "unknown".to_string();
    };
    if score < cutoffs[0] {
        format!("<{}", cutoffs[0])
    } else if score < cutoffs[1] {
        format!("{}-{}", cutoffs[0], cutoffs[1] - 1)
    } else if score < cutoffs[2] {
        format!("{}-{}", cutoffs[1], cutoffs[2] - 1)
    } else if score < cutoffs[3] {
        format!("{}-{}", cutoffs[2], cutoffs[3] - 1)
    } else {
        format!(">={}", cutoffs[3])
    }
}

pub(crate) fn bucket_signed_score(score: Option<i32>, cutoffs: &[i32]) -> String {
    let Some(score) = score else {
        return "unknown".to_string();
    };
    if score < cutoffs[0] {
        format!("<{}", cutoffs[0])
    } else if score < cutoffs[1] {
        format!("{}..{}", cutoffs[0], cutoffs[1] - 1)
    } else if score < cutoffs[2] {
        format!("{}..{}", cutoffs[1], cutoffs[2] - 1)
    } else if score < cutoffs[3] {
        format!("{}..{}", cutoffs[2], cutoffs[3] - 1)
    } else {
        format!(">={}", cutoffs[3])
    }
}

pub(crate) fn suggested_calibration_profile(entries: &[MemoryEntry]) -> serde_json::Value {
    let profile = derive_calibration_profile(entries);
    json!({
        "sample_count": profile.sample_count,
        "min_confidence_score": profile.min_confidence_score,
        "min_action_score": profile.min_action_score,
        "direction_floor_abs": profile.direction_floor_abs,
        "strong_direction_abs": profile.strong_direction_abs,
        "is_default_profile": profile.sample_count < 12,
    })
}

pub(crate) fn derive_calibration_profile(entries: &[MemoryEntry]) -> CalibrationProfile {
    if entries.len() < 12 {
        return CalibrationProfile::default();
    }

    let sample_count = entries.len();
    let hit_count = entries
        .iter()
        .filter(|entry| {
            realized_call_hit(
                entry.rating.as_str(),
                entry.raw_return.unwrap_or_default(),
                entry.alpha_return.unwrap_or_default(),
            )
        })
        .count();
    let avg_alpha_return = entries
        .iter()
        .map(|entry| entry.alpha_return.unwrap_or_default())
        .sum::<f64>()
        / sample_count as f64;
    let min_hit_rate = hit_count as f64 / sample_count as f64;

    let mut best_profile = CalibrationProfile::default();
    let mut best_score = f64::MIN;

    for min_confidence in [50, 55, 60, 65, 70] {
        for min_action in [40, 45, 50, 55, 60] {
            for direction_floor in [10, 15, 20, 25, 30] {
                for strong_direction in [45, 50, 55, 60, 65] {
                    if strong_direction < direction_floor {
                        continue;
                    }
                    let score = evaluate_profile_candidate(
                        entries,
                        min_confidence,
                        min_action,
                        direction_floor,
                        strong_direction,
                    );
                    if score > best_score {
                        best_score = score;
                        best_profile = CalibrationProfile {
                            min_confidence_score: min_confidence,
                            min_action_score: min_action,
                            direction_floor_abs: direction_floor,
                            strong_direction_abs: strong_direction,
                            sample_count,
                            min_hit_rate,
                            min_avg_alpha_return: avg_alpha_return,
                        };
                    }
                }
            }
        }
    }

    best_profile
}

pub(crate) fn evaluate_profile_candidate(
    entries: &[MemoryEntry],
    min_confidence_score: i32,
    min_action_score: i32,
    direction_floor_abs: i32,
    strong_direction_abs: i32,
) -> f64 {
    let mut hits = 0.0;
    let mut alpha_sum = 0.0;
    let mut coverage = 0.0;

    for entry in entries {
        let direction = entry.direction_score.unwrap_or_default();
        let confidence = entry.confidence_score.unwrap_or_default();
        let action = entry.action_score.unwrap_or_default();
        let alpha = entry.alpha_return.unwrap_or_default();
        let raw = entry.raw_return.unwrap_or_default();

        let rating = if confidence < min_confidence_score || action < min_action_score {
            "Hold"
        } else if direction.abs() >= strong_direction_abs {
            if direction > 0 { "Buy" } else { "Sell" }
        } else if direction.abs() >= direction_floor_abs {
            if direction > 0 {
                "Overweight"
            } else {
                "Underweight"
            }
        } else {
            "Hold"
        };

        if rating != "Hold" {
            coverage += 1.0;
        }
        if realized_call_hit(rating, raw, alpha) {
            hits += 1.0;
        }
        alpha_sum += alpha;
    }

    let count = entries.len() as f64;
    let hit_rate = hits / count;
    let avg_alpha = alpha_sum / count;
    let coverage_rate = coverage / count;

    hit_rate * 0.55 + avg_alpha * 4.0 + coverage_rate * 0.15
}

#[derive(Clone, Debug)]
pub struct MemoryOutcomeUpdate {
    pub ticker: String,
    pub trade_date: String,
    pub outcome_return: f64,
    pub benchmark_return: f64,
    pub holding_days: usize,
    pub reflection: String,
}

use super::{MemoryQuery, TradingMemoryLog};

impl TradingMemoryLog {
    pub(crate) fn build_stats_from_resolved_entries(entries: &[MemoryEntry]) -> SetupMatchStats {
        if entries.is_empty() {
            return SetupMatchStats::default();
        }

        let resolved_match_count = entries.len();
        let long_match_count = entries
            .iter()
            .filter(|entry| {
                entry.rating.trim().eq_ignore_ascii_case("buy")
                    || entry.rating.trim().eq_ignore_ascii_case("overweight")
                    || entry.action.trim().eq_ignore_ascii_case("buy")
            })
            .count();
        let short_match_count = entries
            .iter()
            .filter(|entry| {
                entry.rating.trim().eq_ignore_ascii_case("sell")
                    || entry.rating.trim().eq_ignore_ascii_case("underweight")
                    || entry.action.trim().eq_ignore_ascii_case("sell")
            })
            .count();
        let neutral_match_count =
            resolved_match_count.saturating_sub(long_match_count + short_match_count);
        let hit_count = entries
            .iter()
            .filter(|entry| entry.alpha_return.unwrap_or_default() > 0.0)
            .count();
        let avg_alpha_return = entries
            .iter()
            .map(|entry| entry.alpha_return.unwrap_or_default())
            .sum::<f64>()
            / resolved_match_count as f64;
        let hit_rate = hit_count as f64 / resolved_match_count as f64;

        SetupMatchStats {
            total_match_count: resolved_match_count,
            pending_match_count: 0,
            resolved_match_count,
            used_fallback: false,
            calibration_sample_count: resolved_match_count,
            hit_rate,
            avg_alpha_return,
            long_match_count,
            short_match_count,
            neutral_match_count,
        }
    }

    pub(crate) async fn fallback_resolved_match_stats(
        &self,
        query: &MemoryQuery,
    ) -> anyhow::Result<SetupMatchStats> {
        let resolved_entries = self
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| !entry.pending)
            .filter(|entry| entry.alpha_return.is_some() && entry.raw_return.is_some())
            .filter(|entry| entry.ticker.eq_ignore_ascii_case(&query.ticker))
            .collect::<Vec<_>>();
        if !resolved_entries.is_empty() {
            let mut stats = Self::build_stats_from_resolved_entries(&resolved_entries);
            stats.used_fallback = true;
            return Ok(stats);
        }

        let same_market_entries = self
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| !entry.pending)
            .filter(|entry| entry.alpha_return.is_some() && entry.raw_return.is_some())
            .filter(|entry| {
                !query.market.trim().is_empty()
                    && entry
                        .market
                        .trim()
                        .eq_ignore_ascii_case(query.market.trim())
            })
            .collect::<Vec<_>>();
        let mut stats = Self::build_stats_from_resolved_entries(&same_market_entries);
        if stats.total_match_count > 0 {
            stats.used_fallback = true;
        }
        Ok(stats)
    }

    pub async fn effective_setup_match_stats(
        &self,
        query: &MemoryQuery,
    ) -> anyhow::Result<SetupMatchStats> {
        let mut stats = self.setup_match_stats(query).await?;
        if query.setup_tags.is_empty() {
            return Ok(stats);
        }
        if stats.total_match_count > 0 {
            stats.calibration_sample_count = stats.resolved_match_count;
            return Ok(stats);
        }
        let fallback_stats = self.fallback_resolved_match_stats(query).await?;
        if fallback_stats.total_match_count > 0 {
            if stats.pending_match_count > 0 {
                stats.used_fallback = true;
                stats.calibration_sample_count = fallback_stats.resolved_match_count;
                stats.resolved_match_count = fallback_stats.resolved_match_count;
                stats.hit_rate = fallback_stats.hit_rate;
                stats.avg_alpha_return = fallback_stats.avg_alpha_return;
                stats.long_match_count = fallback_stats.long_match_count;
                stats.short_match_count = fallback_stats.short_match_count;
                stats.neutral_match_count = fallback_stats.neutral_match_count;
            } else {
                stats = fallback_stats;
            }
        }
        Ok(stats)
    }
}
