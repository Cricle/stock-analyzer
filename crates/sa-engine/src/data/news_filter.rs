use std::collections::HashSet;

#[cfg(test)]
use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};

use super::{NewsItem, wire};

pub(crate) fn build_dated_news_query(
    base: &str,
    _start_date: Option<&str>,
    _end_date: Option<&str>,
) -> String {
    base.trim().to_string()
}

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

pub(crate) fn news_search_dedup_key(item: &NewsItem) -> String {
    format!(
        "{}|{}|{}",
        item.title.trim().to_lowercase(),
        item.source.trim().to_lowercase(),
        item.url.clone().unwrap_or_default().trim().to_lowercase()
    )
}

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

pub(crate) fn is_investment_research_evidence_page(item: &NewsItem) -> bool {
    let normalized_title = normalize_news_text(&item.title);
    let normalized_summary = normalize_news_text(&item.summary);
    let normalized_source = item.source.to_ascii_lowercase();
    let normalized_url = item.url.as_deref().unwrap_or_default().to_ascii_lowercase();
    let combined = format!("{normalized_title} {normalized_summary}");

    let finance_hub_markers = [
        "eastmoney",
        "xueqiu",
        "sina.com.cn",
        "10jqka",
        "futunn",
        "stockstar",
        "hstong",
        "investing.com",
        "aastocks",
        "etnet",
        "cnevpost",
        "carnewschina",
        "finance.",
        "stock.",
        "quote.",
    ];
    let finance_hub_event_markers = [
        "业绩",
        "公告",
        "财报",
        "业绩快报",
        "中报",
        "年报",
        "季报",
        "交付",
        "销量",
        "订单",
        "指引",
        "投资者关系",
        "数据报告",
        "新闻",
        "研报",
        "评级",
        "investor relations",
        "earnings",
        "results",
        "quarterly results",
        "interim results",
        "annual results",
        "delivery",
        "deliveries",
        "order",
        "orders",
        "guidance",
        "research",
        "report",
        "announcement",
    ];
    let has_finance_hub_signal = finance_hub_markers
        .iter()
        .any(|marker| normalized_source.contains(marker) || normalized_url.contains(marker));
    let has_finance_hub_event_signal = finance_hub_event_markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)));
    let has_quote_or_overview_url =
        url_is_quote_or_overview_page(item.url.as_deref().unwrap_or_default());

    if title_is_reference_or_overview_page(&normalized_title, &normalized_summary)
        || title_is_generic_market_wrap(&normalized_title)
    {
        return false;
    }

    let source_markers = [
        "reuters",
        "bloomberg",
        "yahoo",
        "marketwatch",
        "benzinga",
        "investing",
        "seekingalpha",
        "fool",
        "barron",
        "morningstar",
        "ft",
        "wsj",
        "nikkei",
        "cnbc",
        "eastmoney",
        "etnet",
        "aastocks",
        "hkex",
        "nasdaq",
        "sec",
        "sse",
        "szse",
    ];
    let strong_url_markers = [
        "news",
        "article",
        "press-release",
        "announcement",
        "investor",
        "ir.",
        "earnings",
        "filing",
        "research",
        "report",
        "finance",
        "quote",
        "hkexnews.hk",
        "aastocks.com",
        "etnet.com.hk",
        "eastmoney.com",
    ];
    let event_markers = [
        "业绩",
        "公告",
        "财报",
        "业绩快报",
        "中报",
        "年报",
        "季报",
        "交付",
        "销量",
        "订单",
        "指引",
        "回购",
        "派息",
        "增发",
        "配股",
        "融资",
        "投资者关系",
        "研报",
        "评级",
        "target price",
        "earnings",
        "results",
        "quarterly results",
        "interim results",
        "annual results",
        "delivery",
        "deliveries",
        "order",
        "orders",
        "guidance",
        "analyst",
        "downgrade",
        "upgrade",
        "filing",
        "press release",
        "investor relations",
        "annual report",
        "quarterly report",
    ];
    let weak_reference_markers = [
        "quote", "行情", "股价", "概览", "overview", "profile", "homepage", "官网", "home", "wiki",
        "百科",
    ];
    let entertainment_noise_markers = [
        "白玉兰",
        "杨幂",
        "杨紫",
        "娱乐",
        "douyin",
        "weibo",
        "celebrity",
        "entertainment",
        "sport",
        "足球",
        "live-ticker",
    ];

    if entertainment_noise_markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)))
    {
        return false;
    }

    if weak_reference_markers
        .iter()
        .any(|marker| normalized_title.contains(&normalize_news_text(marker)))
        && !finance_hub_markers
            .iter()
            .any(|marker| normalized_source.contains(marker) || normalized_url.contains(marker))
    {
        return false;
    }

    let has_source_signal = source_markers
        .iter()
        .any(|marker| normalized_source.contains(marker));
    let has_url_signal = strong_url_markers
        .iter()
        .any(|marker| normalized_url.contains(marker));
    let has_event_signal = event_markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)));
    let has_ir_results_signal = (normalized_url.contains("investor")
        || normalized_url.contains("/ir")
        || normalized_url.contains("relations"))
        && (combined.contains("results")
            || combined.contains("earnings")
            || combined.contains("report")
            || combined.contains("announcement")
            || combined.contains(&normalize_news_text("业绩"))
            || combined.contains(&normalize_news_text("公告"))
            || combined.contains(&normalize_news_text("财报")));
    let has_finance_hub_article_signal = has_finance_hub_signal
        && has_finance_hub_event_signal
        && !has_quote_or_overview_url
        && (normalized_url.contains("news")
            || normalized_url.contains("article")
            || normalized_url.contains("/a/")
            || normalized_url.contains("/n/")
            || normalized_url.contains("research")
            || normalized_url.contains("report"));

    if has_quote_or_overview_url {
        return has_ir_results_signal;
    }

    (has_source_signal || has_url_signal) && has_event_signal
        || (has_source_signal && has_url_signal)
        || has_finance_hub_article_signal
        || has_ir_results_signal
}

