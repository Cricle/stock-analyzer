//! News utilities: sentiment classification, deduplication, filtering, and ranking.

use std::collections::HashSet;

#[cfg(test)]
use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};

use super::NewsItem;

// ---------------------------------------------------------------------------
// Junk news filtering
// ---------------------------------------------------------------------------

/// Domains that are not financial news sources.
const PORTAL_DOMAINS: &[&str] = &[
    "baike.baidu.com",
    "jingyan.baidu.com",
    "iciba.com",
    "quote.eastmoney.com",
    "data.eastmoney.com",
    "so.eastmoney.com",
    "sina.com.cn/stock/",
    "finance.qq.com",
    "investing.com/equalities",
    "github.com",
    "stock.sina.com.cn",
];

/// Check if a news item is junk (dictionary entries, portal pages, non-financial content).
pub(crate) fn is_junk_news(item: &NewsItem) -> bool {
    let url = item.url.as_deref().unwrap_or("");
    if PORTAL_DOMAINS.iter().any(|d| url.contains(d)) {
        return true;
    }

    let title = &item.title;
    let lower = title.to_ascii_lowercase();

    // Chinese junk patterns
    if title.contains("是什么意思")
        || title.contains("翻译")
        || title.contains("的用法")
        || title.contains("百度经验")
        || title.contains("百度百科")
        || title.contains("百度知道")
        || title.contains("首页")
        || title.contains("官网")
        || title.contains("实时行情走势")
        || title.contains("行情中心")
        || title.contains("行情_")
        || title.contains("披露易")
        || title.contains("聪明的投资者")
        || title.contains("财经网")
        || title.contains("交易所")
        || title.contains("汇率")
        || title.contains("兑换")
        || title.contains("兌換")
        || title.contains("外币")
    {
        return true;
    }
    // English junk patterns — encyclopedia, how-to, homepages
    if lower.contains("how to")
        || lower.contains("what is")
        || lower.contains("wikipedia")
        || lower.contains("britannica")
        || lower.contains("meaning of")
        || lower.contains("google hangouts")
    {
        return true;
    }
    // Homepage patterns: title is just "{Brand} - Home" or "{Brand} Home"
    if lower.ends_with(" - home")
        || lower.ends_with(" | home")
        || lower == "home"
        || (lower.contains("home") && lower.len() < 30 && !lower.contains("stock"))
    {
        return true;
    }
    // Very short titles are not real news articles
    if title.trim().len() < 10 {
        return true;
    }
    // Index/quote pages (Chinese + English)
    if title.contains("恒生指数")
        || title.contains("恒生指數")
        || title.contains("恒生综合指数")
        || title.contains("经济通")
        || title.contains("經濟通")
        || lower.contains("hang seng index")
        || lower.contains("hsi)")
    {
        return true;
    }
    // Portal pages, bank portals, currency converters
    if lower.ends_with(" - select your location")
        || lower.contains("personal banking services")
        || lower.contains("global tire supply")
        || lower.contains("online banking")
        || lower.contains("currency converter")
        || lower.contains("exchange rate")
        || lower.contains("convert ") && lower.contains(" to ")
        || lower.contains("汇率")
    {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Sentiment classification
// ---------------------------------------------------------------------------

const HARD_NEGATIVE_KEYWORDS: &[&str] = &[
    "investigation",
    "fraud",
    "default",
    "bankruptcy",
    "delist",
    "downgrade",
    "lawsuit",
    "recall",
    "probe",
];

/// Check if a single news item contains hard negative keywords.
pub(crate) fn is_hard_negative(item: &NewsItem) -> bool {
    let title = item.title.to_ascii_lowercase();
    let summary = item.summary.to_ascii_lowercase();
    HARD_NEGATIVE_KEYWORDS
        .iter()
        .any(|keyword| title.contains(keyword) || summary.contains(keyword))
}

/// Check if any news item contains hard negative keywords.
#[cfg(test)]
pub(crate) fn has_hard_negative_news(news: &[NewsItem]) -> bool {
    news.iter().any(|item| {
        let title = item.title.to_ascii_lowercase();
        let summary = item.summary.to_ascii_lowercase();
        HARD_NEGATIVE_KEYWORDS
            .iter()
            .any(|keyword| title.contains(keyword) || summary.contains(keyword))
    })
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

/// Generate a deduplication key from news item fields.
pub(crate) fn news_dedupe_key(
    title: &str,
    source: &str,
    published_at: &str,
    url: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}",
        title.trim().to_ascii_lowercase(),
        source.trim().to_ascii_lowercase(),
        published_at.trim(),
        url.unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    )
}

/// Deduplicate news items by title+source+date+url, then sort by date descending.
pub(crate) fn dedupe_news_items(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut dedup = std::collections::HashSet::new();
    let mut output = Vec::new();
    for item in items {
        let key = news_dedupe_key(&item.title, &item.source, &item.published_at, item.url.as_deref());
        if dedup.insert(key) {
            output.push(item);
        }
    }
    output.sort_by(|left, right| right.published_at.cmp(&left.published_at));
    output
}

// ---------------------------------------------------------------------------
// Date filtering
// ---------------------------------------------------------------------------

pub(crate) fn within_date_window(
    published_at: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> bool {
    if published_at.trim().is_empty() {
        return true;
    }
    let Some(normalized) = normalized_news_date(published_at).or_else(|| {
        published_at
            .get(0..10)
            .filter(|prefix| prefix.len() == 10)
            .map(str::to_string)
    }) else {
        return true;
    };
    start_date.is_none_or(|start| normalized.as_str() >= start)
        && end_date.is_none_or(|end| normalized.as_str() <= end)
}

// ---------------------------------------------------------------------------
// Ranking and filtering
// ---------------------------------------------------------------------------

pub(crate) fn merge_ranked_news(
    items: Vec<NewsItem>,
    limit: usize,
    start_date: Option<&str>,
    end_date: Option<&str>,
    keywords: &[String],
) -> Vec<NewsItem> {
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        if !within_date_window(&item.published_at, start_date, end_date) {
            continue;
        }
        let dedupe_key = format!(
            "{}|{}",
            normalize_news_text(&item.title),
            normalized_news_date(&item.published_at).unwrap_or_else(|| item
                .published_at
                .get(0..10)
                .unwrap_or_default()
                .to_string())
        );
        if seen.insert(dedupe_key) {
            deduped.push(item);
        }
    }
    deduped.sort_by(|left, right| {
        let left_score = news_item_rank(left, keywords);
        let right_score = news_item_rank(right, keywords);
        right_score
            .cmp(&left_score)
            .then_with(|| right.published_at.cmp(&left.published_at))
            .then_with(|| left.title.cmp(&right.title))
    });
    deduped.truncate(limit.max(8));
    deduped
}

pub(crate) fn news_item_rank(item: &NewsItem, keywords: &[String]) -> i32 {
    let mut score = source_priority(&item.source);
    let normalized_title = normalize_news_text(&item.title);
    let normalized_summary = normalize_news_text(&item.summary);
    let combined = format!("{normalized_title} {normalized_summary}");
    let title_primary_keyword_hits = keywords
        .iter()
        .map(|value| normalize_news_text(value))
        .filter(|value| value.len() >= 2)
        .filter(|keyword| normalized_title.contains(keyword))
        .count();
    let combined_primary_keyword_hits = keywords
        .iter()
        .map(|value| normalize_news_text(value))
        .filter(|value| value.len() >= 2)
        .filter(|keyword| combined.contains(keyword))
        .count();
    if is_sec_filing_item(item) {
        score -= 20;
    }
    if title_or_summary_has_high_value_company_event(&normalized_title, &normalized_summary) {
        score += 34;
    }
    if title_or_summary_has_low_value_corporate_filing_noise(&normalized_title, &normalized_summary)
    {
        score -= 42;
    }
    if url_is_ir_landing_page(item.url.as_deref().unwrap_or_default()) {
        score -= 32;
    }
    for keyword in keywords
        .iter()
        .map(|value| normalize_news_text(value))
        .filter(|value| value.len() >= 2)
    {
        if is_sec_filing_item(item) && is_sec_biasing_keyword(&keyword) {
            continue;
        }
        if normalized_title.contains(&keyword) {
            score += 18;
        } else if combined.contains(&keyword) {
            score += 8;
        }
    }
    if item
        .url
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        score += 2;
    }
    if normalized_news_date(&item.published_at).is_some() {
        score += 4;
    }
    if title_is_generic_market_wrap(&normalized_title) {
        score -= 10;
    }
    if title_is_reference_or_overview_page(&normalized_title, &normalized_summary) {
        score -= 18;
    }
    if url_is_quote_or_overview_page(item.url.as_deref().unwrap_or_default()) {
        score -= 28;
    }
    if mentions_competitor_without_primary_company_focus(&normalized_title, &combined, keywords) {
        score -= 8;
    }
    if title_primary_keyword_hits == 0 && combined_primary_keyword_hits > 0 {
        score -= 12;
    }
    if title_primary_keyword_hits == 0
        && mentions_secondary_reference_only(&normalized_title, &normalized_summary, keywords)
    {
        score -= 10;
    }
    score
}

