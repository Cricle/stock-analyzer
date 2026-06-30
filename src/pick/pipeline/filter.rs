use std::collections::HashSet;

use futures::{StreamExt, stream};

use crate::StockPickRequest;
use crate::data::{BillboardEntry, CapitalFlowPoint, MarketDataClient, MarketKind};

use super::CandidateContext;

/// Rotate a slice by a deterministic offset derived from current time.
/// Ensures different stocks are picked each run without full randomness.
fn rotate_by_time<T>(items: &mut Vec<T>) {
    let offset = (chrono::Utc::now().timestamp_millis() as usize) % items.len().max(1);
    items.rotate_left(offset);
}

// ---------------------------------------------------------------------------
// Hardcoded fallback stocks for when external APIs are unavailable
// ---------------------------------------------------------------------------

fn fallback_a_share_stocks() -> Vec<(&'static str, &'static str)> {
    vec![
        // 大盘蓝筹
        ("600519", "贵州茅台"), ("601318", "中国平安"), ("600036", "招商银行"),
        ("000858", "五粮液"), ("601166", "兴业银行"), ("600276", "恒瑞医药"),
        ("000333", "美的集团"), ("600030", "中信证券"), ("601398", "工商银行"),
        ("000001", "平安银行"), ("600900", "长江电力"), ("601088", "中国神华"),
        // 新能源 / 电池
        ("300750", "宁德时代"), ("002594", "比亚迪"), ("601012", "隆基绿能"),
        ("300274", "阳光电源"), ("002709", "天赐材料"), ("300014", "亿纬锂能"),
        // 半导体 / 芯片
        ("688981", "中芯国际"), ("002371", "北方华创"), ("603501", "韦尔股份"),
        ("688008", "澜起科技"), ("300661", "圣邦股份"), ("688012", "中微公司"),
        // 消费 / 医药
        ("600887", "伊利股份"), ("000651", "格力电器"), ("601888", "中国中免"),
        ("002714", "牧原股份"), ("300015", "爱尔眼科"), ("000568", "泸州老窖"),
        ("603259", "药明康德"), ("002304", "洋河股份"), ("600132", "重庆啤酒"),
        // 科创 / 中小盘
        ("002415", "海康威视"), ("300059", "东方财富"), ("688111", "金山办公"),
        ("002230", "科大讯飞"), ("300760", "迈瑞医疗"), ("688169", "石头科技"),
        ("300124", "汇川技术"), ("002049", "紫光国微"), ("688036", "传音控股"),
        // 周期 / 工业
        ("601899", "紫金矿业"), ("002460", "赣锋锂业"), ("600585", "海螺水泥"),
        ("601668", "中国建筑"), ("600031", "三一重工"), ("000776", "广发证券"),
        // 军工 / 新兴
        ("600760", "中航沈飞"), ("688599", "天合光能"), ("300450", "先导智能"),
        ("601127", "赛力斯"), ("300782", "卓胜微"), ("688256", "寒武纪"),
    ]
}

fn fallback_hk_stocks() -> Vec<(&'static str, &'static str)> {
    vec![
        // 科技互联网
        ("0700", "腾讯控股"), ("09988", "阿里巴巴-SW"), ("03690", "美团-W"),
        ("01810", "小米集团-W"), ("09618", "京东集团-SW"), ("09888", "百度集团-SW"),
        ("09999", "网易-S"), ("01024", "快手-W"), ("09626", "哔哩哔哩-SW"),
        ("02015", "理想汽车-W"), ("09866", "蔚来-SW"), ("09868", "小鹏汽车-W"),
        // 金融
        ("00005", "汇丰控股"), ("01299", "友邦保险"), ("00388", "香港交易所"),
        ("02318", "中国平安"), ("03988", "中国银行"), ("01398", "工商银行"),
        ("00939", "建设银行"), ("02628", "中国人寿"), ("00011", "恒生银行"),
        ("02388", "中银香港"), ("01658", "邮储银行"), ("06837", "海通证券"),
        // 消费
        ("02020", "安踏体育"), ("01211", "比亚迪股份"), ("06862", "海底捞"),
        ("09961", "携程集团-S"), ("02331", "李宁"), ("01928", "金沙中国"),
        ("00027", "银河娱乐"), ("06969", "思摩尔国际"), ("01458", "周黑鸭"),
        // 医药生物
        ("02269", "药明生物"), ("01177", "中国生物制药"), ("02196", "复星医药"),
        ("06160", "百济神州"), ("01801", "信达生物"), ("09995", "荣昌生物"),
        // 能源 / 工业
        ("00883", "中国海洋石油"), ("00941", "中国移动"), ("03968", "招商银行"),
        ("00267", "中信股份"), ("01109", "华润置地"), ("02007", "碧桂园"),
        ("00175", "吉利汽车"), ("03888", "金山软件"), ("00285", "比亚迪电子"),
        // 中小盘 / 新兴
        ("06060", "众安在线"), ("09698", "万国数据-SW"), ("06618", "京东健康"),
        ("02518", "汽车之家-S"), ("09901", "新东方在线"), ("01833", "平安好医生"),
        ("00772", "阅文集团"), ("09969", "诺辉健康-B"), ("02126", "药师帮"),
    ]
}

