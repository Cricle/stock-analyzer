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
