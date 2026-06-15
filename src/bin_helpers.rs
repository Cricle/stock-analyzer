//! Shared initialization helpers for CLI and MCP binaries.

use std::sync::Arc;

use crate::data::MarketDataClient;
use crate::engine::llm::LlmClient;
use crate::engine::guidance::{GuidanceMemory, GuidanceMemoryBundle};

/// No-op memory implementation for standalone CLI/MCP usage.
pub struct NoopMemory;

#[async_trait::async_trait]
impl GuidanceMemory for NoopMemory {
    async fn past_context_bundle(
        &self,
        _query: &str,
        _same_ticker_limit: usize,
        _cross_ticker_limit: usize,
    ) -> GuidanceMemoryBundle {
        GuidanceMemoryBundle::default()
    }
}

/// Build a MarketDataClient from environment.
pub async fn build_market_data_client() -> anyhow::Result<MarketDataClient> {
    Ok(MarketDataClient::new().await)
}

/// Build an LlmClient from environment variables:
/// - LLM_BASE_URL (required)
/// - LLM_API_KEY (required)
/// - LLM_MODEL (default: "claude-sonnet-4-20250514")
/// - LLM_PROVIDER (default: "openai", also supports "anthropic")
/// - LLM_TIMEOUT_SECS (default: 120)
pub fn build_llm_client() -> anyhow::Result<LlmClient> {
    let base_url = std::env::var("LLM_BASE_URL")
        .map_err(|_| anyhow::anyhow!("LLM_BASE_URL not set"))?;
    let api_key = std::env::var("LLM_API_KEY")
        .map_err(|_| anyhow::anyhow!("LLM_API_KEY not set"))?;
    let model = std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    let provider = std::env::var("LLM_PROVIDER")
        .unwrap_or_else(|_| "openai".to_string());
    let timeout_secs: u64 = std::env::var("LLM_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    match provider.as_str() {
        "anthropic" => Ok(LlmClient::anthropic(
            &base_url,
            &api_key,
            &model,
            timeout_secs,
        )),
        _ => Ok(LlmClient::openai_compatible(
            &base_url,
            &api_key,
            &model,
            timeout_secs,
        )),
    }
}

/// Build a no-op memory for standalone usage.
pub fn build_memory() -> Arc<dyn GuidanceMemory> {
    Arc::new(NoopMemory)
}

