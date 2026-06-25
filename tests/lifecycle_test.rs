use sa::report::lifecycle::{
    bounded_user_notes, build_user_context, build_user_context_prompt, normalize_language,
    normalize_option,
};
use sa::{AnalysisParameters, AnalysisUserContext};

// ---- normalize_language ----

#[test]
fn normalize_language_returns_zh_cn() {
    assert_eq!(normalize_language(Some("en")), "zh-CN");
}

#[test]
fn normalize_language_none() {
    assert_eq!(normalize_language(None), "zh-CN");
}

#[test]
fn normalize_language_empty() {
    assert_eq!(normalize_language(Some("")), "zh-CN");
}

// ---- normalize_option ----

#[test]
fn normalize_option_matches_allowed() {
    let allowed = &["buy", "sell", "hold"];
    assert_eq!(normalize_option(Some("buy"), allowed, "hold"), "buy");
}

#[test]
fn normalize_option_case_insensitive() {
    let allowed = &["buy", "sell", "hold"];
    assert_eq!(normalize_option(Some("BUY"), allowed, "hold"), "buy");
}

#[test]
fn normalize_option_not_in_allowed_returns_default() {
    let allowed = &["buy", "sell", "hold"];
    assert_eq!(normalize_option(Some("unknown"), allowed, "hold"), "hold");
}

#[test]
fn normalize_option_none_returns_default() {
    let allowed = &["low", "medium", "high"];
    assert_eq!(normalize_option(None, allowed, "medium"), "medium");
}

#[test]
fn normalize_option_empty_string_returns_default() {
    let allowed = &["short_term", "swing", "position"];
    assert_eq!(normalize_option(Some(""), allowed, "swing"), "swing");
}

#[test]
fn normalize_option_whitespace_trimmed() {
    let allowed = &["not_holding", "holding"];
    assert_eq!(
        normalize_option(Some("  holding  "), allowed, "not_holding"),
        "holding"
    );
}

#[test]
fn normalize_option_empty_allowed_list() {
    let allowed: &[&str] = &[];
    assert_eq!(
        normalize_option(Some("anything"), allowed, "default"),
        "default"
    );
}

// ---- bounded_user_notes ----

#[test]
fn bounded_user_notes_normal_text() {
    assert_eq!(bounded_user_notes(Some("hello world")), "hello world");
}

#[test]
fn bounded_user_notes_collapses_whitespace() {
    assert_eq!(bounded_user_notes(Some("hello   world")), "hello world");
}

#[test]
fn bounded_user_notes_none() {
    assert_eq!(bounded_user_notes(None), "");
}

#[test]
fn bounded_user_notes_empty() {
    assert_eq!(bounded_user_notes(Some("")), "");
}

#[test]
fn bounded_user_notes_truncates_at_600() {
    let long_text = "a".repeat(700);
    let result = bounded_user_notes(Some(&long_text));
    assert_eq!(result.len(), 600);
}

#[test]
fn bounded_user_notes_preserves_short_text() {
    let text = "short note";
    assert_eq!(bounded_user_notes(Some(text)), text);
}

#[test]
fn bounded_user_notes_newlines_become_spaces() {
    assert_eq!(bounded_user_notes(Some("line1\nline2")), "line1 line2");
}

#[test]
fn bounded_user_notes_tabs_become_spaces() {
    assert_eq!(bounded_user_notes(Some("a\tb")), "a b");
}

// ---- build_user_context ----

#[test]
fn build_user_context_defaults() {
    let params = AnalysisParameters::default();
    let ctx = build_user_context(&params);
    assert_eq!(ctx.language, "zh-CN");
    assert_eq!(ctx.position_state, "not_holding");
    assert_eq!(ctx.workflow_intent, "stock_picking");
    assert_eq!(ctx.risk_preference, "medium");
    assert_eq!(ctx.investment_horizon, "swing");
    assert!(ctx.holding_cost.is_none());
    assert!(ctx.holding_ratio_pct.is_none());
    assert!(ctx.notes.is_empty());
}

#[test]
fn build_user_context_with_values() {
    let params = AnalysisParameters {
        language: Some("en".to_string()),
        user_position_state: Some("holding".to_string()),
        workflow_intent: Some("holding_review".to_string()),
        holding_cost: Some(150.5),
        holding_ratio_pct: Some(30.0),
        risk_preference: Some("high".to_string()),
        investment_horizon: Some("position".to_string()),
        user_notes: Some("test notes".to_string()),
        ..Default::default()
    };
    let ctx = build_user_context(&params);
    assert_eq!(ctx.language, "zh-CN"); // always zh-CN
    assert_eq!(ctx.position_state, "holding");
    assert_eq!(ctx.workflow_intent, "holding_review");
    assert_eq!(ctx.risk_preference, "high");
    assert_eq!(ctx.investment_horizon, "position");
    assert_eq!(ctx.holding_cost, Some(150.5));
    assert_eq!(ctx.holding_ratio_pct, Some(30.0));
    assert_eq!(ctx.notes, "test notes");
}

#[test]
fn build_user_context_filters_invalid_holding_cost() {
    let params = AnalysisParameters {
        holding_cost: Some(-1.0),
        ..Default::default()
    };
    let ctx = build_user_context(&params);
    assert!(ctx.holding_cost.is_none());
}

#[test]
fn build_user_context_filters_nan_holding_cost() {
    let params = AnalysisParameters {
        holding_cost: Some(f64::NAN),
        ..Default::default()
    };
    let ctx = build_user_context(&params);
    assert!(ctx.holding_cost.is_none());
}

#[test]
fn build_user_context_filters_negative_ratio() {
    let params = AnalysisParameters {
        holding_ratio_pct: Some(-5.0),
        ..Default::default()
    };
    let ctx = build_user_context(&params);
    assert!(ctx.holding_ratio_pct.is_none());
}

// ---- build_user_context_prompt ----

#[test]
fn build_user_context_prompt_minimal() {
    let ctx = AnalysisUserContext {
        language: "zh-CN".into(),
        position_state: "not_holding".into(),
        workflow_intent: "stock_picking".into(),
        risk_preference: "medium".into(),
        investment_horizon: "swing".into(),
        ..Default::default()
    };
    let prompt = build_user_context_prompt(&ctx);
    assert!(prompt.contains("language=zh-CN"));
    assert!(prompt.contains("position_state=not_holding"));
    assert!(!prompt.contains("holding_cost"));
    assert!(!prompt.contains("holding_ratio_pct"));
    assert!(!prompt.contains("notes="));
}

#[test]
fn build_user_context_prompt_with_all_fields() {
    let ctx = AnalysisUserContext {
        language: "zh-CN".into(),
        position_state: "holding".into(),
        workflow_intent: "holding_review".into(),
        risk_preference: "high".into(),
        investment_horizon: "position".into(),
        holding_cost: Some(100.5),
        holding_ratio_pct: Some(25.0),
        notes: "some notes".into(),
    };
    let prompt = build_user_context_prompt(&ctx);
    assert!(prompt.contains("holding_cost=100.5000"));
    assert!(prompt.contains("holding_ratio_pct=25.00"));
    assert!(prompt.contains("notes=some notes"));
}
