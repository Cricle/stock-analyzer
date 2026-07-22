use stock_analyzer::analysis::{
    ReportDiagnosticItem, ReportDiagnostics, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan,
};
use stock_analyzer::analysis::{
    collect_execution_blocking_gaps, normalize_gap_match_text, normalize_gap_to_i18n_key,
    related_gap_items, scenario_gap_messages, score_related_gap_match, tokenize_gap_match_text,
};

// --- normalize_gap_to_i18n_key ---

#[test]
fn normalize_gap_cash_flow_en() {
    assert_eq!(
        normalize_gap_to_i18n_key("fundamentals_cashflow_missing"),
        "setup_gap_cash_flow"
    );
    assert_eq!(
        normalize_gap_to_i18n_key("cashflow_quality_unresolved"),
        "setup_gap_cash_flow"
    );
}

#[test]
fn normalize_gap_news_en() {
    assert_eq!(
        normalize_gap_to_i18n_key("hk_news_sparse"),
        "setup_gap_news_coverage"
    );
    assert_eq!(
        normalize_gap_to_i18n_key("news_fetch_coverage_weak"),
        "setup_gap_news_coverage"
    );
}

#[test]
fn normalize_gap_technical() {
    assert_eq!(
        normalize_gap_to_i18n_key("indicator_unavailable"),
        "setup_gap_technical_confirmation"
    );
}

#[test]
fn normalize_gap_earnings() {
    assert_eq!(
        normalize_gap_to_i18n_key("fundamentals_sparse"),
        "setup_gap_earnings_data"
    );
    assert_eq!(
        normalize_gap_to_i18n_key("fundamentals_period_mixed"),
        "setup_gap_earnings_data"
    );
}

#[test]
fn normalize_gap_unknown_falls_back() {
    assert_eq!(
        normalize_gap_to_i18n_key("some_random_diagnostic_code"),
        "setup_gap_execution_boundary_incomplete"
    );
}

// --- normalize_gap_match_text ---

#[test]
fn normalize_gap_match_text_removes_punctuation() {
    assert_eq!(
        normalize_gap_match_text("hello, world; test: foo"),
        "hello  world  test  foo"
    );
}

#[test]
fn normalize_gap_match_text_lowercases() {
    assert_eq!(normalize_gap_match_text("HELLO World"), "hello world");
}

#[test]
fn normalize_gap_match_text_trims() {
    assert_eq!(normalize_gap_match_text("  hello  "), "hello");
}

#[test]
fn normalize_gap_match_text_cjk() {
    assert_eq!(normalize_gap_match_text("现金流缺失"), "现金流缺失");
}

// --- tokenize_gap_match_text ---

#[test]
fn tokenize_gap_match_text_basic() {
    let tokens = tokenize_gap_match_text("missing cash flow data");
    assert_eq!(tokens, vec!["missing", "cash", "flow", "data"]);
}

#[test]
fn tokenize_gap_match_text_short_tokens_filtered() {
    let tokens = tokenize_gap_match_text("a bb ccc");
    assert_eq!(tokens, vec!["bb", "ccc"]);
}

#[test]
fn tokenize_gap_match_text_empty() {
    let tokens = tokenize_gap_match_text("");
    assert!(tokens.is_empty());
}

// --- score_related_gap_match ---

#[test]
fn score_related_gap_match_some_overlap() {
    let base = vec!["cash".into(), "flow".into(), "data".into()];
    assert_eq!(score_related_gap_match(&base, "missing cash flow"), 2);
}

#[test]
fn score_related_gap_match_no_overlap() {
    let base = vec!["cash".into(), "flow".into()];
    assert_eq!(score_related_gap_match(&base, "volume spike"), 0);
}

#[test]
fn score_related_gap_match_empty_base() {
    let base: Vec<String> = vec![];
    assert_eq!(score_related_gap_match(&base, "cash flow"), 0);
}

// --- related_gap_items ---

#[test]
fn related_gap_items_returns_top_matches() {
    let item = ReportDiagnosticItem {
        code: "test".into(),
        message: "missing cash flow data".into(),
        severity: "warning".into(),
        ..Default::default()
    };
    let pool = vec![
        "cash flow incomplete".into(),
        "volume spike".into(),
        "cash data missing".into(),
    ];
    let results = related_gap_items(&item, &pool);
    assert!(!results.is_empty());
    assert!(results[0].contains("cash") || results[0].contains("data"));
}

// --- collect_execution_blocking_gaps ---

#[test]
fn collect_execution_blocking_gaps_ignores_llm_gaps() {
    // LLM-identified blocking gaps are now ignored; only system diagnostics matter.
    let mut research = StructuredResearchPlan::default();
    research.missing_evidence_ladder.blocking_gaps = vec!["cash flow data".into()];
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    let diagnostics = ReportDiagnostics::default();
    let gaps = collect_execution_blocking_gaps(&research, &trader, &portfolio, &diagnostics);
    assert!(gaps.is_empty());
}

#[test]
fn collect_execution_blocking_gaps_deduplicates() {
    let research = StructuredResearchPlan::default();
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    let diagnostics = ReportDiagnostics {
        availability: vec![
            ReportDiagnosticItem {
                code: "scenario_minimum_1".into(),
                message: "missing data".into(),
                severity: "error".into(),
                details: vec![],
                related_blocking_gaps: vec![],
                related_trigger_checklist: vec![],
                elevated_to_execution_blocking_gap: false,
            },
            ReportDiagnosticItem {
                code: "scenario_minimum_1".into(),
                message: "missing data".into(),
                severity: "error".into(),
                details: vec![],
                related_blocking_gaps: vec![],
                related_trigger_checklist: vec![],
                elevated_to_execution_blocking_gap: false,
            },
        ],
        ..Default::default()
    };
    let gaps = collect_execution_blocking_gaps(&research, &trader, &portfolio, &diagnostics);
    assert_eq!(
        gaps.iter().filter(|g| *g == "scenario_minimum_1").count(),
        1
    );
}

#[test]
fn collect_execution_blocking_gaps_from_diagnostics() {
    let research = StructuredResearchPlan::default();
    let trader = StructuredTraderPlan::default();
    let portfolio = StructuredPortfolioDecision::default();
    let diagnostics = ReportDiagnostics {
        availability: vec![ReportDiagnosticItem {
            code: "scenario_minimum_1".into(),
            message: "missing scenario data".into(),
            severity: "error".into(),
            details: vec![],
            related_blocking_gaps: vec![],
            related_trigger_checklist: vec![],
            elevated_to_execution_blocking_gap: false,
        }],
        ..Default::default()
    };
    let gaps = collect_execution_blocking_gaps(&research, &trader, &portfolio, &diagnostics);
    assert!(gaps.contains(&"scenario_minimum_1".to_string()));
}

// --- scenario_gap_messages ---

#[test]
fn scenario_gap_messages_from_availability() {
    let diagnostics = ReportDiagnostics {
        availability: vec![ReportDiagnosticItem {
            code: "scenario_minimum_1".into(),
            message: "missing data".into(),
            severity: "error".into(),
            details: vec![],
            related_blocking_gaps: vec![],
            related_trigger_checklist: vec![],
            elevated_to_execution_blocking_gap: false,
        }],
        ..Default::default()
    };
    let messages = scenario_gap_messages(&diagnostics);
    assert_eq!(messages, vec!["missing data"]);
}

#[test]
fn scenario_gap_messages_empty() {
    let diagnostics = ReportDiagnostics::default();
    let messages = scenario_gap_messages(&diagnostics);
    assert!(messages.is_empty());
}
