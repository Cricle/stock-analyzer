use std::collections::HashSet;

use crate::data::NewsItem;
use crate::{StockPickHistoryMatchSnapshot, StockPickItem};

use super::{CandidateEvidenceRecord, EnrichedCandidate};

// ---------------------------------------------------------------------------
// Evidence processing
// ---------------------------------------------------------------------------

pub(super) fn news_items_to_evidence_records(
    _symbol: &str,
    market: &str,
    theme_key: &str,
    queries: &[String],
    items: &[NewsItem],
) -> Vec<CandidateEvidenceRecord> {
    let query = queries.first().cloned().unwrap_or_default();
    let mut dedup = HashSet::new();
    let mut records = Vec::new();
    for item in items {
        let dedupe_key = format!(
            "{}|{}|{}|{}",
            item.title.trim().to_ascii_lowercase(),
            item.source.trim().to_ascii_lowercase(),
            item.published_at.trim(),
            item.url
                .clone()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        );
        if !dedup.insert(dedupe_key.clone()) {
            continue;
        }
        let combined = format!("{} {}", item.title, item.summary).to_ascii_lowercase();
        let hard_negative_flag = [
            "investigation",
            "fraud",
            "default",
            "bankruptcy",
            "delist",
            "downgrade",
            "lawsuit",
        ]
        .iter()
        .any(|token| combined.contains(token));
        let sentiment_hint = if hard_negative_flag {
            "negative"
        } else if [
            "beat",
            "growth",
            "upgrade",
            "approval",
            "expansion",
            "contract",
            "buyback",
        ]
        .iter()
        .any(|token| combined.contains(token))
        {
            "positive"
        } else {
            "neutral"
        };
        records.push(CandidateEvidenceRecord {
            query: query.clone(),
            published_at: item.published_at.clone(),
            title: item.title.clone(),
            summary: item.summary.clone(),
            source: item.source.clone(),
            url: item.url.clone().unwrap_or_default(),
            evidence_type: if theme_key.trim().is_empty() {
                format!("{}_news", market.trim())
            } else {
                format!("{}_{}_news", market.trim(), theme_key.trim())
            },
            sentiment_hint: sentiment_hint.to_string(),
            hard_negative_flag,
            dedupe_key,
        });
    }
    records
}

pub(super) fn dedupe_news_items(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut dedup = HashSet::new();
    let mut output = Vec::new();
    for item in items {
        let key = format!(
            "{}|{}|{}|{}",
            item.title.trim().to_ascii_lowercase(),
            item.source.trim().to_ascii_lowercase(),
            item.published_at.trim(),
            item.url
                .clone()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        );
        if dedup.insert(key) {
            output.push(item);
        }
    }
    output.sort_by(|left, right| right.published_at.cmp(&left.published_at));
    output
}

// ---------------------------------------------------------------------------
// Selection reason and quality scoring
// ---------------------------------------------------------------------------

pub(super) fn default_selection_reason_codes(item: &EnrichedCandidate) -> Vec<String> {
    let mut codes = vec!["score_leader".to_string()];
    if item.factor.momentum >= 60.0 {
        codes.push("technical_support".to_string());
    }
    if item.factor.quality >= 60.0 || item.factor.profitability >= 60.0 {
        codes.push("fundamental_support".to_string());
    }
    if item.factor.evidence >= 55.0 {
        codes.push("evidence_support".to_string());
    }
    if item.factor.history >= 55.0 {
        codes.push("history_support".to_string());
    }
    if !item.risk_snapshot.signal_codes.is_empty() {
        codes.push("risk_capped".to_string());
    }
    codes
}

pub(super) fn score_evidence_quality(item: &EnrichedCandidate) -> i32 {
    let source_score = item.news_snapshot.unique_source_count.min(5) as i32 * 10;
    let evidence_score = item.evidence_records.len().min(8) as i32 * 5;
    let history_score = if item.history_match_snapshot.sample_count > 0 {
        20
    } else {
        0
    };
    let penalty = item.news_snapshot.hard_negative_count.min(3) as i32 * 5;
    (source_score + evidence_score + history_score - penalty).clamp(0, 100)
}

