use std::collections::HashSet;

use crate::data::{MarketKind, NewsItem};
use crate::engine::stock_pick::CandidateContext;

pub(super) fn market_kind_from_value(value: &str) -> MarketKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "a" | "a-share" | "a_share" | "ashare" | "cn" | "china" | "a股" => MarketKind::AShare,
        "hk" | "hkex" | "hongkong" | "hong_kong" | "港股" => MarketKind::HongKong,
        _ => MarketKind::UsEquity,
    }
}

pub(super) fn market_display_label(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "A-share",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

pub(super) fn market_search_label(market: MarketKind) -> &'static str {
    market_display_label(market)
}

pub(super) fn market_exchange_code(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "CN",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

pub(super) fn normalize_stock_pick_search_depth(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or("standard")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "light" | "shallow" => "light",
        "deep" | "high" => "deep",
        _ => "standard",
    }
}

pub(super) fn normalize_target_output_mode(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or("focused")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "expanded" | "full" => "expanded",
        _ => "focused",
    }
}

pub(super) fn derive_coarse_candidate_limit(candidate_limit: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => candidate_limit.clamp(6, 12),
        "deep" => candidate_limit.saturating_mul(2).clamp(12, 30),
        _ => candidate_limit.saturating_add(4).clamp(10, 24),
    }
}

pub(super) fn derive_deep_candidate_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.saturating_mul(2).clamp(3, 6),
        "deep" => pick_count.saturating_mul(4).clamp(6, 12),
        _ => pick_count.saturating_mul(3).clamp(4, 8),
    }
}

pub(super) fn derive_llm_review_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.clamp(1, 3),
        "deep" => pick_count.saturating_add(2).clamp(2, 5),
        _ => pick_count.saturating_add(1).clamp(2, 4),
    }
}

pub(super) fn stock_pick_search_time_range(search_depth: &str) -> Option<&'static str> {
    match search_depth {
        "light" => Some("day"),
        "deep" => Some("week"),
        _ => None,
    }
}

pub(super) fn deep_search_limit(search_depth: &str) -> usize {
    match search_depth {
        "light" => 6,
        "deep" => 16,
        _ => 10,
    }
}

pub(super) fn dedup_candidates(
    items: Vec<CandidateContext>,
    limit: usize,
) -> Vec<CandidateContext> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for item in items {
        if seen.insert(item.symbol.clone()) {
            output.push(item);
        }
        if output.len() >= limit {
            break;
        }
    }
    output
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

