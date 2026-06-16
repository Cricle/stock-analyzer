//! Daily guidance report generation.
//!
//! Aggregates data from multiple sources:
//! - News via searxng
//! - Market data via MarketDataClient
//! - Historical patterns from memory system
//!
//! Produces structured JSON reports (no i18n in backend).

mod enrichment;
mod guides;
mod indices;
mod news;
mod patterns;
mod risks;
mod sentiment;
mod stocks;

use std::time::Instant;

use super::store::GuidanceStore;
use super::*;

pub(crate) struct ReportComponents {
    pub date: String,
    pub market: String,
    pub elapsed_ms: u64,
    pub market_sentiment: MarketSentiment,
    pub news_items: Vec<GuidanceNewsItem>,
    pub news_sources: Vec<String>,
    pub sector_highlights: Vec<SectorHighlight>,
    pub stock_guidances: Vec<StockGuidance>,
    pub risk_alerts: Vec<RiskAlert>,
    pub user_guides: Vec<UserProfileGuide>,
    pub recent_stock_picks: Option<RecentStockPickSummary>,
    pub market_indices: Vec<MarketIndex>,
    pub historical_insights: Vec<HistoricalInsight>,
}

/// Generates daily guidance reports by aggregating all available data sources.
pub struct DailyGuidanceGenerator {
    store: GuidanceStore,
    market_data: crate::data::MarketDataClient,
    memory: std::sync::Arc<dyn crate::engine::guidance::GuidanceMemory>,
    llm: Option<crate::engine::llm::LlmClient>,
}

impl Clone for DailyGuidanceGenerator {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            market_data: self.market_data.clone(),
            memory: self.memory.clone(),
            llm: self.llm.clone(),
        }
    }
}

impl DailyGuidanceGenerator {
    pub fn new(
        market_data: crate::data::MarketDataClient,
        memory: std::sync::Arc<dyn crate::engine::guidance::GuidanceMemory>,
    ) -> Self {
        Self {
            store: GuidanceStore::from_env(),
            market_data,
            memory,
            llm: None,
        }
    }

    pub fn with_llm(mut self, llm: crate::engine::llm::LlmClient) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Build a DailyGuidanceReport from its component parts.
    fn build_report(components: ReportComponents) -> DailyGuidanceReport {
        let news_count = components.news_items.len();
        let pos_count = components.news_items.iter().filter(|n| n.impact == "positive").count();
        let neg_count = components.news_items.iter().filter(|n| n.impact == "negative").count();
        let historical_query_count = components.historical_insights.len();
        let historical_hit_count = components.historical_insights
            .iter()
            .filter(|i| i.confidence > 0.3)
            .count();

        let executive_summary = format!(
            "{} | {} news ({}+ / {}-) | {} risks | {} sectors",
            components.market_sentiment.label,
            news_count,
            pos_count,
            neg_count,
            components.risk_alerts.len(),
            components.sector_highlights.len(),
        );
        let executive_summary_key = serde_json::json!({
            "i18n_key": "guidance.executive_summary",
            "label": components.market_sentiment.label,
            "news_count": news_count,
            "pos": pos_count,
            "neg": neg_count,
            "risk_count": components.risk_alerts.len(),
            "sector_count": components.sector_highlights.len(),
        });

        DailyGuidanceReport {
            report_id: uuid::Uuid::new_v4().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            date: components.date,
            market: components.market,
            market_sentiment: components.market_sentiment,
            sector_highlights: components.sector_highlights,
            stock_guidances: components.stock_guidances,
            risk_alerts: components.risk_alerts,
            user_guides: components.user_guides,
            recent_stock_picks: components.recent_stock_picks,
            market_indices: components.market_indices,
            executive_summary,
            executive_summary_key: Some(executive_summary_key),
            metadata: GuidanceMetadata {
                news_count,
                news_sources: components.news_sources,
                historical_query_count,
                historical_hit_count,
                cache_hit: false,
                generation_time_ms: components.elapsed_ms,
                data_freshness: chrono::Utc::now().to_rfc3339(),
            },
            key_news: components.news_items,
            historical_insights: components.historical_insights,
            llm_token_usage: None,
        }
    }