pub(crate) fn is_macro_research_evidence_page(item: &NewsItem) -> bool {
    let normalized_title = normalize_news_text(&item.title);
    let normalized_summary = normalize_news_text(&item.summary);
    let normalized_source = item.source.to_ascii_lowercase();
    let normalized_url = item.url.as_deref().unwrap_or_default().to_ascii_lowercase();
    let combined = format!("{normalized_title} {normalized_summary}");

    if title_is_reference_or_overview_page(&normalized_title, &normalized_summary) {
        return false;
    }

    let entertainment_noise_markers = [
        "白玉兰",
        "杨幂",
        "杨紫",
        "娱乐",
        "douyin",
        "weibo",
        "celebrity",
        "entertainment",
        "sport",
        "足球",
        "live-ticker",
    ];
    if entertainment_noise_markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)))
    {
        return false;
    }

    let macro_source_markers = [
        "reuters",
        "bloomberg",
        "ft",
        "wsj",
        "cnbc",
        "nikkei",
        "yahoo",
        "marketwatch",
        "investing",
        "aastocks",
        "etnet",
        "eastmoney",
        "stcn",
        "caixin",
        "finance",
        "cctv",
        "chinanews",
        "gov.cn",
        "china.com.cn",
    ];
    let macro_url_markers = [
        "news",
        "article",
        "markets",
        "economy",
        "macro",
        "policy",
        "finance",
        "business",
        "gov.cn",
        "cctv.com",
        "chinanews.com.cn",
        "china.com.cn",
    ];
    let macro_event_markers = [
        "宏观",
        "经济",
        "政策",
        "利率",
        "通胀",
        "流动性",
        "汇率",
        "人民币",
        "港股",
        "恒生",
        "科技股",
        "中国互联网",
        "risk sentiment",
        "market",
        "economy",
        "policy",
        "yield",
        "inflation",
        "liquidity",
        "hong kong",
        "china tech",
        "federal reserve",
        "tariff",
        "时政",
        "发布会",
        "刺激",
        "消费",
        "PMI",
        "manufacturing",
        "outlook",
        "equities",
        "新能源车",
        "电动车",
        "汽车",
        "智驾",
        "补贴",
        "以旧换新",
        "内需",
        "出口",
        "关税",
        "ev",
        "electric vehicle",
        "auto",
        "autos",
        "subsidy",
        "consumer",
        "stimulus",
    ];

    let has_source_signal = macro_source_markers
        .iter()
        .any(|marker| normalized_source.contains(marker));
    let has_url_signal = macro_url_markers
        .iter()
        .any(|marker| normalized_url.contains(marker));
    let has_event_signal = macro_event_markers
        .iter()
        .any(|marker| combined.contains(&normalize_news_text(marker)));

    has_event_signal && (has_source_signal || has_url_signal)
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
    } else if normalized.contains("tushare") {
        28
    } else {
        24
    }
}

