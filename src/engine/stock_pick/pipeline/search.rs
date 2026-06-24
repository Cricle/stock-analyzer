use std::collections::HashSet;

use crate::data::NewsItem;
use crate::engine::stock_pick::{CandidateEvidenceRecord, EnrichedCandidate};

use super::helpers::market_kind_from_value;

pub(super) fn build_light_search_queries(
    request: &crate::models::StockPickRequest,
    candidates: &[crate::engine::stock_pick::CandidateContext],
) -> Vec<String> {
    let mut queries = Vec::new();
    let market_label =
        super::helpers::market_display_label(market_kind_from_value(&request.market));
    if let Some(sector) = request
        .sector_type
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{market_label} {sector} market outlook"));
        queries.push(format!("{market_label} {sector} earnings outlook"));
    }
    for candidate in candidates.iter().take(6) {
        let base_query = stock_pick_subject_query(&candidate.symbol, &candidate.name);
        queries.push(base_query.clone());
        queries.push(format!("{base_query} earnings"));
    }
    queries.sort();
    queries.dedup();
    queries
}

pub(super) fn should_skip_light_stage_search(
    request: &crate::models::StockPickRequest,
    candidates: &[crate::engine::stock_pick::CandidateContext],
) -> bool {
    request
        .candidate_symbols
        .as_ref()
        .is_some_and(|symbols| !symbols.is_empty() && symbols.len() == candidates.len())
}

pub(super) fn build_candidate_search_queries(
    candidate: &EnrichedCandidate,
    request: &crate::models::StockPickRequest,
) -> Vec<String> {
    let base_query = stock_pick_subject_query(&candidate.symbol, &candidate.name);
    let mut queries = vec![
        base_query.clone(),
        format!("{base_query} earnings"),
        format!("{base_query} guidance"),
    ];
    if !candidate.theme_key.trim().is_empty() && candidate.theme_key != "general" {
        queries.push(format!("{base_query} {} outlook", candidate.theme_key));
    }
    if let Some(strategy) = request
        .strategy
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{base_query} {strategy}"));
    }
    queries.sort();
    queries.dedup();
    queries
}