pub(super) fn default_selection_reason_codes(
    item: &crate::engine::stock_pick::EnrichedCandidate,
) -> Vec<String> {
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

pub(super) fn score_evidence_quality(item: &crate::engine::stock_pick::EnrichedCandidate) -> i32 {
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

pub(super) fn summarize_history_matches(
    picks: &[crate::models::StockPickItem],
) -> crate::models::StockPickHistoryMatchSnapshot {
    use crate::models::StockPickHistoryMatchSnapshot;
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
#[allow(dead_code)]
pub(crate) fn test_has_hard_negative_news(news: &[NewsItem]) -> bool {
    has_hard_negative_news(news)
}

#[cfg(test)]
#[allow(dead_code)]
fn has_hard_negative_news(news: &[NewsItem]) -> bool {
    news.iter().any(|item| {
        let title = item.title.to_ascii_lowercase();
        let summary = item.summary.to_ascii_lowercase();
        [
            "fraud",
            "bankruptcy",
            "delist",
            "recall",
            "probe",
            "investigation",
            "lawsuit",
            "default",
        ]
        .iter()
        .any(|keyword| title.contains(keyword) || summary.contains(keyword))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_kind_from_value_a_share() {
        assert!(matches!(
            market_kind_from_value("A-share"),
            MarketKind::AShare
        ));
        assert!(matches!(
            market_kind_from_value("a_share"),
            MarketKind::AShare
        ));
        assert!(matches!(market_kind_from_value("CN"), MarketKind::AShare));
        assert!(matches!(market_kind_from_value("A股"), MarketKind::AShare));
    }

    #[test]
    fn test_market_kind_from_value_hk() {
        assert!(matches!(market_kind_from_value("HK"), MarketKind::HongKong));
        assert!(matches!(
            market_kind_from_value("hongkong"),
            MarketKind::HongKong
        ));
        assert!(matches!(
            market_kind_from_value("港股"),
            MarketKind::HongKong
        ));
    }

    #[test]
    fn test_market_kind_from_value_us() {
        assert!(matches!(market_kind_from_value("US"), MarketKind::UsEquity));
        assert!(matches!(
            market_kind_from_value("unknown"),
            MarketKind::UsEquity
        ));
    }

    #[test]
    fn test_market_display_label() {
        assert_eq!(market_display_label(MarketKind::AShare), "A-share");
        assert_eq!(market_display_label(MarketKind::HongKong), "HK");
        assert_eq!(market_display_label(MarketKind::UsEquity), "US");
    }

    #[test]
    fn test_market_exchange_code() {
        assert_eq!(market_exchange_code(MarketKind::AShare), "CN");
        assert_eq!(market_exchange_code(MarketKind::HongKong), "HK");
        assert_eq!(market_exchange_code(MarketKind::UsEquity), "US");
    }

    #[test]
    fn test_normalize_stock_pick_search_depth() {
        assert_eq!(normalize_stock_pick_search_depth(Some("light")), "light");
        assert_eq!(normalize_stock_pick_search_depth(Some("shallow")), "light");
        assert_eq!(normalize_stock_pick_search_depth(Some("deep")), "deep");
        assert_eq!(normalize_stock_pick_search_depth(Some("high")), "deep");
        assert_eq!(normalize_stock_pick_search_depth(None), "standard");
        assert_eq!(normalize_stock_pick_search_depth(Some("other")), "standard");
    }

    #[test]
    fn test_normalize_target_output_mode() {
        assert_eq!(normalize_target_output_mode(Some("expanded")), "expanded");
        assert_eq!(normalize_target_output_mode(Some("full")), "expanded");
        assert_eq!(normalize_target_output_mode(None), "focused");
        assert_eq!(normalize_target_output_mode(Some("other")), "focused");
    }

    #[test]
    fn test_derive_coarse_candidate_limit_light() {
        let limit = derive_coarse_candidate_limit(10, "light");
        assert!(limit >= 6 && limit <= 12);
    }

    #[test]
    fn test_derive_coarse_candidate_limit_deep() {
        let limit = derive_coarse_candidate_limit(10, "deep");
        assert!(limit >= 12 && limit <= 30);
    }

    #[test]
    fn test_derive_coarse_candidate_limit_standard() {
        let limit = derive_coarse_candidate_limit(10, "standard");
        assert!(limit >= 10 && limit <= 24);
    }

    #[test]
    fn test_derive_deep_candidate_limit() {
        let limit = derive_deep_candidate_limit(5, "standard");
        assert!(limit >= 4 && limit <= 8);
    }

    #[test]
    fn test_derive_llm_review_limit() {
        let limit = derive_llm_review_limit(5, "standard");
        assert!(limit >= 2 && limit <= 4);
    }

    #[test]
    fn test_stock_pick_search_time_range() {
        assert_eq!(stock_pick_search_time_range("light"), Some("day"));
        assert_eq!(stock_pick_search_time_range("deep"), Some("week"));
        assert_eq!(stock_pick_search_time_range("standard"), None);
    }

    #[test]
    fn test_deep_search_limit() {
        assert_eq!(deep_search_limit("light"), 6);
        assert_eq!(deep_search_limit("deep"), 16);
        assert_eq!(deep_search_limit("standard"), 10);
    }

    fn make_candidate(symbol: &str, name: &str) -> CandidateContext {
        CandidateContext {
            symbol: symbol.to_string(),
            name: name.to_string(),
            market: "A-share".to_string(),
            exchange: "CN".to_string(),
            source_score: 50.0,
        }
    }

    #[test]
    fn test_dedup_candidates() {
        let items = vec![
            make_candidate("A", "A"),
            make_candidate("A", "A2"),
            make_candidate("B", "B"),
        ];
        let result = dedup_candidates(items, 10);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].symbol, "A");
        assert_eq!(result[1].symbol, "B");
    }

    #[test]
    fn test_dedup_candidates_with_limit() {
        let items = vec![make_candidate("A", "A"), make_candidate("B", "B")];
        let result = dedup_candidates(items, 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_dedupe_news_items() {
        let items = vec![
            NewsItem {
                published_at: "2024-01-15".to_string(),
                title: "Same Title".to_string(),
                summary: "s1".to_string(),
                source: "test".to_string(),
                url: None,
            },
            NewsItem {
                published_at: "2024-01-15".to_string(),
                title: "Same Title".to_string(),
                summary: "s2".to_string(),
                source: "test".to_string(),
                url: None,
            },
            NewsItem {
                published_at: "2024-01-16".to_string(),
                title: "Different".to_string(),
                summary: "s3".to_string(),
                source: "test".to_string(),
                url: None,
            },
        ];
        let result = dedupe_news_items(items);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_has_hard_negative_news_false() {
        let news = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Good news".to_string(),
            summary: "Everything is fine".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        assert!(!test_has_hard_negative_news(&news));
    }

    #[test]
    fn test_has_hard_negative_news_true() {
        let news = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Company under investigation".to_string(),
            summary: "Probe launched".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        assert!(test_has_hard_negative_news(&news));
    }

    #[test]
    fn test_has_hard_negative_news_bankruptcy() {
        let news = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Bankruptcy filing".to_string(),
            summary: "Company files for bankruptcy".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        assert!(test_has_hard_negative_news(&news));
    }

    #[test]
    fn test_summarize_history_matches_empty() {
        let result = summarize_history_matches(&[]);
        assert!(!result.enabled);
        assert_eq!(result.sample_count, 0);
    }
}