fn fallback_us_stocks() -> Vec<(&'static str, &'static str)> {
    vec![
        // 大盘科技
        ("AAPL", "Apple Inc."), ("MSFT", "Microsoft Corp."), ("GOOGL", "Alphabet Inc."),
        ("AMZN", "Amazon.com Inc."), ("NVDA", "NVIDIA Corp."), ("META", "Meta Platforms Inc."),
        ("TSLA", "Tesla Inc."), ("AMD", "Advanced Micro Devices"), ("AVGO", "Broadcom Inc."),
        ("INTC", "Intel Corp."), ("QCOM", "Qualcomm Inc."), ("MU", "Micron Technology"),
        // 金融
        ("BRK-B", "Berkshire Hathaway"), ("JPM", "JPMorgan Chase & Co."), ("V", "Visa Inc."),
        ("MA", "Mastercard Inc."), ("BAC", "Bank of America Corp."), ("GS", "Goldman Sachs"),
        ("MS", "Morgan Stanley"), ("AXP", "American Express"), ("C", "Citigroup Inc."),
        // 医药健康
        ("JNJ", "Johnson & Johnson"), ("UNH", "UnitedHealth Group"), ("PFE", "Pfizer Inc."),
        ("ABBV", "AbbVie Inc."), ("MRK", "Merck & Co."), ("LLY", "Eli Lilly"),
        ("TMO", "Thermo Fisher Scientific"), ("ABT", "Abbott Laboratories"),
        // 消费
        ("WMT", "Walmart Inc."), ("PG", "Procter & Gamble Co."), ("HD", "Home Depot Inc."),
        ("DIS", "Walt Disney Co."), ("NFLX", "Netflix Inc."), ("COST", "Costco Wholesale"),
        ("NKE", "Nike Inc."), ("SBUX", "Starbucks Corp."), ("MCD", "McDonald's Corp."),
        // 工业 / 能源
        ("CAT", "Caterpillar Inc."), ("BA", "Boeing Co."), ("XOM", "Exxon Mobil"),
        ("CVX", "Chevron Corp."), ("GE", "GE Aerospace"), ("HON", "Honeywell International"),
        ("UPS", "United Parcel Service"), ("RTX", "RTX Corp."),
        // 中盘成长
        ("CRM", "Salesforce Inc."), ("NOW", "ServiceNow Inc."), ("PLTR", "Palantir Technologies"),
        ("SNOW", "Snowflake Inc."), ("COIN", "Coinbase Global"), ("SQ", "Block Inc."),
        ("UBER", "Uber Technologies"), ("ABNB", "Airbnb Inc."), ("DKNG", "DraftKings"),
        ("RIVN", "Rivian Automotive"), ("SMCI", "Super Micro Computer"), ("ARM", "Arm Holdings"),
        // 中概 / 新兴
        ("PDD", "PDD Holdings"), ("BABA", "Alibaba Group"), ("JD", "JD.com Inc."),
        ("NIO", "NIO Inc."), ("LI", "Li Auto Inc."), ("XPEV", "XPeng Inc."),
        ("BIDU", "Baidu Inc."), ("NTES", "NetEase Inc."),
    ]
}