pub(crate) fn title_is_reference_or_overview_page(
    normalized_title: &str,
    normalized_summary: &str,
) -> bool {
    normalized_title.contains("stockoverview")
        || normalized_title.contains("marketdata")
        || normalized_title.contains("companyprofile")
        || normalized_title.contains("homepage")
        || normalized_title.contains("officialwebsite")
        || normalized_title.contains("latestnewsandupdates")
        || normalized_title.contains("realtimequotes")
        || normalized_title.contains("latestprice")
        || normalized_title.contains("stockprice")
        || normalized_title.contains("quote")
        || normalized_title.contains("homeoverview")
        || normalized_summary.contains("engagesinthedesigndevelopmentmanufactureandsale")
        || normalized_summary.contains("operatesthroughthe")
}

pub(crate) fn url_is_quote_or_overview_page(url: &str) -> bool {
    let normalized = url.trim().to_ascii_lowercase();
    let Ok(parsed) = reqwest::Url::parse(&normalized) else {
        return normalized.contains("quote.") || normalized.contains("stockpage.");
    };

    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path();

    host.starts_with("quote.")
        || host.starts_with("stockpage.")
        || host == "finance.yahoo.com" && path.starts_with("/quote/")
        || host == "hk.finance.yahoo.com" && path.starts_with("/quote/")
        || host == "tw.stock.yahoo.com" && path.starts_with("/quote/")
        || host == "www.nasdaq.com" && path.starts_with("/market-activity/stocks/")
        || host == "nasdaq.com" && path.starts_with("/market-activity/stocks/")
        || host == "finance.baidu.com" && path.starts_with("/stock/")
        || host == "stock.finance.sina.com.cn" && path.starts_with("/hkstock/quotes/")
        || host == "stock.finance.sina.com.cn" && path.starts_with("/usstock/quotes/")
        || host.ends_with("xueqiu.com") && path.starts_with("/s/")
        || host.ends_with("hstong.com") && path.starts_with("/quotes/")
        || host.ends_with("aastocks.com") && path.starts_with("/en/stocks/quote/")
        || host.ends_with("aastocks.com") && path.starts_with("/tc/stocks/quote/")
        || host.ends_with("etnet.com.hk") && path.ends_with("/quote.php")
}