/// Resolve i18n keys in a JSON value tree.
///
/// For each object:
/// - `i18n_key` fields: resolved with sibling params, output goes to `text` field.
/// - `*_key` / `*_keys` fields: resolved and populate the base field
///   (e.g. `thesis_key` → `thesis`, `catalyst_keys` → `catalysts`).
///   The original base field is skipped if already resolved from a key field.
pub fn resolve_output(value: serde_json::Value, i18n: &crate::i18n::I18n, lang: &str) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            // Two-pass: collect sibling params first, then resolve i18n_key with them.
            let params: serde_json::Map<String, serde_json::Value> = map.iter()
                .filter(|(k, _)| k.as_str() != "i18n_key")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            // Track base fields resolved from *_key(s) so the original field doesn't overwrite them.
            let mut resolved_bases: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut resolved = serde_json::Map::new();
            for (k, v) in map {
                if k == "i18n_key" {
                    if let Some(key) = v.as_str() {
                        if let Some(text) = i18n.resolve_with_params(key, lang, &params) {
                            resolved.insert("text".to_string(), serde_json::json!(text));
                        } else if let Some(text) = i18n.resolve(key, lang) {
                            resolved.insert("text".to_string(), serde_json::json!(text));
                        }
                        resolved.insert("key".to_string(), serde_json::json!(key));
                    }
                } else if let Some(base) = key_base(&k) {
                    // Handle `*_key` / `*_keys` fields: resolve and populate the base field.
                    match &v {
                        serde_json::Value::String(key) => {
                            if let Some(text) = resolve_key_string(key, i18n, lang) {
                                resolved.insert(base.to_string(), serde_json::json!(text));
                                resolved_bases.insert(base.to_string());
                            }
                            resolved.insert(k, v);
                        }
                        serde_json::Value::Object(obj) => {
                            if let Some(key) = obj.get("i18n_key").and_then(|v| v.as_str()) {
                                let obj_params = obj.iter()
                                    .filter(|(name, _)| *name != "i18n_key")
                                    .map(|(name, val)| (name.clone(), val.clone()))
                                    .collect::<serde_json::Map<String, serde_json::Value>>();
                                if let Some(text) = i18n.resolve_with_params(key, lang, &obj_params) {
                                    resolved.insert(base.to_string(), serde_json::json!(text));
                                    resolved_bases.insert(base.to_string());
                                }
                            }
                            resolved.insert(k, resolve_output(v, i18n, lang));
                        }
                        serde_json::Value::Array(arr) => {
                            // Array of i18n keys — items can be strings or objects with i18n_key + params.
                            let texts: Vec<String> = arr.iter()
                                .filter_map(|item| match item {
                                    serde_json::Value::String(s) => i18n.resolve(s, lang),
                                    serde_json::Value::Object(obj) => {
                                        let key = obj.get("i18n_key").and_then(|v| v.as_str())?;
                                        let obj_params: serde_json::Map<String, serde_json::Value> = obj.iter()
                                            .filter(|(name, _)| *name != "i18n_key")
                                            .map(|(name, val)| (name.clone(), val.clone()))
                                            .collect();
                                        i18n.resolve_with_params(key, lang, &obj_params)
                                    }
                                    _ => None,
                                })
                                .collect();
                            if !texts.is_empty() {
                                resolved.insert(base.to_string(), serde_json::json!(texts));
                                resolved_bases.insert(base.to_string());
                            }
                            // Also resolve each array item individually (adds text field to objects).
                            let resolved_arr: Vec<serde_json::Value> = arr.iter()
                                .map(|item| resolve_output(item.clone(), i18n, lang))
                                .collect();
                            resolved.insert(k, serde_json::Value::Array(resolved_arr));
                        }
                        _ => {
                            resolved.insert(k, v);
                        }
                    }
                } else if resolved_bases.contains(&k) {
                    // Skip: already resolved from a *_key(s) field (i18n takes precedence).
                } else {
                    resolved.insert(k, resolve_output(v, i18n, lang));
                }
            }
            serde_json::Value::Object(resolved)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(|v| resolve_output(v, i18n, lang)).collect())
        }
        other => other,
    }
}

/// Resolve an i18n key string that may be a simple key, a multi-key joined by
/// `；` (U+FF1B), or a pipe-separated composite `"main_key|consensus=N|detail_items"`.
///
/// Returns the resolved text, or `None` if no resolution succeeded.
fn resolve_key_string(key: &str, i18n: &crate::i18n::I18n, lang: &str) -> Option<String> {
    // 1. Try direct resolve (simple single key).
    if let Some(text) = i18n.resolve(key, lang) {
        return Some(text);
    }
    // 2. Multi-key format: "key1；key2；key3" — split, resolve each, join.
    if key.contains('\u{FF1B}') {
        let parts: Vec<String> = key
            .split('\u{FF1B}')
            .filter_map(|k| i18n.resolve(k.trim(), lang))
            .collect();
        if !parts.is_empty() {
            return Some(parts.join("\u{FF1B}"));
        }
    }
    // 3. Pipe-separated composite format.
    //    - 3-part: "main_key|param1=val1|detail_key1:v1 detail_key2:v2"
    //    - 2-part: "main_key|detail_key1:v1 detail_key2:v2"
    if key.contains('|') {
        let parts: Vec<&str> = key.splitn(3, '|').collect();
        let main_key = parts[0].trim();
        let mut params = serde_json::Map::new();
        let detail_part = if parts.len() == 3 {
            // Middle part contains key=value params.
            for kv in parts[1].split('|') {
                if let Some((pk, pv)) = kv.split_once('=') {
                    if let Ok(num) = pv.trim().parse::<f64>() {
                        params.insert(pk.trim().to_string(), serde_json::json!(num));
                    } else {
                        params.insert(pk.trim().to_string(), serde_json::json!(pv.trim()));
                    }
                }
            }
            parts[2]
        } else {
            parts.get(1).copied().unwrap_or("")
        };
        // Resolve detail items (each is "i18n_key:value" or plain key).
        let detail_texts: Vec<String> = detail_part
            .split_whitespace()
            .filter_map(|item| {
                if let Some((dk, dv)) = item.split_once(':') {
                    i18n
                        .resolve(dk.trim(), lang)
                        .map(|text| format!("{}:{}", text, dv))
                } else {
                    i18n.resolve(item.trim(), lang)
                }
            })
            .collect();
        if !detail_texts.is_empty() {
            params.insert(
                "detail".to_string(),
                serde_json::json!(detail_texts.join(" ")),
            );
        }
        // Try resolve_with_params first, fall back to plain resolve.
        if let Some(text) = i18n.resolve_with_params(main_key, lang, &params) {
            return Some(text);
        }
        if let Some(text) = i18n.resolve(main_key, lang) {
            return Some(text);
        }
    }
    None
}

