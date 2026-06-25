use sa::data::NewsItem;
use sa::pick::pipeline::rank::{
    dedupe_news_items, default_selection_reason_codes, news_items_to_evidence_records,
    score_evidence_quality, summarize_history_matches,
};
use sa::pick::{CandidateEvidenceRecord, EnrichedCandidate, FactorBreakdown};
use sa::{
    StockPickHistoryMatchSnapshot, StockPickItem, StockPickNewsSnapshot, StockPickRiskSnapshot,
};

fn make_news(title: &str, source: &str, published_at: &str, url: &str, summary: &str) -> NewsItem {
    NewsItem {
        published_at: published_at.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        source: source.to_string(),
        url: Some(url.to_string()),
    }
}

fn make_enriched_candidate(
    factor: FactorBreakdown,
    signal_codes: Vec<String>,
    unique_source_count: usize,
    hard_negative_count: usize,
    evidence_len: usize,
    history_enabled: bool,
    history_sample_count: usize,
) -> EnrichedCandidate {
    EnrichedCandidate {
        symbol: "TEST".to_string(),
        name: "Test Corp".to_string(),
        market: "A-share".to_string(),
        exchange: "CN".to_string(),
        industry: "Technology".to_string(),
        price: Some(100.0),
        change_pct: Some(2.0),
        market_cap: Some(1_000_000_000.0),
        theme_key: "tech".to_string(),
        fundamentals: None,
        news: Vec::new(),
        evidence_records: (0..evidence_len)
            .map(|i| CandidateEvidenceRecord {
                query: format!("q{i}"),
                ..CandidateEvidenceRecord::default()
            })
            .collect(),
        candles: Vec::new(),
        technical_snapshot: Default::default(),
        market_snapshot: Default::default(),
        fundamental_snapshot: Default::default(),
        news_snapshot: StockPickNewsSnapshot {
            unique_source_count,
            hard_negative_count,
            ..StockPickNewsSnapshot::default()
        },
        history_match_snapshot: StockPickHistoryMatchSnapshot {
            enabled: history_enabled,
            sample_count: history_sample_count,
            ..StockPickHistoryMatchSnapshot::default()
        },
        risk_snapshot: StockPickRiskSnapshot {
            signal_codes,
            ..StockPickRiskSnapshot::default()
        },
        data_quality_snapshot: Default::default(),
        factor,
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
    }
}

// --- news_items_to_evidence_records ---

#[test]
fn evidence_records_empty_items() {
    let records = news_items_to_evidence_records("SYM", "A-share", "tech", &[], &[]);
    assert!(records.is_empty());
}

#[test]
fn evidence_records_dedup_identical_news() {
    let items = vec![
        make_news(
            "Same Title",
            "Source",
            "2024-01-01",
            "http://url1",
            "summary",
        ),
        make_news(
            "Same Title",
            "Source",
            "2024-01-01",
            "http://url1",
            "summary",
        ),
    ];
    let records = news_items_to_evidence_records("SYM", "A-share", "tech", &[], &items);
    assert_eq!(records.len(), 1);
}

#[test]
fn evidence_records_hard_negative_fraud() {
    let items = vec![make_news(
        "Company accused of fraud",
        "Reuters",
        "2024-01-01",
        "http://u1",
        "details",
    )];
    let records = news_items_to_evidence_records("SYM", "US", "tech", &[], &items);
    assert_eq!(records.len(), 1);
    assert!(records[0].hard_negative_flag);
    assert_eq!(records[0].sentiment_hint, "negative");
}

#[test]
fn evidence_records_hard_negative_bankruptcy() {
    let items = vec![make_news(
        "Filing for bankruptcy",
        "Bloomberg",
        "2024-01-02",
        "http://u2",
        "",
    )];
    let records = news_items_to_evidence_records("SYM", "US", "", &[], &items);
    assert!(records[0].hard_negative_flag);
    assert_eq!(records[0].sentiment_hint, "negative");
    assert_eq!(records[0].evidence_type, "US_news");
}