pub(super) fn summarize_history_matches(picks: &[StockPickItem]) -> StockPickHistoryMatchSnapshot {
    if picks.is_empty() {
        return StockPickHistoryMatchSnapshot::default();
    }
    let enabled = picks.iter().any(|pick| pick.history_match_snapshot.enabled);
    let sample_count = picks
        .iter()
        .map(|pick| pick.history_match_snapshot.sample_count)
        .sum::<usize>();
    let vector_hit_count = picks
        .iter()
        .map(|pick| pick.history_match_snapshot.vector_hit_count)
        .sum::<usize>();
    let average_score_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.average_score)
        .collect::<Vec<_>>();
    let hit_rate_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.hit_rate)
        .collect::<Vec<_>>();
    let alpha_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.average_alpha_return)
        .collect::<Vec<_>>();
    let top_matches = picks
        .iter()
        .flat_map(|pick| pick.history_match_snapshot.top_matches.clone())
        .take(12)
        .collect::<Vec<_>>();
    StockPickHistoryMatchSnapshot {
        enabled,
        sample_count,
        vector_hit_count,
        average_score: average_score_values
            .is_empty()
            .then_some(0.0)
            .filter(|_| false)
            .or_else(|| {
                (!average_score_values.is_empty()).then_some(
                    average_score_values.iter().sum::<f64>() / average_score_values.len() as f64,
                )
            }),
        hit_rate: (!hit_rate_values.is_empty())
            .then_some(hit_rate_values.iter().sum::<f64>() / hit_rate_values.len() as f64),
        average_alpha_return: (!alpha_values.is_empty())
            .then_some(alpha_values.iter().sum::<f64>() / alpha_values.len() as f64),
        top_matches,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::NewsItem;
    use crate::pick::FactorBreakdown;
    use crate::{StockPickNewsSnapshot, StockPickRiskSnapshot};

    fn make_news(
        title: &str,
        source: &str,
        published_at: &str,
        url: &str,
        summary: &str,
    ) -> NewsItem {
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
        // source_score = min(3,5)*10 = 30, evidence_score = min(4,8)*5 = 20
        assert_eq!(score_evidence_quality(&item), 50);
    }

    #[test]
    fn evidence_quality_with_history() {
        let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 2, 0, 2, true, 5);
        // source=20, evidence=10, history=20 => 50
        assert_eq!(score_evidence_quality(&item), 50);
    }

    #[test]
    fn evidence_quality_hard_negative_penalty() {
        let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 4, 2, 6, false, 0);
        // source=40, evidence=30, history=0, penalty=min(2,3)*5=10 => 60
        assert_eq!(score_evidence_quality(&item), 60);
    }

    #[test]
    fn evidence_quality_max_cap() {
        let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 10, 0, 20, true, 10);
        // source=min(10,5)*10=50, evidence=min(20,8)*5=40, history=20 => 110, capped to 100
        assert_eq!(score_evidence_quality(&item), 100);
    }

    #[test]
    fn evidence_quality_penalty_floor_zero() {
        let item = make_enriched_candidate(FactorBreakdown::default(), vec![], 0, 3, 0, false, 0);
        // source=0, evidence=0, history=0, penalty=15 => -15, clamped to 0
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
        assert!(snapshot.enabled); // at least one enabled
        assert_eq!(snapshot.sample_count, 8); // 5 + 3
        assert_eq!(snapshot.vector_hit_count, 3); // 2 + 1
        // average of 0.8 and 0.6 = 0.7
        assert!((snapshot.average_score.unwrap() - 0.7).abs() < 0.01);
        // average of 0.6 and 0.4 = 0.5
        assert!((snapshot.hit_rate.unwrap() - 0.5).abs() < 0.01);
        // average of 0.10 and 0.0 = 0.05
        assert!((snapshot.average_alpha_return.unwrap() - 0.05).abs() < 0.01);
        assert_eq!(snapshot.top_matches.len(), 2);
    }
}
