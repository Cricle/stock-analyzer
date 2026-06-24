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

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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
#[allow(dead_code)]
pub(crate) fn test_shortlist_candidates_for_news(
    rows: Vec<(&str, f64)>,
    pick_count: usize,
) -> Vec<String> {
    use crate::pick::FactorBreakdown;
    use crate::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickMarketSnapshot,
        StockPickNewsSnapshot, StockPickRiskSnapshot, StockPickTechnicalSnapshot,
    };

    let mut shortlisted = crate::pick::scoring::shortlist_candidates_for_news(
        &rows
            .into_iter()
            .map(|(symbol, total)| EnrichedCandidate {
                symbol: symbol.to_string(),
                name: symbol.to_string(),
                market: "A-share".to_string(),
                exchange: "CN".to_string(),
                industry: "test".to_string(),
                price: Some(10.0),
                change_pct: Some(1.0),
                market_cap: Some(1_000_000_000.0),
                theme_key: "test".to_string(),
                fundamentals: None,
                news: Vec::new(),
                evidence_records: Vec::new(),
                candles: Vec::new(),
                technical_snapshot: StockPickTechnicalSnapshot::default(),
                market_snapshot: StockPickMarketSnapshot::default(),
                fundamental_snapshot: StockPickFundamentalSnapshot::default(),
                news_snapshot: StockPickNewsSnapshot::default(),
                history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
                risk_snapshot: StockPickRiskSnapshot::default(),
                data_quality_snapshot: StockPickDataQualitySnapshot::default(),
                factor: FactorBreakdown {
                    total,
                    ..FactorBreakdown::default()
                },
                pass_filter: true,
                rejected_reasons: Vec::new(),
                description: String::new(),
            })
            .collect::<Vec<_>>(),
        pick_count,
    )
    .into_iter()
    .collect::<Vec<_>>();
    shortlisted.sort();
    shortlisted
}
