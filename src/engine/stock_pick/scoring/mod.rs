use std::collections::HashSet;

use futures::{StreamExt, stream};

use crate::data::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

use crate::engine::stock_pick::{CandidateContext, EnrichedCandidate, FactorBreakdown};

mod factors;
mod normalize;
mod snapshots;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub(crate) use normalize::apply_portfolio_constraints;

pub(crate) async fn enrich_candidates(
    market_data: &MarketDataClient,
    candidates: &[CandidateContext],
    pick_count: usize,
) -> Vec<EnrichedCandidate> {
    let mut items = stream::iter(candidates.iter().cloned())
        .map(|candidate| {
            let market_data = market_data.clone();
            async move { light_enrich_candidate(&market_data, candidate).await }
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

    score_candidates(&mut items);
    let news_symbols = shortlist_candidates_for_news(&items, pick_count);

    let mut refreshed = stream::iter(items)
        .map(|mut candidate| {
            let market_data = market_data.clone();
            let fetch_news = news_symbols.contains(&candidate.symbol);
            async move {
                if fetch_news {
                    let news = market_data
                        .fetch_news(&candidate.symbol, 5, None, None)
                        .await
                        .unwrap_or_default();
                    candidate.theme_key =
                        infer_theme_key(&candidate.name, candidate.fundamentals.as_ref(), &news);
                    candidate.news = news;
                }
                candidate
            }
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

    score_candidates(&mut refreshed);
    refreshed
}

pub(crate) fn shortlist_candidates_for_news(
    items: &[EnrichedCandidate],
    pick_count: usize,
) -> HashSet<String> {
    let mut ranked = items
        .iter()
        .map(|item| (item.symbol.clone(), item.factor.total))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    let news_window = pick_count.saturating_mul(3).clamp(6, 12);
    ranked
        .into_iter()
        .take(news_window)
        .map(|(symbol, _)| symbol)
        .collect()
}

async fn light_enrich_candidate(
    market_data: &MarketDataClient,
    candidate: CandidateContext,
) -> EnrichedCandidate {
    let quote = market_data.fetch_quote(&candidate.symbol).await.ok();
    let fundamentals = market_data.fetch_fundamentals(&candidate.symbol).await.ok();
    let enrichment = market_data
        .fetch_enrichment(&candidate.symbol)
        .await
        .unwrap_or_default();
    let candles = market_data
        .fetch_candles(&candidate.symbol, "qfq", 260)
        .await
        .unwrap_or_default();
    let price = quote.as_ref().map(|item| item.close);
    let change_pct = candles.last().map(|item| item.change_pct);
    let market_cap = fundamentals.as_ref().and_then(|item| item.market_cap);
    let company_name = fundamentals
        .as_ref()
        .map(|item| item.company_name.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| candidate.name.clone());
    let industry = fundamentals
        .as_ref()
        .and_then(|item| item.industry.clone())
        .filter(|value| !value.trim().is_empty())
        .or(enrichment.industry.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut item = EnrichedCandidate {
        symbol: candidate.symbol.clone(),
        name: company_name,
        market: candidate.market.clone(),
        exchange: candidate.exchange.clone(),
        industry,
        price,
        change_pct,
        market_cap,
        theme_key: infer_theme_key(&candidate.name, fundamentals.as_ref(), &[]),
        fundamentals,
        enrichment: crate::engine::stock_pick::types::EnrichmentData {
            pe_ttm: enrichment.pe_ttm,
            pb: enrichment.pb,
            peg: enrichment.peg,
            ps: enrichment.ps,
            revenue_yoy: enrichment.revenue_yoy,
            net_profit_yoy: enrichment.net_profit_yoy,
            gross_margin: enrichment.gross_margin,
            fund_flow_net_ratio: enrichment.fund_flow_net_ratio,
            chip_benefit_ratio: enrichment.chip_benefit_ratio,
            chip_avg_cost: enrichment.chip_avg_cost,
            chip_concentration_90: enrichment.chip_concentration_90,
            dividend_yield: enrichment.dividend_yield,
            analyst_report_count: enrichment.analyst_report_count,
            analyst_buy_ratio: enrichment.analyst_buy_ratio,
        },
        news: Vec::new(),
        evidence_records: Vec::new(),
        candles,
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot: StockPickNewsSnapshot::default(),
        history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
        risk_snapshot: StockPickRiskSnapshot::default(),
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
        factor: FactorBreakdown::default(),
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
    };
    refresh_candidate_state(&mut item);
    item
}

fn refresh_candidate_state(item: &mut EnrichedCandidate) {
    item.market_snapshot = snapshots::build_market_snapshot(item);
    item.technical_snapshot = snapshots::build_technical_snapshot(&item.candles);
    item.fundamental_snapshot = snapshots::build_fundamental_snapshot(item);
    item.news_snapshot = snapshots::build_news_snapshot(item);
    item.risk_snapshot = snapshots::build_risk_snapshot(item);
    item.data_quality_snapshot = snapshots::build_data_quality_snapshot(item);

    let mut rejected = Vec::new();
    if item.candles.len() < 20 {
        rejected.push("insufficient_price_history".to_string());
    }
    if item.price.unwrap_or_default() <= 0.0 {
        rejected.push("invalid_price".to_string());
    }
    if item
        .candles
        .last()
        .is_some_and(|last| last.volume <= 0 || last.close <= 0.0)
    {
        rejected.push("illiquid_latest_bar".to_string());
    }
    if item.risk_snapshot.hard_negative_news {
        rejected.push("material_negative_news".to_string());
    }
    if !item.data_quality_snapshot.quote_ready {
        rejected.push("quote_not_ready".to_string());
    }
    item.pass_filter = rejected.is_empty();
    item.rejected_reasons = rejected;
    item.factor = factors::compute_factor_breakdown(item);
    item.description = snapshots::describe_candidate(item);
}

pub(crate) fn score_candidates(items: &mut [EnrichedCandidate]) {
    for item in items.iter_mut() {
        refresh_candidate_state(item);
    }

    normalize::apply_cross_sectional_normalization(items);

    for item in items.iter_mut() {
        item.description = snapshots::describe_candidate(item);
    }
}

pub(crate) fn infer_theme_key(
    _name: &str,
    fundamentals: Option<&FundamentalsSnapshot>,
    _news: &[NewsItem],
) -> String {
    fundamentals
        .and_then(|f| f.industry.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "general".to_string())
}