pub(crate) fn url_is_ir_landing_page(url: &str) -> bool {
    let normalized = url.trim().to_ascii_lowercase();
    let Ok(parsed) = reqwest::Url::parse(&normalized) else {
        return normalized.contains("investor-relations/default")
            || normalized.ends_with("/investor-relations/")
            || normalized.ends_with("/investor-relations");
    };

    let host = parsed.host_str().unwrap_or_default();
    let path = parsed.path();

    (host.contains("investor.") || host.contains("ir."))
        && (path == "/"
            || path.ends_with("/default.aspx")
            || path.ends_with("/investor-relations/"))
        || path.ends_with("/investor-relations")
}

pub(crate) fn title_or_summary_has_high_value_company_event(
    normalized_title: &str,
    normalized_summary: &str,
) -> bool {
    let combined = format!("{normalized_title} {normalized_summary}");
    let markers = [
        "earnings",
        "quarterlyresults",
        "annualresults",
        "interimresults",
        "financialresults",
        "resultsannouncement",
        "businessupdate",
        "tradingupdate",
        "guidance",
        "buyback",
        "sharebuyback",
        "dividend",
        "delivery",
        "deliveries",
        "sales",
        "orders",
        "productlaunch",
        "业绩",
        "财报",
        "公告",
        "季报",
        "年报",
        "中报",
        "回购",
        "派息",
        "交付",
        "销量",
        "订单",
        "指引",
    ];
    markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)))
}

