use std::time::Duration;

#[allow(unused_imports)]
use anyhow::{Context, bail};
use futures::future::join_all;

use super::akshare_conv::news_item_from_akshare;
use super::news_filter::{
    extract_site_name_from_url, is_guidance_relevant_news,
    is_investment_research_evidence_page, is_macro_research_evidence_page,
    news_search_dedup_key,
};
use super::search::{preferred_search_language_for_query, within_date_window};
use super::{
    GENERAL_SEARCH_FALLBACK_QUERY_LIMIT, GeneralSearchIntent, MarketDataClient,
    NEWS_SEARCH_EVIDENCE_QUERY_LIMIT_PER_PROVIDER, NEWS_SEARCH_PROVIDER_TIMEOUT_SECS,
    NewsFetchAttempt, NewsItem, SEARXNG_QUERY_CACHE_TTL_SECS,
    SearchProviderConfig, SearchProviderKind, SearchScope,
    SearxngNewsEvidenceCacheEntry, SearxngNewsQueryCacheEntry,
};
/// Parameters for fetching search evidence with locale and scope mix strategy.
pub(super) struct SearchEvidenceParams<'a> {
    pub queries: &'a [String],
    pub time_range: Option<&'a str>,
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub general_intent: GeneralSearchIntent,
    pub proactive_general_query_limit: usize,
    pub provider_kind_filter: Option<SearchProviderKind>,
    pub news_query_limit_per_provider: Option<usize>,
    pub general_query_limit_per_provider: Option<usize>,
    pub batch_size: usize,
}

impl MarketDataClient {
    pub(super) async fn fetch_news_search_with_scope(
        &self,
        provider: &SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        scope: SearchScope,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let cache_key = self.search_query_cache_key(provider, query, language, time_range, scope);
        if let Some(cached) = self
            .cache_get_json_exact::<SearxngNewsQueryCacheEntry>(&cache_key)
            .await
        {
            if let Some(cached_error) = cached.cached_error {
                bail!(cached_error);
            }
            return Ok(cached.items);
        }
        let stale_key = self.stale_cache_key(&cache_key);
        if let Some(cached) = self
            .cache_get_json_exact::<SearxngNewsQueryCacheEntry>(&stale_key)
            .await
            && cached.cached_error.is_none()
            && !cached.items.is_empty()
        {
            tracing::info!(
                key = %cache_key,
                stale_key = %stale_key,
                scope = %scope.as_str(),
                "market data stale cache hit"
            );
            return Ok(cached.items);
        }

        let rewritten_query = provider.rewrite_query(query, language);
        if rewritten_query.trim().is_empty() {
            bail!("{} query reduced to empty", provider.display_name());
        }

        let items = match provider.kind {
            SearchProviderKind::Searxng => {
                self.fetch_searxng_provider_news_search(
                    provider,
                    &rewritten_query,
                    language,
                    time_range,
                    scope,
                )
                .await
            }
            SearchProviderKind::Gdelt => {
                self.fetch_gdelt_provider_news_search(
                    provider,
                    &rewritten_query,
                    language,
                    time_range,
                    scope,
                )
                .await
            }
            SearchProviderKind::Baidu => {
                self.fetch_baidu_news_search(&rewritten_query, time_range)
                    .await
            }
            SearchProviderKind::Uapis => {
                self.fetch_uapis_news_search(&rewritten_query, language, time_range)
                    .await
            }
        };
        match items {
            Ok(items) if !items.is_empty() => {
                self.cache_set_json(
                    &cache_key,
                    provider.cache_ttl_secs(),
                    &SearxngNewsQueryCacheEntry {
                        items: items.clone(),
                        cached_error: None,
                    },
                )
                .await;
                Ok(items)
            }
            Ok(_) => {
                let message = format!("{} returned no items", provider.display_name());
                self.cache_set_json(
                    &cache_key,
                    provider.negative_cache_ttl_secs(),
                    &SearxngNewsQueryCacheEntry {
                        items: Vec::new(),
                        cached_error: Some(message.clone()),
                    },
                )
                .await;
                bail!(message);
            }
            Err(error) => {
                let message = error.to_string();
                self.cache_set_json(
                    &cache_key,
                    provider.negative_cache_ttl_secs(),
                    &SearxngNewsQueryCacheEntry {
                        items: Vec::new(),
                        cached_error: Some(message.clone()),
                    },
                )
                .await;
                bail!(message);
            }
        }
    }