#[test]
fn evidence_records_positive_growth() {
    let items = vec![make_news(
        "Revenue growth exceeds expectations",
        "CNBC",
        "2024-01-03",
        "http://u3",
        "",
    )];
    let records = news_items_to_evidence_records("SYM", "HK", "ai", &[], &items);
    assert!(!records[0].hard_negative_flag);
    assert_eq!(records[0].sentiment_hint, "positive");
    assert_eq!(records[0].evidence_type, "HK_ai_news");
}

#[test]
fn evidence_records_positive_upgrade() {
    let items = vec![make_news(
        "Analyst upgrade to buy",
        "WSJ",
        "2024-01-04",
        "http://u4",
        "strong beat",
    )];
    let records = news_items_to_evidence_records("SYM", "US", "semicon", &[], &items);
    assert_eq!(records[0].sentiment_hint, "positive");
}

#[test]
fn evidence_records_neutral_default() {
    let items = vec![make_news(
        "Company holds annual meeting",
        "Local",
        "2024-01-05",
        "http://u5",
        "",
    )];
    let records = news_items_to_evidence_records("SYM", "A-share", "industry", &[], &items);
    assert!(!records[0].hard_negative_flag);
    assert_eq!(records[0].sentiment_hint, "neutral");
}

#[test]
fn evidence_records_uses_first_query() {
    let queries = vec!["query_a".to_string(), "query_b".to_string()];
    let items = vec![make_news("News", "Src", "2024-01-01", "http://u", "")];
    let records = news_items_to_evidence_records("SYM", "US", "tech", &queries, &items);
    assert_eq!(records[0].query, "query_a");
}

// --- dedupe_news_items ---

#[test]
fn dedupe_removes_exact_duplicates() {
    let items = vec![
        make_news("Title", "Src", "2024-01-01", "http://u", ""),
        make_news("Title", "Src", "2024-01-01", "http://u", ""),
    ];
    let result = dedupe_news_items(items);
    assert_eq!(result.len(), 1);
}

#[test]
fn dedupe_keeps_different_items() {
    let items = vec![
        make_news("Title A", "Src", "2024-01-01", "http://a", ""),
        make_news("Title B", "Src", "2024-01-02", "http://b", ""),
    ];
    let result = dedupe_news_items(items);
    assert_eq!(result.len(), 2);
}

#[test]
fn dedupe_sorted_by_date_desc() {
    let items = vec![
        make_news("Old", "Src", "2024-01-01", "http://a", ""),
        make_news("New", "Src", "2024-06-15", "http://b", ""),
        make_news("Mid", "Src", "2024-03-10", "http://c", ""),
    ];
    let result = dedupe_news_items(items);
    assert_eq!(result[0].title, "New");
    assert_eq!(result[1].title, "Mid");
    assert_eq!(result[2].title, "Old");
}

#[test]
fn dedupe_case_insensitive_title() {
    let items = vec![
        make_news("Hello World", "Src", "2024-01-01", "http://u", ""),
        make_news("hello world", "Src", "2024-01-01", "http://u", ""),
    ];
    let result = dedupe_news_items(items);
    assert_eq!(result.len(), 1);
}

#[test]
fn dedupe_empty_input() {
    let result = dedupe_news_items(Vec::new());
    assert!(result.is_empty());
}

// --- default_selection_reason_codes ---

#[test]
fn reason_codes_base_always_has_score_leader() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"score_leader".to_string()));
}

#[test]
fn reason_codes_high_momentum() {
    let factor = FactorBreakdown {
        momentum: 65.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"technical_support".to_string()));
}

#[test]
fn reason_codes_low_momentum_no_technical() {
    let factor = FactorBreakdown {
        momentum: 50.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(!codes.contains(&"technical_support".to_string()));
}

#[test]
fn reason_codes_high_quality() {
    let factor = FactorBreakdown {
        quality: 70.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"fundamental_support".to_string()));
}

#[test]
fn reason_codes_high_profitability() {
    let factor = FactorBreakdown {
        profitability: 65.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"fundamental_support".to_string()));
}

#[test]
fn reason_codes_evidence_support() {
    let factor = FactorBreakdown {
        evidence: 60.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"evidence_support".to_string()));
}

#[test]
fn reason_codes_history_support() {
    let factor = FactorBreakdown {
        history: 56.0,
        ..FactorBreakdown::default()
    };
    let item = make_enriched_candidate(factor, vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"history_support".to_string()));
}

