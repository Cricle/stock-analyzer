use std::time::Duration;

#[allow(unused_imports)]
use anyhow::{Context, bail};
use futures::future::join_all;

use super::news_filter::{
    extract_site_name_from_url, gdelt_timestamp_to_published_at,
    is_investment_research_evidence_page, is_macro_research_evidence_page, news_search_dedup_key,
};
use super::search::{preferred_search_language_for_query, within_date_window};
use super::{
    GENERAL_SEARCH_FALLBACK_QUERY_LIMIT, GeneralSearchIntent, MarketDataClient,
    NEWS_SEARCH_EVIDENCE_QUERY_LIMIT_PER_PROVIDER, NEWS_SEARCH_PROVIDER_TIMEOUT_SECS,
    NewsFetchAttempt, NewsItem, SEARXNG_QUERY_CACHE_TTL_SECS,
    SearchProviderConfig, SearchProviderKind, SearchScope,
    SearxngNewsEvidenceCacheEntry, SearxngNewsQueryCacheEntry,
};
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
        scope: SearchScope,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let base_url = provider.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            bail!("GDELT base URL is not configured");
        }
        let max_records = match scope {
            SearchScope::News => 25,
            SearchScope::General => 15,
        };
        let mode = match scope {
            SearchScope::News => "ArtList",
            SearchScope::General => "ArtList",
        };
        let final_query = if language.eq_ignore_ascii_case("zh-CN") {
            format!("{query} sourceLang:Chinese")
        } else if language.eq_ignore_ascii_case("en-US") {
            format!("{query} sourceLang:English")
        } else {
            query.to_string()
        };
        let mut request = self.http.get(base_url).query(&[
            ("query", final_query.as_str()),
            ("mode", mode),
            ("format", "json"),
            ("maxrecords", &max_records.to_string()),
            ("sort", "DateDesc"),
        ]);
        if let Some(range) = time_range {
            let timespan = match range {
                "day" => Some("1day"),
                "week" => Some("7days"),
                "month" => Some("1month"),
                other => Some(other),
            };
            if let Some(timespan) = timespan {
                request = request.query(&[("timespan", timespan)]);
            }
        } else {
            request = request.query(&[("timespan", "1month")]);
        }

        let response = match tokio::time::timeout(
            Duration::from_secs(NEWS_SEARCH_PROVIDER_TIMEOUT_SECS),
            request.send(),
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
            .get("articles")
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
                let source = item
                    .get("sourceCommonName")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| url.as_deref().and_then(extract_site_name_from_url))
                    .unwrap_or("GDELT")
                    .to_string();
                let summary = item
                    .get("seendate")
                    .and_then(|value| value.as_str())
                    .map(|value| format!("GDELT seen date: {value}"))
                    .unwrap_or_default();
                let published_at = item
                    .get("seendate")
                    .and_then(|value| value.as_str())
                    .map(gdelt_timestamp_to_published_at)
                    .unwrap_or_default();
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

    pub(super) async fn fetch_baidu_news_search(
        &self,
        query: &str,
        _time_range: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let encoded_query = percent_encode(query);
        let search_url = format!(
            "https://www.baidu.com/s?wd={}&tn=news&rtt=4&bsst=1&cl=2&medium=0",
            encoded_query
        );

        let response = match tokio::time::timeout(
            Duration::from_secs(NEWS_SEARCH_PROVIDER_TIMEOUT_SECS),
            self.http
                .get(&search_url)
                .header(
                    reqwest::header::USER_AGENT,
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                )
                .send(),
        )
        .await
        {
            Ok(Ok(response)) => match response.error_for_status() {
                Ok(response) => response,
                Err(error) => {
                    bail!("Baidu News request failed: {error}");
                }
            },
            Ok(Err(error)) => {
                bail!("failed to fetch Baidu News: {error}");
            }
            Err(_) => {
                bail!(
                    "Baidu News request timed out after {}s",
                    NEWS_SEARCH_PROVIDER_TIMEOUT_SECS
                );
            }
        };

        let body = match response.text().await {
            Ok(body) => body,
            Err(error) => {
                bail!("failed to read Baidu News response body: {error}");
            }
        };

        let mut items = Vec::new();
        // Parse Baidu news search result HTML
        // Each result is in a div with class "result"
        for block in body.split("class=\"result\"").skip(1) {
            let block_end = block.find("class=\"result\"").unwrap_or(block.len());
            let block = &block[..block_end.min(4000)];

            // Extract title and URL from <a> tag
            let title_and_url = extract_baidu_link(block);
            let (title, url) = match title_and_url {
                Some(pair) => pair,
                None => continue,
            };
            if title.trim().is_empty() {
                continue;
            }

            // Extract summary from content div or abstract
            let summary = extract_baidu_text_between(
                block,
                &["c-abstract", "c-span-last", "content-right_8Zs40"],
            )
            .unwrap_or_default();

            // Extract source and time from source span
            let source_info = extract_baidu_source(block);
            let (source, published_at) = source_info.unwrap_or_else(|| ("Baidu".to_string(), String::new()));

            items.push(NewsItem {
                published_at,
                title: title.trim().to_string(),
                summary,
                source,
                url: Some(url),
            });
        }

        // Fallback: try parsing with a simpler pattern for newer Baidu layouts
        if items.is_empty() {
            for chunk in body.split("<h3").skip(1) {
                let chunk_end = chunk.find("<h3").unwrap_or(chunk.len());
                let chunk = &chunk[..chunk_end.min(4000)];

                let title_and_url = extract_baidu_link(chunk);
                let (title, url) = match title_and_url {
                    Some(pair) => pair,
                    None => continue,
                };
                if title.trim().is_empty() {
                    continue;
                }

                let summary = extract_baidu_plain_text(chunk).unwrap_or_default();
                let source_info = extract_baidu_source(chunk);
                let (source, published_at) = source_info.unwrap_or_else(|| ("Baidu".to_string(), String::new()));

                items.push(NewsItem {
                    published_at,
                    title: title.trim().to_string(),
                    summary,
                    source,
                    url: Some(url),
                });
            }
        }

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

fn extract_baidu_link(html: &str) -> Option<(String, String)> {
    // Find first <a> with href and text
    let a_start = html.find("<a ")?;
    let a_end_tag = html[a_start..].find('>')? + a_start;
    let a_tag = &html[a_start..a_end_tag];

    // Extract href
    let href = a_tag
        .find("href=\"")
        .and_then(|i| {
            let rest = &a_tag[i + 6..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .or_else(|| {
            a_tag.find("href='").and_then(|i| {
                let rest = &a_tag[i + 6..];
                rest.find('\'').map(|end| rest[..end].to_string())
            })
        })?;

    // Extract title text between <a> and </a>
    let after_a = &html[a_end_tag + 1..];
    let a_close = after_a.find("</a>")?;
    let title_html = &after_a[..a_close];
    let title = strip_html_tags(title_html);
    let title = decode_html_entities(&title);

    if title.trim().is_empty() || href.is_empty() {
        return None;
    }

    let url = if href.starts_with("http") {
        href
    } else {
        format!("https://www.baidu.com{}", href)
    };

    Some((title, url))
}

fn extract_baidu_text_between(html: &str, class_names: &[&str]) -> Option<String> {
    for class_name in class_names {
        let marker = format!("class=\"{}\"", class_name);
        if let Some(pos) = html.find(&marker) {
            let after = &html[pos..];
            let tag_end = after.find('>')? + 1;
            let content_start = &after[tag_end..];
            let close_div = content_start.find("</div>").unwrap_or(content_start.len().min(800));
            let text = strip_html_tags(&content_start[..close_div]);
            let text = decode_html_entities(&text);
            if !text.trim().is_empty() {
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

fn extract_baidu_source(html: &str) -> Option<(String, String)> {
    // Look for source and time info
    let source_markers = ["c-color-gray", "c-gap-right-small", "news-source", "source"];
    for marker in &source_markers {
        let class_attr = format!("class=\"{}\"", marker);
        if let Some(pos) = html.find(&class_attr) {
            let after = &html[pos..];
            let tag_end = after.find('>')? + 1;
            let content = &after[tag_end..];
            let span_close = content.find("</span>").or_else(|| content.find("</a>")).unwrap_or(content.len().min(200));
            let text = strip_html_tags(&content[..span_close]);
            let text = decode_html_entities(&text);
            if !text.trim().is_empty() {
                // Try to split "source time" patterns
                let parts: Vec<&str> = text.split_whitespace().collect();
                if let Some(last) = parts.last().filter(|_| parts.len() >= 2)
                    && (last.contains('-') || last.contains(':'))
                {
                    let source = parts[..parts.len() - 1].join(" ");
                    return Some((source, last.to_string()));
                }
                return Some((text.trim().to_string(), String::new()));
            }
        }
    }
    None
}

fn extract_baidu_plain_text(html: &str) -> Option<String> {
    let text = strip_html_tags(html);
    let text = decode_html_entities(&text);
    let text = text.trim();
    if text.is_empty() { None } else { Some(text.to_string()) }
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
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
            // (not when they returned successfully with empty results).
            // From China, searxng can't reach upstream engines
            // (GFW blocks DuckDuckGo, Brave, Startpage, etc. and Bing News
            // redirects to homepage), but Bing regular search RSS works.
            for query in queries.iter().take(2) {
                let rss_url = format!(
                    "https://cn.bing.com/search?q={}&format=rss",
                    query.replace(' ', "+")
                );
                let response = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    self.http.get(&rss_url).send(),
                )
                .await;
                if let Ok(Ok(response)) = response
                    && let Ok(body) = response.text().await
                {
                    for item_xml in body.split("<item>").skip(1) {
                        let end = item_xml.find("</item>").unwrap_or(item_xml.len());
                        let xml = &item_xml[..end];
                        let title = extract_rss_tag(xml, "title")
                            .filter(|t| !t.contains("必应") && !t.contains("Bing"));
                        let link = extract_rss_tag(xml, "link");
                        let desc = extract_rss_tag(xml, "description");
                        let date = extract_rss_tag(xml, "pubDate")
                            .map(|d| normalize_rss_date(&d))
                            .unwrap_or_default();
                        if let (Some(title), Some(url)) = (title, link)
                            && dedup.insert(url.clone())
                        {
                            merged.push(NewsItem {
                                published_at: date,
                                title,
                                summary: desc.unwrap_or_default(),
                                source: "bing_rss".to_string(),
                                url: Some(url),
                            });
                        }
                    }
                }
            }
            // Google News RSS fallback
            if merged.is_empty() {
                for query in queries.iter().take(2) {
                    let gnews_url = format!(
                        "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
                        query.replace(' ', "+")
                    );
                    let response = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        self.http.get(&gnews_url).send(),
                    )
                    .await;
                    if let Ok(Ok(response)) = response
                        && let Ok(body) = response.text().await
                    {
                        for item_xml in body.split("<item>").skip(1) {
                            let end = item_xml.find("</item>").unwrap_or(item_xml.len());
                            let xml = &item_xml[..end];
                            let title = extract_rss_tag(xml, "title");
                            let link = extract_rss_tag(xml, "link");
                            let desc = extract_rss_tag(xml, "description");
                            let date = extract_rss_tag(xml, "pubDate")
                                .map(|d| normalize_rss_date(&d))
                                .unwrap_or_default();
                            if let (Some(title), Some(url)) = (title, link)
                                && dedup.insert(url.clone())
                            {
                                merged.push(NewsItem {
                                    published_at: date,
                                    title,
                                    summary: desc.unwrap_or_default(),
                                    source: "google_news_rss".to_string(),
                                    url: Some(url),
                                });
                            }
                        }
                    }
                }
            }
            // Sogou news RSS fallback (works from China without proxy)
            if merged.is_empty() {
                for query in queries.iter().take(2) {
                    let sogou_url = format!(
                        "https://news.sogou.com/news?query={}&sort=1",
                        query.replace(' ', "+")
                    );
                    let response = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        self.http.get(&sogou_url).send(),
                    )
                    .await;
                    if let Ok(Ok(response)) = response
                        && let Ok(body) = response.text().await
                    {
                        // Sogou returns HTML, extract links and titles from h3/a tags
                            let mut remaining = body.as_str();
                            while let Some(pos) = remaining.find("<h3") {
                                let chunk = &remaining[pos..];
                                if let Some(a_start) = chunk.find("<a href=\"") {
                                    let after_href = &chunk[a_start + 9..];
                                    if let Some(quote_end) = after_href.find('"') {
                                        let url = after_href[..quote_end].to_string();
                                        let title_start = after_href.find('>').map(|i| i + 1);
                                        let title = title_start.and_then(|start| {
                                            let title_chunk = &after_href[start..];
                                            title_chunk.find("</a>").map(|end| {
                                                let raw = &title_chunk[..end];
                                                // Strip HTML tags
                                                let clean = raw
                                                    .split('<')
                                                    .next()
                                                    .unwrap_or(raw)
                                                    .to_string();
                                                clean.trim().to_string()
                                            })
                                        });
                                        if let (Some(title), true) = (
                                            title,
                                            !url.is_empty() && url.starts_with("http"),
                                        ) && !title.is_empty()
                                            && dedup.insert(url.clone())
                                        {
                                            merged.push(NewsItem {
                                                published_at: String::new(),
                                                title,
                                                summary: String::new(),
                                                source: "sogou_news".to_string(),
                                                url: Some(url),
                                            });
                                        }
                                    }
                                }
                                remaining = &remaining[pos + 3..];
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
        self.fetch_search_evidence_with_query_locales_and_scope_mix_strategy(
            queries,
            time_range,
            start_date,
            end_date,
            general_intent,
            proactive_general_query_limit,
            None,
            None,
            None,
            3,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn fetch_search_evidence_with_query_locales_and_scope_mix_strategy(
        &self,
        queries: &[String],
        time_range: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        general_intent: GeneralSearchIntent,
        proactive_general_query_limit: usize,
        provider_kind_filter: Option<SearchProviderKind>,
        news_query_limit_per_provider: Option<usize>,
        general_query_limit_per_provider: Option<usize>,
        batch_size: usize,
    ) -> (Vec<NewsItem>, Vec<NewsFetchAttempt>) {
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

fn extract_rss_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml.find(&end_tag)?;
    let value = xml[start..end].trim();
    // Strip CDATA
    let value = value
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(value);
    let value = value.trim();
    if value.is_empty() { None } else { Some(value.to_string()) }
}

fn normalize_rss_date(raw: &str) -> String {
    // Try RFC 2822: "Wed, 03 Jun 2026 00:36:00 GMT"
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(raw) {
        return dt.format("%Y-%m-%d").to_string();
    }
    // Try ISO: "2026-06-03"
    if raw.len() >= 10 && raw.as_bytes()[4] == b'-' && raw.as_bytes()[7] == b'-' {
        return raw[..10].to_string();
    }
    String::new()
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