    pub(super) async fn fetch_news_search(
        &self,
        provider: &SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let news_items = self
            .fetch_news_search_with_scope(provider, query, language, time_range, SearchScope::News)
            .await
            .unwrap_or_default();
        if !news_items.is_empty() {
            return Ok(news_items);
        }
        bail!("{} returned no items", provider.display_name());
    }

    pub(super) async fn fetch_searxng_provider_news_search(
        &self,
        provider: &SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        scope: SearchScope,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let base_url = provider.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            bail!("SearXNG base URL is not configured");
        }
        let mut request = self.http.get(format!("{base_url}/search")).query(&[
            ("q", query),
            ("format", "json"),
            ("language", language),
        ]);
        if scope == SearchScope::News {
            request = request.query(&[("categories", "news")]);
        }
        let response = match tokio::time::timeout(
            Duration::from_secs(NEWS_SEARCH_PROVIDER_TIMEOUT_SECS),
            request
                .query(
                    &time_range
                        .map(|value| vec![("time_range", value)])
                        .unwrap_or_default(),
                )
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => match response.error_for_status() {
                Ok(response) => response,
                Err(error) => {
                    bail!("{} request failed: {error}", provider.display_name());
                }
            },
            Ok(Err(error)) => {
                bail!("failed to fetch {}: {error}", provider.display_name());
            }
            Err(_) => {
                bail!(
                    "{} request timed out after {}s",
                    provider.display_name(),
                    NEWS_SEARCH_PROVIDER_TIMEOUT_SECS
                );
            }
        };
        let payload = match response.json::<serde_json::Value>().await {
            Ok(payload) => payload,
            Err(error) => {
                bail!(
                    "failed to decode {} response: {error}",
                    provider.display_name()
                );
            }
        };
        let items = payload
            .get("results")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let title = item.get("title").and_then(|value| value.as_str())?.trim();
                if title.is_empty() {
                    return None;
                }
                let url = item
                    .get("url")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                let metadata = item
                    .get("metadata")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let source = item
                    .get("source")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| {
                        metadata
                            .and_then(|value| value.split('|').nth(1))
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .or_else(|| {
                        item.get("parsed_url")
                            .and_then(|value| value.get(1))
                            .and_then(|value| value.as_str())
                    })
                    .or_else(|| url.as_deref().and_then(extract_site_name_from_url))
                    .or_else(|| item.get("engine").and_then(|value| value.as_str()))
                    .unwrap_or("SearXNG")
                    .to_string();
                let summary = item
                    .get("content")
                    .or_else(|| item.get("snippet"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let published_at = item
                    .get("publishedDate")
                    .or_else(|| item.get("published_date"))
                    .or_else(|| item.get("pubdate"))
                    .and_then(|value| value.as_str())
                    .or_else(|| {
                        metadata
                            .and_then(|value| value.split('|').next())
                            .map(str::trim)
                    })
                    .unwrap_or_default()
                    .to_string();
                Some(NewsItem {
                    published_at,
                    title: title.to_string(),
                    summary,
                    source,
                    url,
                })
            })
            .collect::<Vec<_>>();
        Ok(items)
    }