// ---------------------------------------------------------------------------
// Market helpers
// ---------------------------------------------------------------------------

pub fn market_kind_from_value(value: &str) -> MarketKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "a" | "a-share" | "a_share" | "ashare" | "cn" | "china" | "a股" => MarketKind::AShare,
        "hk" | "hkex" | "hongkong" | "hong_kong" | "港股" => MarketKind::HongKong,
        _ => MarketKind::UsEquity,
    }
}

pub fn market_display_label(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "A-share",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

pub fn market_search_label(market: MarketKind) -> &'static str {
    market_display_label(market)
}

pub fn market_exchange_code(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "CN",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

fn default_market_candidate_query(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "industry",
        MarketKind::HongKong => "腾讯",
        MarketKind::UsEquity => "technology",
    }
}

/// Decode response bytes as GBK (Sina Finance uses GBK encoding).
async fn decode_gbk_response(resp: reqwest::Response) -> anyhow::Result<String> {
    let bytes = resp.bytes().await?;
    let (decoded, _encoding, _had_errors) = encoding_rs::GBK.decode(&bytes);
    Ok(decoded.into_owned())
}

/// Parse Sina sector rankings from the JS response.
/// Format: var S_Finance_bankuai_sinaindustry = {"key":"code,name,count,avg_price,change_pct,...",...}
async fn fetch_sina_sector_rankings(http: &reqwest::Client, limit: usize) -> anyhow::Result<Vec<(String, String, f64)>> {
    let resp = http
        .get("https://vip.stock.finance.sina.com.cn/q/view/newSinaHy.php")
        .send()
        .await?;
    let resp = decode_gbk_response(resp).await?;

    let Some(start) = resp.find('{') else {
        anyhow::bail!("sina sector: no JSON found");
    };
    let Some(end) = resp.rfind('}') else {
        anyhow::bail!("sina sector: no closing brace");
    };
    let json_str = &resp[start..=end];
    let obj: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("sina sector JSON parse error: {e}"))?;

    let mut sectors: Vec<(String, String, f64)> = Vec::new();
    if let Some(map) = obj.as_object() {
        for (_key, val) in map {
            if let Some(s) = val.as_str() {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() >= 5 {
                    let code = parts[0].to_string();
                    let name = parts[1].to_string();
                    let change_pct: f64 = parts[4].parse().unwrap_or(0.0);
                    sectors.push((code, name, change_pct));
                }
            }
        }
    }
    sectors.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    sectors.truncate(limit);
    Ok(sectors)
}

/// Fetch top stocks from a Sina sector.
async fn fetch_sina_sector_stocks(http: &reqwest::Client, sector_code: &str) -> anyhow::Result<Vec<(String, String)>> {
    let url = format!(
        "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData?page=1&num=8&sort=changepercent&asc=0&node={}&symbol=&_s_r_a=init",
        sector_code
    );
    let resp = http.get(&url).send().await?;
    let text = decode_gbk_response(resp).await?;

    let mut stocks = Vec::new();
    // Parse JSON array: [{"symbol":"sh600519","name":"贵州茅台",...},...]
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
        for item in arr {
            let symbol = item.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !symbol.is_empty() {
                stocks.push((symbol, name));
            }
        }
    }
    Ok(stocks)
}

// ---------------------------------------------------------------------------
// Candidate resolution
// ---------------------------------------------------------------------------