fn stock_pick_subject_query(symbol: &str, name: &str) -> String {
    let normalized_symbol = symbol.trim();
    let normalized_name = name.trim();
    if normalized_name.is_empty() || normalized_name.eq_ignore_ascii_case(normalized_symbol) {
        return normalized_symbol.to_string();
    }
    format!("{normalized_name} {normalized_symbol}")
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_pick_subject_query_with_name() {
        let result = stock_pick_subject_query("AAPL", "Apple Inc");
        assert_eq!(result, "Apple Inc AAPL");
    }

    #[test]
    fn test_stock_pick_subject_query_same_name_symbol() {
        let result = stock_pick_subject_query("AAPL", "AAPL");
        assert_eq!(result, "AAPL");
    }

    #[test]
    fn test_stock_pick_subject_query_empty_name() {
        let result = stock_pick_subject_query("AAPL", "");
        assert_eq!(result, "AAPL");
    }

    #[test]
    fn test_build_light_search_queries_basic() {
        let request = crate::models::StockPickRequest {
            market: "US".to_string(),
            sector_type: Some("Technology".to_string()),
            strategy: None,
            candidate_symbols: None,
            candidate_limit: None,
            pick_count: None,
            analysis_date: None,
            history_retrieval: None,
            language: None,
            target_output_mode: None,
            search_depth: None,
        };
        let candidates = vec![crate::engine::stock_pick::CandidateContext {
            symbol: "AAPL".to_string(),
            name: "Apple".to_string(),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            source_score: 80.0,
        }];
        let queries = build_light_search_queries(&request, &candidates);
        assert!(!queries.is_empty());
        assert!(queries.iter().any(|q| q.contains("Technology")));
        assert!(queries.iter().any(|q| q.contains("AAPL")));
    }

    #[test]
    fn test_build_light_search_queries_no_sector() {
        let request = crate::models::StockPickRequest {
            market: "US".to_string(),
            strategy: None,
            candidate_symbols: None,
            sector_type: None,
            candidate_limit: None,
            pick_count: None,
            analysis_date: None,
            history_retrieval: None,
            language: None,
            target_output_mode: None,
            search_depth: None,
        };
        let candidates = vec![];
        let queries = build_light_search_queries(&request, &candidates);
        assert!(queries.is_empty());
    }

    #[test]
    fn test_should_skip_light_stage_search_with_symbols() {
        let request = crate::models::StockPickRequest {
            market: "US".to_string(),
            strategy: None,
            candidate_symbols: Some(vec!["AAPL".to_string(), "MSFT".to_string()]),
            sector_type: None,
            candidate_limit: None,
            pick_count: None,
            analysis_date: None,
            history_retrieval: None,
            language: None,
            target_output_mode: None,
            search_depth: None,
        };
        let candidates = vec![
            crate::engine::stock_pick::CandidateContext {
                symbol: "AAPL".to_string(),
                name: "Apple".to_string(),
                market: "US".to_string(),
                exchange: "NASDAQ".to_string(),
                source_score: 80.0,
            },
            crate::engine::stock_pick::CandidateContext {
                symbol: "MSFT".to_string(),
                name: "Microsoft".to_string(),
                market: "US".to_string(),
                exchange: "NASDAQ".to_string(),
                source_score: 75.0,
            },
        ];
        assert!(should_skip_light_stage_search(&request, &candidates));
    }

    #[test]
    fn test_should_skip_light_stage_search_without_symbols() {
        let request = crate::models::StockPickRequest {
            market: "US".to_string(),
            strategy: None,
            candidate_symbols: None,
            sector_type: None,
            candidate_limit: None,
            pick_count: None,
            analysis_date: None,
            history_retrieval: None,
            language: None,
            target_output_mode: None,
            search_depth: None,
        };
        let candidates = vec![];
        assert!(!should_skip_light_stage_search(&request, &candidates));
    }

    #[test]
    fn test_news_items_to_evidence_records_positive() {
        let items = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Company beats earnings expectations".to_string(),
            summary: "Strong growth reported".to_string(),
            source: "reuters".to_string(),
            url: Some("http://example.com".to_string()),
        }];
        let records = news_items_to_evidence_records(
            "AAPL",
            "US",
            "tech",
            &["test query".to_string()],
            &items,
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].sentiment_hint, "positive");
        assert!(!records[0].hard_negative_flag);
    }

    #[test]
    fn test_news_items_to_evidence_records_negative() {
        let items = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Company under investigation".to_string(),
            summary: "Fraud allegations".to_string(),
            source: "reuters".to_string(),
            url: None,
        }];
        let records = news_items_to_evidence_records(
            "AAPL",
            "US",
            "tech",
            &["test query".to_string()],
            &items,
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].sentiment_hint, "negative");
        assert!(records[0].hard_negative_flag);
    }

    #[test]
    fn test_news_items_to_evidence_records_neutral() {
        let items = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Company announces new product".to_string(),
            summary: "Details to follow".to_string(),
            source: "reuters".to_string(),
            url: None,
        }];
        let records = news_items_to_evidence_records(
            "AAPL",
            "US",
            "tech",
            &["test query".to_string()],
            &items,
        );
        assert_eq!(records[0].sentiment_hint, "neutral");
    }

    #[test]
    fn test_news_items_to_evidence_records_dedup() {
        let items = vec![
            NewsItem {
                published_at: "2024-01-15".to_string(),
                title: "Same Title".to_string(),
                summary: "s1".to_string(),
                source: "reuters".to_string(),
                url: None,
            },
            NewsItem {
                published_at: "2024-01-15".to_string(),
                title: "Same Title".to_string(),
                summary: "s2".to_string(),
                source: "reuters".to_string(),
                url: None,
            },
        ];
        let records = news_items_to_evidence_records(
            "AAPL",
            "US",
            "tech",
            &["test query".to_string()],
            &items,
        );
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_news_items_to_evidence_records_empty_theme() {
        let items = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Test".to_string(),
            summary: "".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        let records = news_items_to_evidence_records("AAPL", "US", "", &["q".to_string()], &items);
        assert_eq!(records[0].evidence_type, "US_news");
    }
}
