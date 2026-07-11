use stock_analyzer::memory::{EmbeddingBackend, MemoryEntry, RagConfig, TradingMemoryLog};
use stock_analyzer::{LocalText, StructuredReflection, StructuredRiskAssessment};

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

fn make_log(max_entries: usize) -> TradingMemoryLog {
    TradingMemoryLog {
        log_path: std::path::PathBuf::new(),
        max_entries,
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
    let log = make_log(10);
    let blocks = vec![
        "[2025-01-01 | AAPL | Buy | 5.0%]".to_string(),
        "[2025-01-02 | MSFT | Hold | pending]".to_string(),
    ];
    let result = log.apply_rotation(blocks);
    assert_eq!(result.len(), 2);
}

#[test]
fn apply_rotation_drops_old_resolved() {
    let log = make_log(1);
    let blocks = vec![
        "[2025-01-01 | AAPL | Buy | 5.0%]".to_string(),
        "[2025-01-02 | MSFT | Hold | 2.0%]".to_string(),
        "[2025-01-03 | TSLA | Sell | pending]".to_string(),
    ];
    let result = log.apply_rotation(blocks);
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|b| b.contains("pending")));
}

#[test]
fn apply_rotation_zero_max_entries() {
    let log = make_log(0);
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