pub(crate) fn title_or_summary_has_low_value_corporate_filing_noise(
    normalized_title: &str,
    normalized_summary: &str,
) -> bool {
    let combined = format!("{normalized_title} {normalized_summary}");
    let markers = [
        "nextdaydisclosurereturn",
        "monthlyreturnofequityissuer",
        "pollresults",
        "annualgeneralmeeting",
        "proxyform",
        "formofproxy",
        "notificationletter",
        "circular",
        "changeofdirector",
        "listofdirectors",
        "closureofregisterofmembers",
        "independentdirectorcandidate",
        "statementandundertaking",
        "144filing",
    ];
    markers.iter().any(|marker| combined.contains(marker))
}

pub(crate) fn is_sec_filing_item(item: &NewsItem) -> bool {
    let normalized_title = normalize_news_text(&item.title);
    let normalized_source = item.source.to_ascii_lowercase();
    normalized_source.contains("sec")
        && (normalized_title.contains("filing")
            || normalized_title.contains("form")
            || normalized_title.contains("def14a")
            || normalized_title.contains("8-k")
            || normalized_title.contains("13d")
            || normalized_title.contains("proxy")
            || normalized_title.contains("ars")
            || normalized_title.contains("defa14a")
            || normalized_title.contains("px14a6g"))
}

pub(crate) fn is_sec_biasing_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "filing" | "sec" | "form" | "proxy" | "def14a" | "8-k" | "13d"
    )
}

pub(crate) fn title_is_generic_market_wrap(normalized_title: &str) -> bool {
    normalized_title.contains("dowjones")
        || normalized_title.contains("s&p500")
        || normalized_title.contains("stockmarket")
        || normalized_title.contains("markets")
        || normalized_title.contains("sensex")
        || normalized_title.contains("nifty")
}

pub(crate) fn mentions_competitor_without_primary_company_focus(
    normalized_title: &str,
    combined: &str,
    keywords: &[String],
) -> bool {
    let mentions_primary = keywords
        .iter()
        .map(|value| normalize_news_text(value))
        .filter(|value| !value.is_empty())
        .any(|keyword| normalized_title.contains(&keyword) || combined.contains(&keyword));
    let competitor_markers = [
        "cerebras", "apple", "intel", "amd", "broadcom", "samsung", "micron", "groq",
    ];
    let mentions_competitor = competitor_markers
        .iter()
        .any(|marker| normalized_title.contains(marker));
    mentions_competitor && !mentions_primary
}

pub(crate) fn mentions_secondary_reference_only(
    normalized_title: &str,
    normalized_summary: &str,
    keywords: &[String],
) -> bool {
    if !keywords
        .iter()
        .map(|value| normalize_news_text(value))
        .filter(|value| !value.is_empty())
        .any(|keyword| normalized_summary.contains(&keyword))
    {
        return false;
    }
    let secondary_markers = [
        "dominated by",
        "compared with",
        "competes with",
        "versus",
        "rival",
        "peer",
        "challenge to",
        "take on",
    ];
    secondary_markers
        .iter()
        .any(|marker| normalized_title.contains(marker) || normalized_summary.contains(marker))
}

pub(crate) fn source_priority(source: &str) -> i32 {
    let normalized = source.to_ascii_lowercase();
    if normalized.contains("ir.")
        || normalized.contains("investor")
        || normalized.contains("relations")
    {
        58
    } else if normalized.contains("hkex") || normalized.contains("hkexnews") {
        56
    } else if normalized.contains("reuters")
        || normalized.contains("bloomberg")
        || normalized.contains("ft")
        || normalized.contains("wsj")
        || normalized.contains("nikkei")
        || normalized.contains("cnbc")
    {
        52
    } else if normalized.contains("sec") {
        26
    } else if normalized.contains("aastocks") {
        46
    } else if normalized.contains("eastmoney") {
        44
    } else if normalized.contains("sse") {
        42
    } else if normalized.contains("etnet") {
        38
    } else if normalized.contains("futunn")
        || normalized.contains("hstong")
        || normalized.contains("xueqiu")
    {
        34
    } else if normalized.contains("google") {
        30
    } else {
        24
    }
}