pub(crate) fn extract_site_name_from_url(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme
        .split('/')
        .next()?
        .trim()
        .trim_start_matches("www.");
    if host.is_empty() { None } else { Some(host) }
}

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
    .or_else(|| normalize_relative_news_date(trimmed, Utc::now()))
}

pub(crate) fn gdelt_timestamp_to_published_at(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() < 14 || !trimmed.chars().take(14).all(|ch| ch.is_ascii_digit()) {
        return trimmed.to_string();
    }
    format!(
        "{}-{}-{} {}:{}:{}",
        &trimmed[0..4],
        &trimmed[4..6],
        &trimmed[6..8],
        &trimmed[8..10],
        &trimmed[10..12],
        &trimmed[12..14]
    )
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

pub(crate) fn latest_metric_value(units: &wire::MetricUnits) -> Option<f64> {
    latest_preferred_metric(units, MetricSelection::LatestFiled).map(|item| item.val)
}

pub(crate) fn latest_strict_annual_metric_value(units: &wire::MetricUnits) -> Option<f64> {
    latest_preferred_metric(units, MetricSelection::StrictAnnual).map(|item| item.val)
}

pub(crate) fn latest_annual_metric_value(units: &wire::MetricUnits) -> Option<f64> {
    latest_preferred_metric(units, MetricSelection::Annual).map(|item| item.val)
}

pub(crate) fn latest_instant_metric_value(units: &wire::MetricUnits) -> Option<f64> {
    latest_preferred_metric(units, MetricSelection::Instant).map(|item| item.val)
}

#[derive(Clone, Copy)]
pub(crate) enum MetricSelection {
    LatestFiled,
    StrictAnnual,
    Annual,
    Instant,
}

pub(crate) fn latest_preferred_metric(
    units: &wire::MetricUnits,
    selection: MetricSelection,
) -> Option<&wire::MetricValue> {
    units
        .usd
        .as_ref()
        .or(units.shares.as_ref())
        .and_then(|values| {
            values
                .iter()
                .filter(|item| matches!(item.form.as_deref(), Some("10-K") | Some("10-Q")))
                .filter(|item| match selection {
                    MetricSelection::LatestFiled => true,
                    MetricSelection::StrictAnnual | MetricSelection::Annual => {
                        item.start.is_some()
                            && item.end.is_some()
                            && matches!(item.form.as_deref(), Some("10-K"))
                            && matches!(item.fp.as_deref(), Some("FY"))
                    }
                    MetricSelection::Instant => item.start.is_none() && item.end.is_some(),
                })
                .max_by_key(|item| {
                    (
                        item.end.as_deref().unwrap_or_default(),
                        item.filed.as_str(),
                        item.fp.as_deref().unwrap_or_default(),
                    )
                })
        })
        .or_else(|| {
            matches!(selection, MetricSelection::Annual).then(|| {
                units
                    .usd
                    .as_ref()
                    .or(units.shares.as_ref())
                    .and_then(|values| {
                        values
                            .iter()
                            .filter(|item| {
                                matches!(item.form.as_deref(), Some("10-K") | Some("10-Q"))
                                    && item.start.is_some()
                                    && item.end.is_some()
                            })
                            .max_by_key(|item| {
                                (
                                    item.end.as_deref().unwrap_or_default(),
                                    item.filed.as_str(),
                                    item.fp.as_deref().unwrap_or_default(),
                                )
                            })
                    })
            })?
        })
}

/// Permissive relevance filter for guidance news.
/// Rejects obvious noise but accepts any page with financial market content.
/// Unlike `is_macro_research_evidence_page`, does NOT require matching source/URL markers.
pub(crate) fn is_guidance_relevant_news(item: &NewsItem) -> bool {
    let url = item.url.as_deref().unwrap_or_default();
    let url_lower = url.to_ascii_lowercase();

    // Extract domain from URL for domain-based filtering
    let domain = extract_domain(url);

    // Reject known non-news domains (dictionaries, portals, reference sites)
    let rejected_domains = [
        // Dictionary/translation sites
        "cambridge.org", "iciba.com", "youdao.com", "merriam-webster.com",
        "oxfordlearnersdictionaries.com", "collinsdictionary.com",
        "dictionary.com", "vocabulary.com", "wordreference.com",
        // Portal homepages (not article pages)
        "hkex.com.hk", "hkma.gov.hk", "sse.com.cn", "szse.cn",
        "investopedia.com", "marketwatch.com",
        // Reference/encyclopedia sites
        "baike.baidu.com", "wikipedia.org", "wikimedia.org",
        // Government sites
        "gov.cn", "gov.hk", "gov.uk", "gov.au", "gov.in",
    ];
    if rejected_domains.iter().any(|d| domain.ends_with(d)) {
        return false;
    }

    // Reject URLs that look like portal homepages (short paths)
    if let Some(path) = extract_url_path(url) {
        let path_lower = path.to_ascii_lowercase();
        // Reject root paths or very short paths (likely homepages)
        if path_lower == "/" || path_lower.is_empty() || path_lower.len() < 10 {
            // But allow if it's clearly an article path
            if !path_lower.contains("/article") && !path_lower.contains("/news")
                && !path_lower.contains("/story") && !path_lower.contains("/post")
            {
                return false;
            }
        }
    }

    // Reject reference/overview pages by URL pattern
    if url_is_quote_or_overview_page(url) {
        return false;
    }

    // For Bing RSS items, apply stricter filtering
    if item.source == "bing_rss" {
        // Reject if URL contains dictionary/translation patterns
        let dict_patterns = [
            "/dictionary", "/translate", "/词汇", "/词典", "/翻译",
            "/definition", "/meaning", "/用法", "/例句",
        ];
        if dict_patterns.iter().any(|p| url_lower.contains(p)) {
            return false;
        }

        // Reject if URL contains portal/overview patterns
        let portal_patterns = [
            "/homepage", "/首页", "/index.html", "/main.html",
            "/overview", "/概览", "/market-overview",
        ];
        if portal_patterns.iter().any(|p| url_lower.contains(p)) {
            return false;
        }
    }

    // Accept if source is a known financial news source
    let trusted_sources = [
        "CLS 财联社", "THS 同花顺", "Sina 新浪", "Futu 富途",
        "reuters.com", "bloomberg.com", "wsj.com", "ft.com",
        "cnbc.com", "seekingalpha.com", "finance.yahoo.com",
        "eastmoney.com", "10jqka.com.cn", "stockstar.com",
    ];
    if trusted_sources.contains(&item.source.as_str()) {
        return true;
    }

    // For other sources, check if URL looks like an article (has meaningful path)
    if let Some(path) = extract_url_path(url) {
        let path_lower = path.to_ascii_lowercase();
        // Article URLs typically have longer paths with dates or IDs
        if path_lower.len() > 20 && (path_lower.contains("2026") || path_lower.contains("2025")
            || path_lower.contains("/article") || path_lower.contains("/news")
            || path_lower.contains("/story") || path_lower.matches('/').count() >= 3)
        {
            return true;
        }
    }

    // Default: reject (be conservative for non-trusted sources)
    false
}

/// Extract domain from URL (e.g., "https://www.example.com/path" -> "example.com")
fn extract_domain(url: &str) -> String {
    let url = url.trim().to_ascii_lowercase();
    // Remove protocol
    let without_protocol = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        &url
    };
    // Remove path
    let domain = if let Some(pos) = without_protocol.find('/') {
        &without_protocol[..pos]
    } else {
        without_protocol
    };
    // Remove www. prefix
    let domain = domain.strip_prefix("www.").unwrap_or(domain);
    domain.to_string()
}

/// Extract path from URL
fn extract_url_path(url: &str) -> Option<String> {
    let url = url.trim();
    let without_protocol = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    without_protocol.find('/').map(|pos| without_protocol[pos..].to_string())
}