/// Extract the base field name from a `*_key` or `*_keys` field name.
/// Returns `Some("catalysts")` for `"catalyst_keys"`, `Some("thesis")` for `"thesis_key"`, etc.
///
/// The mapping is: `X_key` → `X`, `X_keys` → `Xs` (plural).
fn key_base(field: &str) -> Option<String> {
    if let Some(base) = field.strip_suffix("_keys") && !base.is_empty() {
        return Some(format!("{base}s"));
    }
    if let Some(base) = field.strip_suffix("_key") && !base.is_empty() {
        return Some(base.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;
    use serde_json::json;

    #[test]
    fn key_base_variants() {
        assert_eq!(key_base("catalyst_keys").as_deref(), Some("catalysts"));
        assert_eq!(key_base("risk_keys").as_deref(), Some("risks"));
        assert_eq!(key_base("evidence_point_keys").as_deref(), Some("evidence_points"));
        assert_eq!(key_base("thesis_key").as_deref(), Some("thesis"));
        assert_eq!(key_base("headline_key").as_deref(), Some("headline"));
        // Non-key fields return None.
        assert_eq!(key_base("catalysts"), None);
        assert_eq!(key_base("symbol"), None);
        assert_eq!(key_base("_key"), None);
        assert_eq!(key_base("_keys"), None);
    }

    #[test]
    fn resolve_output_catalyst_keys_overwrite_catalysts() {
        let i18n = I18n::new();
        let input = json!({
            "symbol": "600519",
            "catalysts": ["No catalyst returned"],
            "catalyst_keys": [
                {"i18n_key": "stock_pick.catalyst.rsi_bullish", "rsi": 51.28}
            ]
        });
        let result = resolve_output(input, &i18n, "zh");
        let cats = result.get("catalysts").unwrap().as_array().unwrap();
        assert_eq!(cats.len(), 1, "catalysts should have 1 item from catalyst_keys");
        assert!(
            cats[0].as_str().unwrap().contains("RSI"),
            "catalysts[0] should be i18n-resolved, got: {}",
            cats[0]
        );
    }

    #[test]
    fn resolve_output_risk_keys_overwrite_risks() {
        let i18n = I18n::new();
        let input = json!({
            "symbol": "600519",
            "risks": ["Some LLM risk"],
            "risk_keys": [
                {"i18n_key": "stock_pick.risk.high_pe", "pe": 35.0}
            ]
        });
        let result = resolve_output(input, &i18n, "zh");
        let risks = result.get("risks").unwrap().as_array().unwrap();
        assert_eq!(risks.len(), 1, "risks should have 1 item from risk_keys");
        assert!(
            risks[0].as_str().unwrap().contains("PE"),
            "risks[0] should be i18n-resolved, got: {}",
            risks[0]
        );
    }

    #[test]
    fn resolve_output_nested_object_with_i18n_key() {
        let i18n = I18n::new();
        let input = json!({
            "objective_assessment": {
                "headline_key": {
                    "i18n_key": "stock_pick.headline.high_quality"
                }
            }
        });
        let result = resolve_output(input, &i18n, "zh");
        let headline = result.pointer("/objective_assessment/headline").unwrap().as_str().unwrap();
        assert_eq!(headline, "高质量候选，证据覆盖充分。");
    }

    #[test]
    fn resolve_output_thesis_key_with_params() {
        let i18n = I18n::new();
        let input = json!({
            "thesis_key": {
                "i18n_key": "stock_pick.thesis.market_context",
                "name": "贵州茅台",
                "symbol": "600519",
                "market": "上交所",
                "industry": "白酒"
            }
        });
        let result = resolve_output(input, &i18n, "zh");
        let thesis = result.get("thesis").unwrap().as_str().unwrap();
        assert!(thesis.contains("贵州茅台"), "thesis should contain stock name, got: {thesis}");
        assert!(thesis.contains("600519"), "thesis should contain symbol, got: {thesis}");
    }

    #[test]
    fn resolve_output_en_lang() {
        let i18n = I18n::new();
        let input = json!({
            "catalysts": ["No catalyst returned"],
            "catalyst_keys": [
                {"i18n_key": "stock_pick.catalyst.rsi_bullish", "rsi": 55.0}
            ]
        });
        let result = resolve_output(input, &i18n, "en");
        let cats = result.get("catalysts").unwrap().as_array().unwrap();
        assert_eq!(cats.len(), 1);
        assert!(
            cats[0].as_str().unwrap().contains("RSI"),
            "should be English, got: {}",
            cats[0]
        );
    }

    #[test]
    fn resolve_key_string_simple() {
        let i18n = I18n::new();
        let text = resolve_key_string("score.fundamental.negative_pe", &i18n, "zh");
        assert_eq!(text.as_deref(), Some("PE为负，盈利承压"));
    }

    #[test]
    fn resolve_key_string_multi_key() {
        let i18n = I18n::new();
        let key = "score.fundamental.negative_pe\u{FF1B}score.fundamental.roe_loss";
        let text = resolve_key_string(key, &i18n, "zh");
        assert!(text.is_some(), "multi-key should resolve");
        let t = text.unwrap();
        assert!(t.contains("PE为负"), "should contain negative PE, got: {t}");
        assert!(t.contains("亏损"), "should contain loss, got: {t}");
        assert!(t.contains('\u{FF1B}'), "should keep separator, got: {t}");
    }

    #[test]
    fn resolve_key_string_pipe_separated_with_params() {
        let i18n = I18n::new();
        let key = "score.llm_analysis.consensus|consensus=75|score.llm_analysis.signal_llm:75 score.llm_analysis.signal_technical:60";
        let text = resolve_key_string(key, &i18n, "zh");
        assert!(text.is_some(), "pipe-separated should resolve");
        let t = text.unwrap();
        assert!(t.contains("75"), "should contain consensus value, got: {t}");
        assert!(t.contains("LLM"), "should contain LLM signal name, got: {t}");
        assert!(t.contains("技术"), "should contain tech signal name, got: {t}");
    }

    #[test]
    fn resolve_key_string_pipe_separated_two_part() {
        let i18n = I18n::new();
        // 2-part format: main_key|detail_items
        let key = "score.llm_analysis.consensus|score.llm_analysis.signal_llm:80";
        let text = resolve_key_string(key, &i18n, "zh");
        assert!(text.is_some(), "2-part pipe should resolve");
    }

    #[test]
    fn resolve_output_reason_key_multi() {
        let i18n = I18n::new();
        let input = json!({
            "fundamental": {
                "score": 50,
                "reason": "PE为负，盈利承压；ROE 亏损",
                "reason_key": "score.fundamental.negative_pe\u{FF1B}score.fundamental.roe_loss"
            }
        });
        let result = resolve_output(input, &i18n, "zh");
        let reason = result.pointer("/fundamental/reason").unwrap().as_str().unwrap();
        assert!(reason.contains("PE为负"), "reason should be i18n-resolved, got: {reason}");
        assert!(reason.contains("亏损"), "reason should contain roe_loss text, got: {reason}");
    }

    #[test]
    fn resolve_output_reason_key_pipe_separated() {
        let i18n = I18n::new();
        let input = json!({
            "llm_analysis": {
                "score": 65,
                "reason": "some fallback",
                "reason_key": "score.llm_analysis.consensus|consensus=80|score.llm_analysis.signal_llm:75 score.llm_analysis.signal_technical:65"
            }
        });
        let result = resolve_output(input, &i18n, "zh");
        let reason = result.pointer("/llm_analysis/reason").unwrap().as_str().unwrap();
        assert!(reason.contains("80"), "reason should have consensus value, got: {reason}");
        assert!(reason.contains("LLM"), "reason should have signal name, got: {reason}");
    }

    #[test]
    fn resolve_key_string_unknown_key_returns_none() {
        let i18n = I18n::new();
        let text = resolve_key_string("nonexistent.key.here", &i18n, "zh");
        assert!(text.is_none(), "unknown key should return None");
    }
}