    pub(super) async fn fetch_gdelt_provider_news_search(
        &self,
        provider: &SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        _scope: SearchScope,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let base_url = provider.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            bail!("GDELT base URL is not configured");
        }
        let language_hint = if language.eq_ignore_ascii_case("zh-CN") {
            Some("zh-CN")
        } else if language.eq_ignore_ascii_case("en-US") {
            Some("en-US")
        } else {
            None
        };
        let items = self
            .ak
            .gdelt_news_search(query, base_url, language_hint, time_range, NEWS_SEARCH_PROVIDER_TIMEOUT_SECS)
            .await
            .map_err(|e| anyhow::anyhow!("{}: {e}", provider.display_name()))?
            .into_iter()
            .map(news_item_from_akshare)
            .collect::<Vec<_>>();
        if items.is_empty() {
            bail!("{} returned no items", provider.display_name());
        }
        Ok(items)
    }

    pub(super) async fn fetch_baidu_news_search(
        &self,
        query: &str,
        _time_range: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let items = self
            .ak
            .baidu_news_search(query, NEWS_SEARCH_PROVIDER_TIMEOUT_SECS)
            .await
            .map_err(|e| anyhow::anyhow!("Baidu News: {e}"))?
            .into_iter()
            .map(news_item_from_akshare)
            .collect::<Vec<_>>();
        if items.is_empty() {
            tracing::warn!(query = %query, "Baidu News returned no items");
            bail!("Baidu News returned no items");
        }
        tracing::info!(query = %query, item_count = items.len(), "Baidu News search succeeded");
        Ok(items)
    }

    pub(super) async fn fetch_uapis_news_search(
        &self,
        query: &str,
        language: &str,
        _time_range: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let mut body = serde_json::json!({"query": query});
        if !language.is_empty() {
            body["language"] = serde_json::json!(language);
        }
        let response = match tokio::time::timeout(
            Duration::from_secs(NEWS_SEARCH_PROVIDER_TIMEOUT_SECS),
            self.http
                .post("https://uapis.cn/api/v1/search/aggregate")
                .json(&body)
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => match response.error_for_status() {
                Ok(response) => response,
                Err(error) => {
                    bail!("Uapis News request failed: {error}");
                }
            },
            Ok(Err(error)) => {
                bail!("failed to fetch Uapis News: {error}");
            }
            Err(_) => {
                bail!(
                    "Uapis News request timed out after {}s",
                    NEWS_SEARCH_PROVIDER_TIMEOUT_SECS
                );
            }
        };
        let payload = match response.json::<serde_json::Value>().await {
            Ok(payload) => payload,
            Err(error) => {
                bail!("failed to decode Uapis News response: {error}");
            }
        };
        let mut items = Vec::new();
        // The API may return results under different keys; try common patterns.
        let result_arrays = [
            payload.get("results").and_then(|v| v.as_array()),
            payload.get("data").and_then(|v| v.as_array()),
            payload
                .get("data")
                .and_then(|v| v.get("results"))
                .and_then(|v| v.as_array()),
        ];
        for array in result_arrays.into_iter().flatten() {
            for item in array {
                let title = item
                    .get("title")
                    .or_else(|| item.get("name"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .trim();
                if title.is_empty() {
                    continue;
                }
                let url = item
                    .get("url")
                    .or_else(|| item.get("link"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                let summary = item
                    .get("description")
                    .or_else(|| item.get("snippet"))
                    .or_else(|| item.get("summary"))
                    .or_else(|| item.get("content"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let source = item
                    .get("source")
                    .or_else(|| item.get("engine"))
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| url.as_deref().and_then(extract_site_name_from_url))
                    .unwrap_or("Uapis")
                    .to_string();
                let published_at = item
                    .get("published_at")
                    .or_else(|| item.get("date"))
                    .or_else(|| item.get("publishedDate"))
                    .or_else(|| item.get("time"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                items.push(NewsItem {
                    published_at,
                    title: title.to_string(),
                    summary,
                    source,
                    url,
                });
            }
        }
        if items.is_empty() {
            tracing::warn!(query = %query, "Uapis News returned no items");
            bail!("Uapis News returned no items");
        }
        tracing::info!(query = %query, item_count = items.len(), "Uapis News search succeeded");
        Ok(items)
    }
}

impl MarketDataClient {

    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) async fn fetch_searxng_news_search(
        &self,
        query: &str,
        language: &str,
        time_range: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let provider = self
            .search_providers
            .iter()
            .find(|provider| provider.kind == SearchProviderKind::Searxng)
            .context("SearXNG search provider is not configured")?;
        self.fetch_news_search_with_scope(provider, query, language, time_range, SearchScope::News)
            .await
    }

    pub async fn fetch_news_search_evidence(
        &self,
        queries: &[&str],
        language: &str,
        time_range: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let cache_key =
            self.search_evidence_cache_key(queries, language, time_range, SearchScope::News);
        if let Some(cached) = self
            .cache_get_json_exact::<SearxngNewsEvidenceCacheEntry>(&cache_key)
            .await
        {
            return Ok(cached.items.into_iter().take(limit).collect());
        }

        let mut merged = Vec::new();
        let mut dedup = std::collections::HashSet::new();
        let mut errors = Vec::new();
        let request_specs = self
            .search_providers
            .iter()
            .flat_map(|provider| {
                queries
                    .iter()
                    .take(NEWS_SEARCH_EVIDENCE_QUERY_LIMIT_PER_PROVIDER)
                    .map(move |query| (provider.clone(), (*query).to_string()))
            })
            .collect::<Vec<_>>();
        for batch in request_specs.chunks(2) {
            let responses = join_all(batch.iter().map(|(provider, query)| async move {
                (
                    provider.display_name(),
                    query.clone(),
                    self.fetch_news_search(provider, query, language, time_range)
                        .await,
                )
            }))
            .await;
            for (provider_name, query, response) in responses {
                match response {
                    Ok(items) => {
                        for item in items {
                            if dedup.insert(news_search_dedup_key(&item)) {
                                merged.push(item);
                            }
                        }
                    }
                    Err(error) => {
                        errors.push(format!("{provider_name} [{query}]: {error}"));
                    }
                }
            }
        }
        if merged.is_empty() && !errors.is_empty() {
            // Fallback: try Bing RSS web search when all providers failed
            for query in queries.iter().take(2) {
                if let Ok(items) = self.ak.bing_news_rss(query, 10).await {
                    for item in items.into_iter().map(news_item_from_akshare) {
                        if let Some(url) = &item.url
                            && dedup.insert(url.clone())
                            && is_guidance_relevant_news(&item)
                        {
                            merged.push(item);
                        }
                    }
                }
            }
            // Google News RSS fallback
            if merged.is_empty() {
                for query in queries.iter().take(2) {
                    if let Ok(items) = self.ak.google_news_rss(query, 10).await {
                        for item in items.into_iter().map(news_item_from_akshare) {
                            if let Some(url) = &item.url
                                && dedup.insert(url.clone())
                                && is_guidance_relevant_news(&item)
                            {
                                merged.push(item);
                            }
                        }
                    }
                }
            }
            // Sogou news fallback (works from China without proxy)
            if merged.is_empty() {
                for query in queries.iter().take(2) {
                    if let Ok(items) = self.ak.sogou_news_search(query, 10).await {
                        for item in items.into_iter().map(news_item_from_akshare) {
                            if let Some(url) = &item.url
                                && dedup.insert(url.clone())
                                && is_guidance_relevant_news(&item)
                            {
                                merged.push(item);
                            }
                        }
                    }
                }
            }
            if merged.is_empty() {
                let reason = if errors.is_empty() {
                    "search evidence returned no items".to_string()
                } else {
                    format!("search evidence returned no items; {}", errors.join(" | "))
                };
                tracing::warn!(
                    queries = ?queries,
                    language = %language,
                    time_range = ?time_range,
                    provider_count = self.search_providers.len(),
                    error_count = errors.len(),
                    reason = %reason,
                    "news search evidence returned no items"
                );
                bail!(reason);
            }
        }
        merged.sort_by(|left, right| right.published_at.cmp(&left.published_at));
        let limited = merged.into_iter().take(limit).collect::<Vec<_>>();
        self.cache_set_json(
            &cache_key,
            SEARXNG_QUERY_CACHE_TTL_SECS,
            &SearxngNewsEvidenceCacheEntry {
                items: limited.clone(),
            },
        )
        .await;
        Ok(limited)
    }

    pub async fn fetch_news_search_queries_with_attempts(
        &self,
        queries: &[String],
        language: &str,
        time_range: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        general_intent: GeneralSearchIntent,
    ) -> (Vec<NewsItem>, Vec<NewsFetchAttempt>) {
        let query_refs = queries
            .iter()
            .map(|query| query.as_str())
            .collect::<Vec<_>>();
        if let Ok(result_items) = self
            .fetch_news_search_evidence(&query_refs, language, time_range, 24)
            .await
        {
            let filtered = result_items
                .into_iter()
                .filter(|item| within_date_window(&item.published_at, start_date, end_date))
                .collect::<Vec<_>>();
            let mut attempts = Vec::new();
            for provider in &self.search_providers {
                for query in queries {
                    attempts.push(NewsFetchAttempt {
                        source: provider.display_name(),
                        query: Some(query.clone()),
                        success: true,
                        item_count: filtered.len(),
                        error: None,
                    });
                }
            }
            return (filtered, attempts);
        }

        let request_specs = self
            .search_providers
            .iter()
            .flat_map(|provider| {
                if !provider.supports_scope(SearchScope::News) {
                    return Vec::new();
                }
                queries
                    .iter()
                    .take(provider.query_budget(SearchScope::News))
                    .map(|query| (provider.clone(), (*query).clone(), SearchScope::News))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let responses = join_all(
            request_specs
                .iter()
                .map(|(provider, query, scope)| async move {
                    (
                        provider,
                        query,
                        *scope,
                        self.fetch_news_search(provider, query, language, time_range)
                            .await,
                    )
                }),
        )
        .await;
        let mut items = Vec::new();
        let mut attempts = Vec::new();
        for (provider, query, scope, response) in responses {
            match response {
                Ok(result_items) => {
                    let filtered = result_items
                        .into_iter()
                        .filter(|item| within_date_window(&item.published_at, start_date, end_date))
                        .collect::<Vec<_>>();
                    attempts.push(NewsFetchAttempt {
                        source: format!("{} [{}]", provider.display_name(), scope.as_str()),
                        query: Some(query.clone()),
                        success: true,
                        item_count: filtered.len(),
                        error: None,
                    });
                    items.extend(filtered);
                }
                Err(error) => attempts.push(NewsFetchAttempt {
                    source: format!("{} [{}]", provider.display_name(), scope.as_str()),
                    query: Some(query.clone()),
                    success: false,
                    item_count: 0,
                    error: Some(error.to_string()),
                }),
            }
        }
        if !items.is_empty() {
            return (items, attempts);
        }
        let general_queries = queries
            .iter()
            .take(GENERAL_SEARCH_FALLBACK_QUERY_LIMIT)
            .collect::<Vec<_>>();
        let general_request_specs = self
            .search_providers
            .iter()
            .flat_map(|provider| {
                if !provider.supports_scope(SearchScope::General) {
                    return Vec::new();
                }
                general_queries
                    .iter()
                    .take(provider.query_budget(SearchScope::General))
                    .map(|query| (provider.clone(), (*query).clone(), SearchScope::General))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let general_responses = join_all(general_request_specs.iter().map(
            |(provider, query, scope)| async move {
                (
                    provider,
                    query,
                    *scope,
                    self.fetch_general_search_evidence_with_intent(
                        provider,
                        query,
                        language,
                        None,
                        general_intent,
                    )
                    .await,
                )
            },
        ))
        .await;
        for (provider, query, scope, response) in general_responses {
            match response {
                Ok(result_items) => {
                    let filtered = result_items
                        .into_iter()
                        .filter(|item| within_date_window(&item.published_at, start_date, end_date))
                        .collect::<Vec<_>>();
                    attempts.push(NewsFetchAttempt {
                        source: format!("{} [{}]", provider.display_name(), scope.as_str()),
                        query: Some(query.clone()),
                        success: true,
                        item_count: filtered.len(),
                        error: None,
                    });
                    items.extend(filtered);
                }
                Err(error) => attempts.push(NewsFetchAttempt {
                    source: format!("{} [{}]", provider.display_name(), scope.as_str()),
                    query: Some(query.clone()),
                    success: false,
                    item_count: 0,
                    error: Some(error.to_string()),
                }),
            }
        }
        (items, attempts)
    }

    pub(super) async fn fetch_search_evidence_with_query_locales_and_scope_mix(
        &self,
        queries: &[String],
        time_range: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        general_intent: GeneralSearchIntent,
        proactive_general_query_limit: usize,
    ) -> (Vec<NewsItem>, Vec<NewsFetchAttempt>) {
        self.fetch_search_evidence_with_query_locales_and_scope_mix_strategy(SearchEvidenceParams {
            queries,
            time_range,
            start_date,
            end_date,
            general_intent,
            proactive_general_query_limit,
            provider_kind_filter: None,
            news_query_limit_per_provider: None,
            general_query_limit_per_provider: None,
            batch_size: 3,
        })
        .await
    }

    pub(super) async fn fetch_search_evidence_with_query_locales_and_scope_mix_strategy(
        &self,
        params: SearchEvidenceParams<'_>,
    ) -> (Vec<NewsItem>, Vec<NewsFetchAttempt>) {
        let SearchEvidenceParams {
            queries,
            time_range,
            start_date,
            end_date,
            general_intent,
            proactive_general_query_limit,
            provider_kind_filter,
            news_query_limit_per_provider,
            general_query_limit_per_provider,
            batch_size,
        } = params;
        let mut requests = Vec::new();
        let general_query_limit = general_query_limit_per_provider
            .unwrap_or(proactive_general_query_limit)
            .min(proactive_general_query_limit);
        let news_query_limit = news_query_limit_per_provider.unwrap_or(queries.len());

        for provider in self
            .search_providers
            .iter()
            .filter(|provider| provider_kind_filter.is_none_or(|kind| provider.kind == kind))
        {
            for query in queries.iter().take(news_query_limit) {
                requests.push((
                    provider.clone(),
                    query.clone(),
                    preferred_search_language_for_query(query).to_string(),
                    SearchScope::News,
                ));
            }
            for query in queries.iter().take(general_query_limit) {
                requests.push((
                    provider.clone(),
                    query.clone(),
                    preferred_search_language_for_query(query).to_string(),
                    SearchScope::General,
                ));
            }
        }

        let mut items = Vec::new();
        let mut attempts = Vec::new();
        for batch in requests.chunks(batch_size.max(1)) {
            let responses = join_all(batch.iter().map(
                |(provider, query, language, scope)| async move {
                    let response = match scope {
                        SearchScope::News => {
                            self.fetch_news_search(provider, query, language, time_range)
                                .await
                        }
                        SearchScope::General => {
                            self.fetch_general_search_evidence_with_intent(
                                provider,
                                query,
                                language,
                                None,
                                general_intent,
                            )
                            .await
                        }
                    };
                    (provider, query, language, scope, response)
                },
            ))
            .await;

            for (provider, query, language, scope, response) in responses {
                match response {
                    Ok(result_items) => {
                        let filtered = result_items
                            .into_iter()
                            .filter(|item| {
                                within_date_window(&item.published_at, start_date, end_date)
                            })
                            .collect::<Vec<_>>();
                        attempts.push(NewsFetchAttempt {
                            source: format!(
                                "{} [{}:{}]",
                                provider.display_name(),
                                scope.as_str(),
                                language
                            ),
                            query: Some(query.clone()),
                            success: true,
                            item_count: filtered.len(),
                            error: None,
                        });
                        items.extend(filtered);
                    }
                    Err(error) => attempts.push(NewsFetchAttempt {
                        source: format!(
                            "{} [{}:{}]",
                            provider.display_name(),
                            scope.as_str(),
                            language
                        ),
                        query: Some(query.clone()),
                        success: false,
                        item_count: 0,
                        error: Some(error.to_string()),
                    }),
                }
            }
        }

        (items, attempts)
    }
}

impl MarketDataClient {

    pub(super) async fn fetch_general_search_evidence_with_intent(
        &self,
        provider: &SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        intent: GeneralSearchIntent,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let items = self
            .fetch_news_search_with_scope(
                provider,
                query,
                language,
                time_range,
                SearchScope::General,
            )
            .await?;
        let raw_count = items.len();
        let filtered = items
            .into_iter()
            .filter(|item| match intent {
                GeneralSearchIntent::CompanyEvidence => is_investment_research_evidence_page(item),
                GeneralSearchIntent::MacroEvidence => is_macro_research_evidence_page(item),
            })
            .collect::<Vec<_>>();
        tracing::info!(
            provider = %provider.display_name(),
            query = %query,
            intent = ?intent,
            raw_count,
            filtered_count = filtered.len(),
            "general search evidence filter applied"
        );
        if filtered.is_empty() {
            bail!(
                "{} returned no usable general-search evidence",
                provider.display_name()
            );
        }
        Ok(filtered)
    }
}