#[test]
fn reason_codes_risk_capped() {
    let item = make_enriched_candidate(
        FactorBreakdown::default(),
        vec!["volatility".to_string()],
        0,
        0,
        0,
        false,
        0,
    );
    let codes = default_selection_reason_codes(&item);
    assert!(codes.contains(&"risk_capped".to_string()));
}

#[test]
fn reason_codes_no_risk_capped_when_empty() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 0, 0, 0, false, 0);
    let codes = default_selection_reason_codes(&item);
    assert!(!codes.contains(&"risk_capped".to_string()));
}

// --- score_evidence_quality ---

#[test]
fn evidence_quality_empty() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 0, 0, 0, false, 0);
    assert_eq!(score_evidence_quality(&item), 0);
}

#[test]
fn evidence_quality_with_sources_and_evidence() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 3, 0, 4, false, 0);
    assert_eq!(score_evidence_quality(&item), 50);
}

#[test]
fn evidence_quality_with_history() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 2, 0, 2, true, 5);
    assert_eq!(score_evidence_quality(&item), 50);
}

#[test]
fn evidence_quality_hard_negative_penalty() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 4, 2, 6, false, 0);
    assert_eq!(score_evidence_quality(&item), 60);
}

#[test]
fn evidence_quality_max_cap() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 10, 0, 20, true, 10);
    assert_eq!(score_evidence_quality(&item), 100);
}

#[test]
fn evidence_quality_penalty_floor_zero() {
    let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 0, 3, 0, false, 0);
    assert_eq!(score_evidence_quality(&item), 0);
}

// --- summarize_history_matches ---

#[test]
fn summarize_empty_picks() {
    let snapshot = summarize_history_matches(&[]);
    assert!(!snapshot.enabled);
    assert_eq!(snapshot.sample_count, 0);
}

#[test]
fn summarize_single_pick() {
    let pick = StockPickItem {
        history_match_snapshot: StockPickHistoryMatchSnapshot {
            enabled: true,
            sample_count: 5,
            vector_hit_count: 3,
            average_score: Some(0.8),
            hit_rate: Some(0.6),
            average_alpha_return: Some(0.05),
            top_matches: vec!["match1".to_string()],
        },
        ..StockPickItem::default()
    };
    let snapshot = summarize_history_matches(&[pick]);
    assert!(snapshot.enabled);
    assert_eq!(snapshot.sample_count, 5);
    assert_eq!(snapshot.vector_hit_count, 3);
    assert!((snapshot.average_score.unwrap() - 0.8).abs() < 0.01);
    assert!((snapshot.hit_rate.unwrap() - 0.6).abs() < 0.01);
    assert!((snapshot.average_alpha_return.unwrap() - 0.05).abs() < 0.01);
    assert_eq!(snapshot.top_matches.len(), 1);
}

#[test]
fn summarize_multiple_picks_aggregates() {
    let pick1 = StockPickItem {
        history_match_snapshot: StockPickHistoryMatchSnapshot {
            enabled: true,
            sample_count: 5,
            vector_hit_count: 2,
            average_score: Some(0.8),
            hit_rate: Some(0.6),
            average_alpha_return: Some(0.10),
            top_matches: vec!["m1".to_string()],
        },
        ..StockPickItem::default()
    };
    let pick2 = StockPickItem {
        history_match_snapshot: StockPickHistoryMatchSnapshot {
            enabled: false,
            sample_count: 3,
            vector_hit_count: 1,
            average_score: Some(0.6),
            hit_rate: Some(0.4),
            average_alpha_return: Some(0.0),
            top_matches: vec!["m2".to_string()],
        },
        ..StockPickItem::default()
    };
    let snapshot = summarize_history_matches(&[pick1, pick2]);
    assert!(snapshot.enabled);
    assert_eq!(snapshot.sample_count, 8);
    assert_eq!(snapshot.vector_hit_count, 3);
    assert!((snapshot.average_score.unwrap() - 0.7).abs() < 0.01);
    assert!((snapshot.hit_rate.unwrap() - 0.5).abs() < 0.01);
    assert!((snapshot.average_alpha_return.unwrap() - 0.05).abs() < 0.01);
    assert_eq!(snapshot.top_matches.len(), 2);
}