pub(super) async fn resolve_candidates(
    market_data: &MarketDataClient,
    request: &StockPickRequest,
    candidate_limit: usize,
) -> anyhow::Result<Vec<CandidateContext>> {
    if let Some(symbols) = request
        .candidate_symbols
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        return Ok(symbols
            .iter()
            .map(|symbol| {
                let normalized = symbol.trim().to_uppercase();
                let market_kind = market_data.detect_market(&normalized);
                CandidateContext {
                    symbol: normalized.clone(),
                    name: normalized,
                    market: market_display_label(market_kind).to_string(),
                    exchange: market_exchange_code(market_kind).to_string(),
                    source_score: 0.0,
                }
            })
            .collect());
    }

    let market_kind = market_kind_from_value(&request.market);
    tracing::info!(market = ?market_kind, "resolving candidates");
    match market_kind {
        MarketKind::AShare => {
            resolve_a_share_candidates(market_data, request, candidate_limit).await
        }
        MarketKind::HongKong => {
            let mut all_items = Vec::new();
            for query in ["腾讯", "阿里", "美团", "小米", "比亚迪", "汇丰", "友邦", "港交所"] {
                let items = market_data
                    .search_stocks(
                        query,
                        Some(market_search_label(market_kind)),
                        candidate_limit,
                    )
                    .await
                    .unwrap_or_default();
                all_items.extend(items);
            }
            // Fallback to hardcoded list if search returns nothing
            if all_items.is_empty() {
                tracing::warn!("HK search returned no results, using fallback blue-chip list");
                let mut stocks = fallback_hk_stocks();
                rotate_by_time(&mut stocks);
                return Ok(stocks
                    .into_iter()
                    .take(candidate_limit)
                    .map(|(symbol, name)| CandidateContext {
                        symbol: symbol.to_string(),
                        name: name.to_string(),
                        market: market_display_label(MarketKind::HongKong).to_string(),
                        exchange: market_exchange_code(MarketKind::HongKong).to_string(),
                        source_score: 0.0,
                    })
                    .collect());
            }
            Ok(all_items
                .into_iter()
                .map(|item| CandidateContext {
                    symbol: item.symbol,
                    name: item.name,
                    market: item.market,
                    exchange: item.exchange,
                    source_score: 0.0,
                })
                .collect())
        }
        MarketKind::UsEquity => {
            let query = request
                .sector_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_market_candidate_query(market_kind));
            let items = market_data
                .search_stocks(
                    query,
                    Some(market_search_label(market_kind)),
                    candidate_limit,
                )
                .await
                .unwrap_or_default();
            // Fallback to hardcoded list if search returns nothing
            if items.is_empty() {
                tracing::warn!("US search returned no results, using fallback stock list");
                let mut stocks = fallback_us_stocks();
                rotate_by_time(&mut stocks);
                return Ok(stocks
                    .into_iter()
                    .take(candidate_limit)
                    .map(|(symbol, name)| CandidateContext {
                        symbol: symbol.to_string(),
                        name: name.to_string(),
                        market: market_display_label(MarketKind::UsEquity).to_string(),
                        exchange: market_exchange_code(MarketKind::UsEquity).to_string(),
                        source_score: 0.0,
                    })
                    .collect());
            }
            Ok(items
                .into_iter()
                .map(|item| CandidateContext {
                    symbol: item.symbol,
                    name: item.name,
                    market: item.market,
                    exchange: item.exchange,
                    source_score: 0.0,
                })
                .collect())
        }
    }
}

