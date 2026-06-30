//! Daily guidance report generation.
//!
//! Aggregates data from multiple sources:
//! - News via searxng
//! - Market data via MarketDataClient
//! - Historical patterns via VectorStore
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

pub use news::{classify_impact, has_negation_before};
pub use sentiment::{sentiment_label, sentiment_score};

use std::time::Instant;

use super::store::GuidanceStore;
use super::*;
use crate::guide::embedding::semantic_embed;

const VECTOR_HISTORY_LIMIT: usize = 5;
const VECTOR_NEWS_LIMIT: usize = 10;

/// Generates daily guidance reports by aggregating all available data sources.
pub struct DailyGuidanceGenerator {
    store: GuidanceStore,
    market_data: crate::data::MarketDataClient,
    memory: std::sync::Arc<dyn crate::guide::GuidanceMemory>,
    http: reqwest::Client,
}

impl Clone for DailyGuidanceGenerator {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            market_data: self.market_data.clone(),
            memory: self.memory.clone(),
            http: self.http.clone(),
        }
    }
}

impl DailyGuidanceGenerator {
    pub fn new(
        market_data: crate::data::MarketDataClient,
        memory: std::sync::Arc<dyn crate::guide::GuidanceMemory>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            store: GuidanceStore::from_env(),
            market_data,
            memory,
            http,
        }
    }

    /// Build a DailyGuidanceReport from its component parts.
    fn build_report(
        date: &str,
        market: &str,
        elapsed_ms: u64,
        market_sentiment: MarketSentiment,
        news_items: Vec<GuidanceNewsItem>,
        news_sources: Vec<String>,
        sector_highlights: Vec<SectorHighlight>,
        stock_guidances: Vec<StockGuidance>,
        risk_alerts: Vec<RiskAlert>,
        user_guides: Vec<UserProfileGuide>,
        recent_stock_picks: Option<RecentStockPickSummary>,
        market_indices: Vec<MarketIndex>,
        historical_insights: Vec<HistoricalInsight>,
    ) -> DailyGuidanceReport {
        let news_count = news_items.len();
        let pos_count = news_items.iter().filter(|n| n.impact == "positive").count();
        let neg_count = news_items.iter().filter(|n| n.impact == "negative").count();
        let vector_memory_queries = historical_insights.len();
        let vector_memory_hits = historical_insights
            .iter()
            .filter(|i| i.confidence > 0.3)
            .count();

        let executive_summary = format!(
            "{} | {} news ({}+ / {}-) | {} risks | {} sectors",
            market_sentiment.label,
            news_count,
            pos_count,
            neg_count,
            risk_alerts.len(),
            sector_highlights.len(),
        );

        DailyGuidanceReport {
            report_id: uuid::Uuid::new_v4().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            date: date.to_string(),
            market: market.to_string(),
            market_sentiment,
            sector_highlights,
            stock_guidances,
            risk_alerts,
            user_guides,
            recent_stock_picks,
            market_indices,
            executive_summary,
            metadata: GuidanceMetadata {
                news_count,
                news_sources,
                vector_memory_queries,
                vector_memory_hits,
                cache_hit: false,
                generation_time_ms: elapsed_ms,
                data_freshness: chrono::Utc::now().to_rfc3339(),
            },
            key_news: news_items,
            historical_insights,
        }
    }

    /// Cache report and store embeddings.
    async fn persist_report(&self, report: &DailyGuidanceReport, date: &str, market: &str) {
        self.store.cache_report(report).await;

        let summary_text = format!(
            "daily guidance {} market {} sentiment {} news {}",
            date,
            market,
            report.market_sentiment.label,
            report.key_news.len(),
        );
        let embedding = semantic_embed(&summary_text);
        if let Err(e) = self.store.store_daily_summary(report, &embedding).await {
            tracing::warn!("failed to store daily summary in vector store: {e}");
        }

        match self.store.store_sector_embeddings(report).await {
            Ok(count) => tracing::info!(count, "stored sector/sentiment embeddings"),
            Err(e) => tracing::warn!("failed to store sector embeddings in vector store: {e}"),
        }

        let news_embeddings: Vec<(String, String, String, Option<String>, Vec<f32>)> = report
            .key_news
            .iter()
            .map(|news| {
                let text = format!("{} {} {}", news.title, news.summary, news.source);
                let embedding = semantic_embed(&text);
                (
                    news.title.clone(),
                    news.summary.clone(),
                    news.source.clone(),
                    news.url.clone(),
                    embedding,
                )
            })
            .collect();
        match self
            .store
            .batch_store_news_embeddings(date, market, &news_embeddings)
            .await
        {
            Ok(count) => tracing::info!(count, "batch stored news embeddings"),
            Err(e) => tracing::warn!("failed to batch store news embeddings: {e}"),
        }
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

        // Check cache first (unless force refresh)
        if !force_refresh {
            if let Some(cached) = self.store.get_cached_report(&date, market.as_str()).await {
                tracing::info!(
                    date = %date,
                    market = %market.as_str(),
                    "daily guidance cache hit"
                );
                return Ok(cached);
            }
        }

        // (Redundant second cache check removed — get_cached_report already checks both
        // fresh and stale keys, so a miss from the first call guarantees a miss here.)

        tracing::info!(
            date = %date,
            market = %market.as_str(),
            "generating daily guidance report"
        );

        // 1. Fetch news from searxng
        let (news_items, news_sources) = self.fetch_guidance_news(&market, &date).await;

        // 1b. Ensure vector guidance collection exists
        if let Err(e) = self.store.ensure_collection().await {
            tracing::warn!("failed to ensure vector guidance collection: {e}");
        }

        // 2. Query vector store for historical patterns
        let historical_insights = self.query_historical_patterns(&market, &date).await;

        // 3. Build market sentiment from news + memory
        let market_sentiment = self.assess_market_sentiment(&news_items, &market).await;

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
        let risk_alerts =
            self.generate_risk_alerts(&news_items, &market_sentiment, &market, &market_indices);

        // 7. Generate user profile guides
        let user_guides = self.generate_user_guides(
            &market_sentiment,
            &stock_guidances,
            &risk_alerts,
            &market_indices,
            &sector_highlights,
        );

        let elapsed = started.elapsed().as_millis() as u64;

        let mut report = Self::build_report(
            &date,
            market.as_str(),
            elapsed,
            market_sentiment,
            news_items,
            news_sources,
            sector_highlights,
            stock_guidances,
            risk_alerts,
            user_guides,
            None,
            market_indices,
            historical_insights,
        );

        // Enrich with latest stock pick results
        report.recent_stock_picks = self.fetch_recent_stock_picks(&market).await;

        self.persist_report(&report, &date, market.as_str()).await;

        tracing::info!(
            date = %date,
            market = %market.as_str(),
            elapsed_ms = elapsed,
            news_count = report.key_news.len(),
            "daily guidance report generated"
        );

        Ok(report)
    }

    /// Stage 1: Pre-fetch data and store in cache for later assembly.
    /// This does NOT call LLM — only fetches external data.
    pub async fn prepare(
        &self,
        market: &str,
        date: &str,
        _tickers: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let started = Instant::now();
        let market_enum = GuidanceMarket::from_str(market);

        // 1. Fetch news from searxng
        let (news_items, news_sources) = self.fetch_guidance_news(&market_enum, date).await;

        // 2. Ensure vector collection
        if let Err(e) = self.store.ensure_collection().await {
            tracing::warn!("failed to ensure vector guidance collection: {e}");
        }

        // 3. Query vector store for historical patterns
        let historical_insights = self.query_historical_patterns(&market_enum, date).await;

        // 4. Fetch market indices
        let market_indices = self.fetch_market_indices(&market_enum).await;

        // 5. Fetch recent stock picks
        let recent_stock_picks = self.fetch_recent_stock_picks(&market_enum).await;

        let elapsed = started.elapsed().as_millis() as u64;

        let prepared = crate::guide::store::PreparedData {
            market: market.to_string(),
            date: date.to_string(),
            news_json: serde_json::to_string(&news_items).unwrap_or_default(),
            news_sources,
            historical_insights_json: serde_json::to_string(&historical_insights)
                .unwrap_or_default(),
            market_indices_json: serde_json::to_string(&market_indices).unwrap_or_default(),
            recent_stock_picks_json: recent_stock_picks
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_default()),
            prepared_at: chrono::Utc::now().to_rfc3339(),
        };

        self.store.store_prepared(&prepared).await;

        tracing::info!(
            market = %market,
            date = %date,
            elapsed_ms = elapsed,
            "guidance data prepared"
        );

        Ok(())
    }

    /// Stage 2: Generate report from pre-prepared data (may call LLM).
    pub async fn generate_from_prepared(
        &self,
        market: &str,
        date: &str,
        tickers: Option<Vec<String>>,
    ) -> anyhow::Result<DailyGuidanceReport> {
        let started = Instant::now();
        let market_enum = GuidanceMarket::from_str(market);

        let prepared = self
            .store
            .get_prepared(date, market)
            .await
            .ok_or_else(|| anyhow::anyhow!("no prepared data found for {}:{}", market, date))?;

        // Deserialize pre-fetched data
        let news_items: Vec<GuidanceNewsItem> =
            serde_json::from_str(&prepared.news_json).unwrap_or_default();
        let historical_insights: Vec<HistoricalInsight> =
            serde_json::from_str(&prepared.historical_insights_json).unwrap_or_default();
        let market_indices: Vec<MarketIndex> =
            serde_json::from_str(&prepared.market_indices_json).unwrap_or_default();
        let recent_stock_picks: Option<RecentStockPickSummary> = prepared
            .recent_stock_picks_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        // LLM-dependent steps
        let market_sentiment = self
            .assess_market_sentiment(&news_items, &market_enum)
            .await;

        let mut stock_guidances = if let Some(ref tickers) = tickers {
            self.generate_stock_guidances(tickers, &market_enum, &news_items)
                .await
        } else {
            Vec::new()
        };
        self.enrich_stock_guidances(&mut stock_guidances).await;

        let sector_highlights = self.extract_sector_highlights(&news_items);
        let risk_alerts = self.generate_risk_alerts(
            &news_items,
            &market_sentiment,
            &market_enum,
            &market_indices,
        );
        let user_guides = self.generate_user_guides(
            &market_sentiment,
            &stock_guidances,
            &risk_alerts,
            &market_indices,
            &sector_highlights,
        );

        let elapsed = started.elapsed().as_millis() as u64;

        let report = Self::build_report(
            date,
            market,
            elapsed,
            market_sentiment,
            news_items,
            prepared.news_sources,
            sector_highlights,
            stock_guidances,
            risk_alerts,
            user_guides,
            recent_stock_picks,
            market_indices,
            historical_insights,
        );

        self.persist_report(&report, date, market).await;

        tracing::info!(
            market = %market,
            date = %date,
            elapsed_ms = elapsed,
            news_count = report.key_news.len(),
            "guidance report generated from prepared data"
        );

        Ok(report)
    }
}