    /// Cache report.
    #[allow(clippy::type_complexity)]
    async fn persist_report(&self, report: &DailyGuidanceReport, _date: &str, _market: &str) {
        self.store.cache_report(report).await;
        // Vector store persistence removed with RAG system.
    }

    /// Generate or retrieve a cached daily guidance report.
    pub async fn generate(
        &self,
        request: &DailyGuidanceRequest,
    ) -> anyhow::Result<DailyGuidanceReport> {
        let started = Instant::now();
        let market = request.market();
        let date = chrono::Utc::now().date_naive().to_string();
        let force_refresh = request.refresh.unwrap_or(false);

        // Check Redis cache first (unless force refresh)
        if !force_refresh
            && let Some(cached) = self.store.get_cached_report(&date, market.as_str()).await
        {
            tracing::info!(
                date = %date,
                market = %market.as_str(),
                "daily guidance cache hit"
            );
            return Ok(cached);
        }

        // (Redundant second cache check removed — get_cached_report already checks both
        // fresh and stale keys, so a miss from the first call guarantees a miss here.)

        tracing::info!(
            date = %date,
            market = %market.as_str(),
            "generating daily guidance report"
        );

        // 1. Fetch news from searxng
        let (mut news_items, news_sources) = self.fetch_guidance_news(&market, &date).await;

        // 2. Query historical patterns from memory
        let historical_insights = self.query_historical_patterns(&market, &date).await;

        // 3. Build market sentiment from news + memory
        let market_sentiment = self.assess_market_sentiment(&mut news_items, &market).await;

        // 4. Generate stock guidances for specific tickers if requested
        let mut stock_guidances = if let Some(tickers) = &request.tickers {
            self.generate_stock_guidances(tickers, &market, &news_items)
                .await
        } else {
            Vec::new()
        };

        // 4b. Enrich stock guidances with live price data and company names
        self.enrich_stock_guidances(&mut stock_guidances).await;

        // 4c. Fetch major market indices
        let market_indices = self.fetch_market_indices(&market).await;

        // 5. Derive sector highlights from news
        let sector_highlights = self.extract_sector_highlights(&news_items);

        // 6. Generate risk alerts
        let risk_alerts = self.generate_risk_alerts(&news_items, &market_sentiment, &market, &market_indices);

        // 7. Generate user profile guides
        let user_guides =
            self.generate_user_guides(&market_sentiment, &stock_guidances, &risk_alerts, &market_indices, &sector_highlights);

        let elapsed = started.elapsed().as_millis() as u64;

        let mut report = Self::build_report(ReportComponents {
            date,
            market: market.as_str().to_string(),
            elapsed_ms: elapsed,
            market_sentiment,
            news_items,
            news_sources,
            sector_highlights,
            stock_guidances,
            risk_alerts,
            user_guides,
            recent_stock_picks: None,
            market_indices,
            historical_insights,
        });

        // Attach LLM token usage
        self.attach_token_usage(&mut report).await;

        // Enrich with latest stock pick results
        report.recent_stock_picks = self.fetch_recent_stock_picks(&market).await;

        self.persist_report(&report, &report.date, market.as_str()).await;

        tracing::info!(
            date = %report.date,
            market = %market.as_str(),
            elapsed_ms = elapsed,
            news_count = report.key_news.len(),
            "daily guidance report generated"
        );

        Ok(report)
    }

    /// Attach LLM token usage to a report if an LLM client is available.
    pub async fn attach_token_usage(&self, report: &mut DailyGuidanceReport) {
        if let Some(ref llm) = self.llm {
            report.llm_token_usage = Some(llm.usage_summary().await);
        }
    }


}