async fn resolve_a_share_candidates(
    market_data: &MarketDataClient,
    request: &StockPickRequest,
    candidate_limit: usize,
) -> anyhow::Result<Vec<CandidateContext>> {
    let preferred_sector_type = request.sector_type.as_deref().unwrap_or("industry");
    let secondary_sector_type = if preferred_sector_type == "industry" {
        "concept"
    } else {
        "industry"
    };

    let mut sector_types = vec![preferred_sector_type];
    if secondary_sector_type != preferred_sector_type {
        sector_types.push(secondary_sector_type);
    }

    let sector_limit = candidate_limit.clamp(6, 16);
    let per_sector_constituents = candidate_limit.clamp(5, 8);
    let mut ranked_sectors = Vec::new();

    for sector_type in sector_types {
        let sectors = market_data
            .fetch_a_share_sector_rankings(sector_type, sector_limit)
            .await
            .unwrap_or_default();

        tracing::info!(sector_type, sectors_count = sectors.len(), "fetched sector rankings from eastmoney");

        // Sort by change_pct (Sina doesn't provide main_net_inflow)
        let mut by_change = sectors.clone();
        by_change.sort_by(|left, right| {
            right
                .change_pct
                .partial_cmp(&left.change_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked_sectors.extend(by_change.into_iter().take(4));

        // Also take top sectors by main_net_inflow if available
        let mut by_inflow = sectors;
        by_inflow.sort_by(|left, right| {
            right
                .main_net_inflow
                .partial_cmp(&left.main_net_inflow)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked_sectors.extend(by_inflow.into_iter().take(4));
    }

    // Fallback to Sina sector rankings if eastmoney returns nothing
    if ranked_sectors.is_empty() {
        tracing::warn!("eastmoney sector rankings empty, trying Sina fallback");
        let http = reqwest::Client::new();
        if let Ok(sina_sectors) = fetch_sina_sector_rankings(&http, sector_limit).await {
            tracing::info!(count = sina_sectors.len(), "fetched Sina sector rankings");
            for (code, name, change_pct) in sina_sectors {
                ranked_sectors.push(crate::types::SectorSnapshot {
                    sector_code: code,
                    sector_name: name,
                    latest_index: 0.0,
                    change_pct,
                    main_net_inflow: 0.0,
                    main_net_inflow_ratio_pct: 0.0,
                });
            }
        }
    }

    let mut sector_seen = HashSet::new();
    let mut sector_candidates = Vec::new();
    for sector in &ranked_sectors {
        if !sector_seen.insert(sector.sector_code.clone()) {
            continue;
        }

        // Try eastmoney constituents first
        let constituents = market_data
            .fetch_a_share_sector_constituents(&sector.sector_code, per_sector_constituents)
            .await
            .unwrap_or_default();

        if !constituents.is_empty() {
            let mut by_inflow = constituents.clone();
            by_inflow.sort_by(|left, right| {
                right
                    .main_net_inflow
                    .unwrap_or_default()
                    .partial_cmp(&left.main_net_inflow.unwrap_or_default())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        right
                            .change_pct
                            .partial_cmp(&left.change_pct)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
            sector_candidates.extend(by_inflow.into_iter().take(3).map(|constituent| {
                CandidateContext {
                    symbol: constituent.symbol,
                    name: constituent.name,
                    market: market_display_label(MarketKind::AShare).to_string(),
                    exchange: market_exchange_code(MarketKind::AShare).to_string(),
                    source_score: constituent.main_net_inflow.unwrap_or_default() / 1_0000_0000.0
                        + constituent.change_pct.max(0.0),
                }
            }));

            let mut by_change = constituents;
            by_change.sort_by(|left, right| {
                right
                    .change_pct
                    .partial_cmp(&left.change_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        right
                            .main_net_inflow
                            .unwrap_or_default()
                            .partial_cmp(&left.main_net_inflow.unwrap_or_default())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
            sector_candidates.extend(by_change.into_iter().take(2).map(|constituent| {
                CandidateContext {
                    symbol: constituent.symbol,
                    name: constituent.name,
                    market: market_display_label(MarketKind::AShare).to_string(),
                    exchange: market_exchange_code(MarketKind::AShare).to_string(),
                    source_score: constituent.change_pct
                        + constituent.main_net_inflow.unwrap_or_default() / 2_0000_0000.0,
                }
            }));
        } else {
            // Fallback: try Sina sector stocks
            let http = reqwest::Client::new();
            if let Ok(stocks) = fetch_sina_sector_stocks(&http, &sector.sector_code).await {
                tracing::info!(sector = %sector.sector_code, count = stocks.len(), "fetched Sina sector stocks");
                for (symbol, name) in stocks.into_iter().take(3) {
                    sector_candidates.push(CandidateContext {
                        symbol,
                        name,
                        market: market_display_label(MarketKind::AShare).to_string(),
                        exchange: market_exchange_code(MarketKind::AShare).to_string(),
                        source_score: sector.change_pct.max(0.0),
                    });
                }
            }
        }
    }

    let mut search_candidates = Vec::new();
    for query in [
        "600", "000", "300", "688",
        "贵州", "中国", "科技", "银行", "能源", "医药",
    ] {
        let items = market_data
            .search_stocks(
                query,
                Some(market_search_label(MarketKind::AShare)),
                candidate_limit.clamp(5, 8),
            )
            .await
            .unwrap_or_default();
        search_candidates.extend(items.into_iter().map(|item| CandidateContext {
            symbol: item.symbol,
            name: item.name,
            market: item.market,
            exchange: item.exchange,
            source_score: 1.0,
        }));
    }

    let mut all_candidates = Vec::new();
    all_candidates.extend(sector_candidates);
    all_candidates.extend(search_candidates);

    // Final fallback: use hardcoded list if everything else failed
    if all_candidates.is_empty() {
        tracing::warn!("all A-share candidate sources empty, using hardcoded fallback list");
        let mut stocks = fallback_a_share_stocks();
        rotate_by_time(&mut stocks);
        return Ok(stocks
            .into_iter()
            .take(candidate_limit)
            .map(|(symbol, name)| CandidateContext {
                symbol: symbol.to_string(),
                name: name.to_string(),
                market: market_display_label(MarketKind::AShare).to_string(),
                exchange: market_exchange_code(MarketKind::AShare).to_string(),
                source_score: 0.0,
            })
            .collect());
    }

    let all_candidates = dedup_candidates(all_candidates, candidate_limit.saturating_mul(4));
    tracing::info!(all_candidates_count = all_candidates.len(), "deduped all candidates");
    let shortlist = shortlist_a_share_candidates_for_flow(all_candidates, candidate_limit);
    tracing::info!(shortlist_count = shortlist.len(), "shortlisted candidates");
    let result = pre_rank_a_share_candidates(market_data, shortlist, candidate_limit)
        .await
        .into_iter()
        .take(candidate_limit)
        .collect::<Vec<_>>();
    tracing::info!(final_count = result.len(), "final candidates");
    Ok(result)
}

fn dedup_candidates(items: Vec<CandidateContext>, limit: usize) -> Vec<CandidateContext> {
    let mut seen = HashSet::new();
    let mut output = Vec::with_capacity(limit);
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

pub(crate) fn shortlist_a_share_candidates_for_flow(
    mut candidates: Vec<CandidateContext>,
    candidate_limit: usize,
) -> Vec<CandidateContext> {
    candidates.sort_by(|left, right| {
        right
            .source_score
            .partial_cmp(&left.source_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });

    let expensive_window = candidate_limit.saturating_mul(2).clamp(8, 18);
    candidates.truncate(expensive_window.min(candidates.len()));
    candidates
}

async fn pre_rank_a_share_candidates(
    market_data: &MarketDataClient,
    candidates: Vec<CandidateContext>,
    candidate_limit: usize,
) -> Vec<CandidateContext> {
    let mut ranked = stream::iter(candidates.into_iter())
        .map(|candidate| {
            let market_data = market_data.clone();
            async move {
                let capital_flow = market_data
                    .fetch_capital_flow(&candidate.symbol, 2)
                    .await
                    .unwrap_or_default();
                let billboard = market_data
                    .fetch_billboard_entries(&candidate.symbol, 2)
                    .await
                    .unwrap_or_default();
                let score = candidate.source_score
                    + capital_flow_source_score(&capital_flow)
                    + billboard_source_score(&billboard);
                CandidateContext {
                    source_score: score,
                    ..candidate
                }
            }
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;

    ranked.sort_by(|left, right| {
        right
            .source_score
            .partial_cmp(&left.source_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    dedup_candidates(ranked, candidate_limit)
}

// ---------------------------------------------------------------------------
// Source score helpers
// ---------------------------------------------------------------------------

pub fn capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let hundred_million = 100_000_000.0;
    let inflow_component = (latest.main_net_inflow / hundred_million).clamp(-8.0, 12.0);
    let ratio_component = latest.main_net_inflow_ratio_pct.clamp(-10.0, 20.0) * 0.35;
    let price_component = latest.change_pct.clamp(-5.0, 12.0) * 0.5;
    inflow_component + ratio_component + price_component
}

pub fn billboard_source_score(items: &[BillboardEntry]) -> f64 {
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let net_component = latest
        .net_amount
        .map(|value| (value / 1_0000_0000.0).clamp(-6.0, 10.0))
        .unwrap_or(1.5);
    let turnover_component = latest
        .turnover_rate_pct
        .unwrap_or_default()
        .clamp(0.0, 30.0)
        * 0.15;
    let change_component = latest.change_rate_pct.clamp(-5.0, 12.0) * 0.4;
    net_component + turnover_component + change_component + 2.0
}