// ---------------------------------------------------------------------------
// Text normalization
// ---------------------------------------------------------------------------

pub(crate) fn normalize_news_text(value: &str) -> String {
    value.split_whitespace().collect::<String>().to_lowercase()
}

pub fn normalized_news_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    [
        "%Y-%m-%d",
        "%Y%m%d",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%m/%d/%Y",
        "%m/%d/%Y %H:%M:%S",
        "%Y.%m.%d",
        "%Y.%m.%d %H:%M:%S",
        "%b %d, %Y",
        "%B %d, %Y",
        "%Y年%m月%d日",
        "%Y年%m月%d日 %H:%M",
        "%Y年%m月%d日 %H:%M:%S",
    ]
    .iter()
    .find_map(|format| {
        NaiveDate::parse_from_str(trimmed, format)
            .ok()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(trimmed, format)
                    .ok()
                    .map(|datetime| datetime.date().format("%Y-%m-%d").to_string())
            })
    })
    .or_else(|| {
        trimmed
            .get(0..10)
            .filter(|prefix| prefix.chars().nth(4) == Some('-'))
            .map(str::to_string)
    })
    .or_else(|| {
        // Try first token for formats with trailing time
        let first_token = trimmed.split_whitespace().next().unwrap_or("");
        if first_token != trimmed {
            [
                "%m/%d/%Y",
                "%Y.%m.%d",
                "%b %d, %Y",
                "%B %d, %Y",
            ]
            .iter()
            .find_map(|format| {
                NaiveDate::parse_from_str(first_token, format)
                    .ok()
                    .map(|date| date.format("%Y-%m-%d").to_string())
            })
        } else {
            None
        }
    })
    .or_else(|| normalize_relative_news_date(trimmed, Utc::now()))
}

pub(crate) fn normalize_relative_news_date(value: &str, now: DateTime<Utc>) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }

    if let Some((amount, unit)) = lower
        .split_once(' ')
        .and_then(|(amount, rest)| amount.parse::<i64>().ok().map(|value| (value, rest.trim())))
    {
        let date = if unit.starts_with("minute") || unit.starts_with("min") {
            Some((now - ChronoDuration::minutes(amount)).date_naive())
        } else if unit.starts_with("hour") || unit.starts_with("hr") {
            Some((now - ChronoDuration::hours(amount)).date_naive())
        } else if unit.starts_with("day") {
            Some((now - ChronoDuration::days(amount)).date_naive())
        } else if unit.starts_with("week") {
            Some((now - ChronoDuration::weeks(amount)).date_naive())
        } else {
            None
        }?;
        return Some(date.format("%Y-%m-%d").to_string());
    }

    if lower == "yesterday" {
        return Some(
            (now.date_naive() - ChronoDuration::days(1))
                .format("%Y-%m-%d")
                .to_string(),
        );
    }

    if lower == "today" {
        return Some(now.date_naive().format("%Y-%m-%d").to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_negative_detects_fraud() {
        let news = vec![NewsItem {
            title: "Company under investigation for fraud".to_string(),
            summary: String::new(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: None,
        }];
        assert!(has_hard_negative_news(&news));
    }

    #[test]
    fn hard_negative_no_false_positive() {
        let news = vec![NewsItem {
            title: "Company reports strong earnings".to_string(),
            summary: String::new(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: None,
        }];
        assert!(!has_hard_negative_news(&news));
    }

    #[test]
    fn dedupe_removes_duplicates() {
        let items = vec![
            NewsItem {
                title: "Same Title".to_string(),
                summary: "A".to_string(),
                source: "Reuters".to_string(),
                published_at: "2024-01-15".to_string(),
                url: None,
            },
            NewsItem {
                title: "Same Title".to_string(),
                summary: "B".to_string(),
                source: "Reuters".to_string(),
                published_at: "2024-01-15".to_string(),
                url: None,
            },
        ];
        let result = dedupe_news_items(items);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn dedupe_key_case_insensitive() {
        let key1 = news_dedupe_key("Hello World", "Reuters", "2024-01-15", None);
        let key2 = news_dedupe_key("hello world", "reuters", "2024-01-15", None);
        assert_eq!(key1, key2);
    }
}
