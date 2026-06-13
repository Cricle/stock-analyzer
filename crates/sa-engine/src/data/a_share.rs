use std::collections::HashSet;

use anyhow::{Context, bail};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::{
    CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
    f64_to_dec, opt_f64_to_dec,
    wire::AkshareIndividualInfo,
};
use crate::types::{NewsFetchAttempt, NewsFetchResult};

impl MarketDataClient {
    pub(crate) async fn fetch_a_share_quote_from_eastmoney(
        &self,
        symbol: &str,
    ) -> anyhow::Result<QuoteSnapshot> {
        let q = self
            .ak
            .a_share_quote(symbol)
            .await
            .context("akshare a_share_quote failed")?;
        Ok(super::quote_from_akshare(q))
    }

    pub(crate) async fn fetch_a_share_tencent_candles(
        &self,
        symbol: &str,
        adjust: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CandlePoint>> {
        let items = self
            .ak
            .a_share_candles(symbol, adjust, limit)
            .await
            .context("akshare a_share_candles failed")?;
        Ok(items
            .into_iter()
            .map(super::candle_from_akshare)
            .collect())
    }

    pub(crate) async fn fetch_a_share_eastmoney_candles(
        &self,
        symbol: &str,
        adjust: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CandlePoint>> {
        // Delegate to akshare which has Tencent -> Eastmoney -> Tushare fallback
        self.fetch_a_share_tencent_candles(symbol, adjust, limit).await
    }
}
impl MarketDataClient {
    pub(crate) async fn fetch_a_share_insider_transactions(&self, symbol: &str) -> anyhow::Result<Vec<NewsItem>> {
        let items = self.ak.stock_ggcg_em(symbol).await?;
        Ok(items
            .into_iter()
            .map(|item| {
                let direction = if item.direction.contains("增") || item.direction.to_uppercase() == "IN" {
                    "增持"
                } else {
                    "减持"
                };
                NewsItem {
                    published_at: item.notice_date,
                    title: format!("{} {} {}", item.holder_name, direction, item.name),
                    summary: format!(
                        "变动{}股, 占总股本{:.4}%, 占流通股{:.4}%, 持股{}股",
                        item.change_amount, item.change_total_ratio, item.change_circulating_ratio, item.holding_count
                    ),
                    source: "Eastmoney 高管持股".to_string(),
                    url: None,
                }
            })
            .collect())
    }
}
impl MarketDataClient {
    pub(crate) fn a_share_fiscal_year_end_candidate(value: Option<String>) -> Option<String> {
        let raw = value?;
        let digits = raw
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.len() < 8 || &digits[4..8] != "1231" {
            return None;
        }
        Some(format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]))
    }

    fn a_share_macro_reference_pages(curr_date: &str) -> Vec<NewsItem> {
        vec![
            NewsItem {
                published_at: curr_date.to_string(),
                title: "中国宏观与政策跟踪 - 国家统计局".to_string(),
                summary: "A股场景宏观参考页，覆盖经济数据、公报与统计发布。".to_string(),
                source: "stats.gov.cn".to_string(),
                url: Some("https://www.stats.gov.cn/".to_string()),
            },
            NewsItem {
                published_at: curr_date.to_string(),
                title: "中国货币政策与金融数据 - 中国人民银行".to_string(),
                summary: "A股场景宏观参考页，覆盖利率、流动性与金融统计。".to_string(),
                source: "pbc.gov.cn".to_string(),
                url: Some("http://www.pbc.gov.cn/".to_string()),
            },
            NewsItem {
                published_at: curr_date.to_string(),
                title: "A股市场总览 - 东方财富".to_string(),
                summary: "A股市场场景参考页，覆盖指数、板块、资金面与市场新闻入口。".to_string(),
                source: "eastmoney.com".to_string(),
                url: Some("https://www.eastmoney.com/".to_string()),
            },
        ]
    }

    async fn fetch_eastmoney_main_finance_indicator(
        &self,
        secucode: &str,
    ) -> anyhow::Result<super::wire::EastmoneyMainFinanceIndicatorItem> {
        let items = self
            .ak
            .stock_financial_analysis_indicator_em(secucode, "按报告期")
            .await
            .context("akshare stock_financial_analysis_indicator_em failed")?;
        let first = items
            .into_iter()
            .next()
            .context("eastmoney main finance indicator returned no rows")?;
        let value = serde_json::Value::Object(first.into_iter().collect());
        serde_json::from_value(value)
            .context("failed to deserialize eastmoney main finance indicator")
    }

    async fn fetch_eastmoney_balance_sheet(
        &self,
        secucode: &str,
    ) -> anyhow::Result<super::wire::EastmoneyBalanceSheetItem> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_balance_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare balance sheet failed")?;
        let first = sheets.first().context("akshare balance sheet returned no rows")?;
        Ok(super::balance_sheet_to_wire(first))
    }

    async fn fetch_eastmoney_cashflow(
        &self,
        secucode: &str,
    ) -> anyhow::Result<super::wire::EastmoneyCashflowItem> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_cash_flow_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare cashflow failed")?;
        let first = sheets.first().context("akshare cashflow returned no rows")?;
        Ok(super::cashflow_to_wire(first))
    }

    async fn fetch_a_share_spot_quote(
        &self,
        symbol: &str,
    ) -> anyhow::Result<super::wire::AkshareIndividualInfo> {
        let spot_items = self
            .ak
            .stock_zh_a_spot_em()
            .await
            .context("akshare stock_zh_a_spot_em failed")?;
        let code = symbol.split_once('.').map(|(c, _)| c).unwrap_or(symbol);
        let spot = spot_items
            .iter()
            .find(|item| item.code == code)
            .context("symbol not found in spot quotes")?;
        Ok(super::wire::AkshareIndividualInfo {
            stock_name: Some(spot.name.clone()),
            total_share: None,
            market_cap: Some(spot.total_market_cap),
            industry: None,
        })
    }

    async fn fetch_a_share_profit_sheet(
        &self,
        secucode: &str,
    ) -> anyhow::Result<super::wire::ProfitSheetWire> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_profit_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare profit sheet failed")?;
        let first = sheets.first().context("akshare profit sheet returned no rows")?;
        Ok(super::profit_sheet_to_wire(first))
    }

    pub(super) async fn fetch_a_share_fundamentals(
        &self,
        symbol: &str,
        ts_code: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let mut search_items = self
            .ak.a_share_search(symbol.trim(), Some("A股"), 8)
            .await
            .unwrap_or_default();
        let search_match = search_items
            .drain(..)
            .find(|item| item.symbol == symbol.trim())
            .or(None);
        let info = self.fetch_a_share_individual_info(symbol).await.ok();
        let spot_quote = self.fetch_a_share_spot_quote(symbol).await.ok();
        let quote = self.fetch_a_share_quote_from_eastmoney(symbol).await.ok();
        let eastmoney_main = self.fetch_eastmoney_main_finance_indicator(ts_code).await.ok();
        let eastmoney_balance = self.fetch_eastmoney_balance_sheet(ts_code).await.ok();
        let eastmoney_cashflow = self.fetch_eastmoney_cashflow(ts_code).await.ok();
        let profit_sheet = self.fetch_a_share_profit_sheet(ts_code).await.ok();

        let fiscal_year_end = Self::a_share_fiscal_year_end_candidate(
            profit_sheet
                .as_ref()
                .and_then(|p| p.notice_date.clone())
                .or_else(|| {
                    eastmoney_main.as_ref().and_then(|row| {
                        row.report_date
                            .clone()
                            .or_else(|| row.std_report_date.clone())
                    })
                }),
        );
        let provisional_shares_outstanding = info
            .as_ref()
            .and_then(|value| value.total_share)
            .or_else(|| {
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.total_share)
                    .map(|value| value.round() as i64)
            });
        let provisional_market_cap: Option<Decimal> = spot_quote
            .as_ref()
            .and_then(|q| q.market_cap)
            .map(f64_to_dec)
            .or_else(|| opt_f64_to_dec(info.as_ref().and_then(|value| value.market_cap)))
            .or_else(|| {
                let shares = provisional_shares_outstanding?;
                let quote = quote.as_ref()?;
                Some(quote.close * Decimal::from(shares))
            });
        let shares_outstanding = provisional_shares_outstanding.or_else(|| {
            let market_cap = provisional_market_cap?;
            let price = quote.as_ref()?.close;
            if price > Decimal::ZERO {
                (market_cap / price).round().to_i64()
            } else {
                None
            }
        });
        let eastmoney_equity: Option<Decimal> = eastmoney_balance.as_ref().and_then(|item| item.total_equity)
            .map(f64_to_dec)
            .or_else(|| eastmoney_main.as_ref().and_then(|item| {
                let shares = item.total_share?;
                let bps = item.bps?;
                Some(f64_to_dec(shares * bps))
            }));
        let eastmoney_assets: Option<Decimal> = eastmoney_balance.as_ref().and_then(|item| item.total_assets)
            .map(f64_to_dec)
            .or_else(|| eastmoney_main.as_ref().and_then(|item| {
                let equity = eastmoney_equity?;
                let debt_ratio_pct = item.zcfzl?;
                let equity_ratio = f64_to_dec(1.0 - (debt_ratio_pct / 100.0));
                (equity_ratio > Decimal::ZERO).then_some(equity / equity_ratio)
            }));
        let eastmoney_liabilities: Option<Decimal> = eastmoney_balance
            .as_ref()
            .and_then(|item| item.total_liabilities)
            .map(f64_to_dec)
            .or_else(|| eastmoney_assets.zip(eastmoney_equity).map(|(assets, equity)| assets - equity));

        Ok(FundamentalsSnapshot {
            symbol: symbol.trim().to_uppercase(),
            company_name: info
                .as_ref()
                .and_then(|value| value.stock_name.clone())
                .or_else(|| spot_quote.as_ref().and_then(|q| q.stock_name.clone()))
                .or_else(|| search_match.as_ref().map(|item| item.name.clone()))
                .unwrap_or_else(|| symbol.to_string()),
            cik: ts_code.to_string(),
            industry: info
                .as_ref()
                .and_then(|value| value.industry.clone()),
            currency: eastmoney_main
                .as_ref()
                .and_then(|item| item.currency.clone())
                .unwrap_or_else(|| "CNY".to_string()),
            fiscal_year_end,
            shares_outstanding,
            market_cap: provisional_market_cap,
            net_income_usd: opt_f64_to_dec(profit_sheet
                .as_ref()
                .and_then(|p| p.net_profit_deducted.or(p.net_profit))
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.parent_net_profit.or(item.holder_profit))
                })),
            revenues_usd: opt_f64_to_dec(profit_sheet
                .as_ref()
                .and_then(|p| p.total_revenue)
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.total_operate_reve.or(item.operate_income))
                })),
            assets_usd: eastmoney_assets,
            liabilities_usd: eastmoney_liabilities,
            stockholders_equity_usd: eastmoney_equity,
            cash_and_equivalents_usd: opt_f64_to_dec(
                eastmoney_balance.as_ref().and_then(|item| item.monetary_funds)
                    .or_else(|| eastmoney_cashflow.as_ref().and_then(|item| item.end_cce))),
            gross_profit_usd: opt_f64_to_dec(eastmoney_main
                .as_ref()
                .and_then(|item| item.gross_profit.or(item.mlr))),
            operating_income_usd: opt_f64_to_dec(eastmoney_main
                .as_ref()
                .and_then(|item| item.operate_income)),
            operating_expenses_usd: None,
            operating_cash_flow_usd: opt_f64_to_dec(
                eastmoney_main.as_ref().and_then(|item| {
                    item.netcash_operate
                        .or(item.mgjyxjje.map(|per_share| {
                            item.total_share
                                .map(|shares| per_share * shares)
                                .unwrap_or(per_share)
                        }))
                })
                .or_else(|| eastmoney_cashflow.as_ref().and_then(|item| item.netcash_operate))),
            capital_expenditure_usd: opt_f64_to_dec(eastmoney_main
                .as_ref()
                .and_then(|item| item.capital_expenditure)
                .map(f64::abs)
                .or_else(|| {
                    eastmoney_cashflow
                        .as_ref()
                        .and_then(|item| item.construct_long_asset)
                        .map(f64::abs)
                })),
            free_cash_flow_usd: opt_f64_to_dec(
                eastmoney_main.as_ref().and_then(|item| match (
                    item.netcash_operate,
                    item.capital_expenditure.map(f64::abs),
                ) {
                    (Some(ocf), Some(capex)) => Some(ocf - capex),
                    _ => None,
                })
                .or_else(|| {
                    eastmoney_cashflow.as_ref().and_then(|item| match (
                        item.netcash_operate,
                        item.construct_long_asset.map(f64::abs),
                    ) {
                        (Some(ocf), Some(capex)) => Some(ocf - capex),
                        _ => None,
                    })
                })),
            long_term_debt_usd: None,
            current_debt_usd: opt_f64_to_dec(
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.current_liab.or(item.current_liability))
                    .or_else(|| eastmoney_balance.as_ref().and_then(|item| item.current_liab))),
            total_debt_usd: opt_f64_to_dec(
                eastmoney_main.as_ref().and_then(|item| match (
                    item.current_liab.or(item.current_liability),
                    item.totalnoncliab.or(item.noncurrent_liab_1year),
                ) {
                    (Some(current), Some(noncurrent)) => Some(current + noncurrent),
                    (Some(current), None) => Some(current),
                    (None, Some(noncurrent)) => Some(noncurrent),
                    (None, None) => None,
                })
                .or_else(|| {
                    eastmoney_balance.as_ref().and_then(|item| match (item.current_liab, item.totalnoncliab) {
                        (Some(current), Some(noncurrent)) => Some(current + noncurrent),
                        (Some(current), None) => Some(current),
                        (None, Some(noncurrent)) => Some(noncurrent),
                        (None, None) => None,
                    })
                })),
            diluted_shares_outstanding: None,
        })
    }
}
impl MarketDataClient {

    pub(super) async fn fetch_a_share_news_diagnostics(
        &self,
        ts_code: &str,
        limit: usize,
    ) -> anyhow::Result<NewsFetchResult> {
        let symbol = ts_code.split('.').next().unwrap_or(ts_code);
        let search_match = self
            .ak.a_share_search(symbol, Some("A股"), 8)
            .await
            .ok()
            .and_then(|mut items| {
                items
                    .drain(..)
                    .find(|item| item.symbol == symbol)
                    .map(|item| item.name)
            })
            .unwrap_or_else(|| symbol.to_string());
        let query_terms = vec![
            symbol.to_string(),
            search_match.clone(),
            format!("{search_match} 公告"),
            format!("{search_match} 业绩"),
            format!("{search_match} 回购 分红"),
            format!("{search_match} 调研"),
        ];

        // Fetch eastmoney announcements (web search removed).
        let eastmoney_result = self.fetch_a_share_eastmoney_news(ts_code, limit).await;
        let result = Self::merge_a_share_news(
            ts_code,
            limit,
            eastmoney_result,
            Vec::new(),
            Vec::new(),
            query_terms,
        )?;

        Ok(result)
    }

    pub(super) async fn fetch_a_share_global_news_diagnostics(
        &self,
        curr_date: &str,
        _look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<NewsFetchResult> {
        // Fetch from akshare Chinese financial news sources in parallel.
        let (cls_res, ths_res, sina_res, futu_res) = tokio::join!(
            self.ak.stock_info_global_cls(),
            self.ak.stock_info_global_ths(),
            self.ak.stock_info_global_sina(),
            self.ak.stock_info_global_futu(),
        );
        let mut akshare_items = Vec::new();
        let mut attempts = Vec::new();
        for (name, result) in [
            ("CLS 财联社", cls_res),
            ("THS 同花顺", ths_res),
            ("Sina 新浪", sina_res),
            ("Futu 富途", futu_res),
        ] {
            match result {
                Ok(items) => {
                    let count = items.len();
                    akshare_items.extend(items.into_iter().map(super::news_item_from_news_entry));
                    attempts.push(NewsFetchAttempt {
                        source: name.to_string(),
                        query: None,
                        success: true,
                        item_count: count,
                        error: None,
                    });
                }
                Err(e) => {
                    attempts.push(NewsFetchAttempt {
                        source: name.to_string(),
                        query: None,
                        success: false,
                        item_count: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let merged = akshare_items;
        let year = curr_date.get(0..4).unwrap_or_default();
        let filtered_by_year: Vec<_> = merged
            .iter()
            .filter(|item| {
                super::news_filter::normalized_news_date(&item.published_at)
                    .is_some_and(|date| date.starts_with(year))
            })
            .cloned()
            .collect();
        let selected_items = if filtered_by_year.is_empty() {
            merged
        } else {
            filtered_by_year
        };
        let merged = super::news_filter::merge_ranked_news(
            selected_items,
            limit.max(8),
            None,
            None,
            &[
                "A股".to_string(),
                "中国经济".to_string(),
                "政策".to_string(),
                "资金面".to_string(),
            ],
        );
        let (merged, attempts, cacheable) = if merged.is_empty() {
            let fallback_items = Self::a_share_macro_reference_pages(curr_date);
            let mut attempts = attempts;
            attempts.push(NewsFetchAttempt {
                source: "A-share Macro Reference".to_string(),
                query: Some(curr_date.to_string()),
                success: true,
                item_count: fallback_items.len(),
                error: None,
            });
            (fallback_items, attempts, false)
        } else {
            let cacheable = super::news_result_cacheable(&merged, &attempts);
            (merged, attempts, cacheable)
        };
        Ok(NewsFetchResult {
            items: merged,
            attempts,
            cacheable,
        })
    }

    fn merge_a_share_news(
        ts_code: &str,
        limit: usize,
        eastmoney_result: anyhow::Result<Vec<NewsItem>>,
        google_items: Vec<NewsItem>,
        mut google_attempts: Vec<NewsFetchAttempt>,
        keywords: Vec<String>,
    ) -> anyhow::Result<NewsFetchResult> {
        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        let mut errors = Vec::new();
        let mut attempts = Vec::new();

        match eastmoney_result {
            Ok(items) => {
                attempts.push(NewsFetchAttempt {
                    source: "Eastmoney 公告".to_string(),
                    query: Some(ts_code.to_string()),
                    success: true,
                    item_count: items.len(),
                    error: None,
                });
                Self::push_unique_news(&mut merged, &mut seen, items)
            }
            Err(error) => {
                errors.push(format!("eastmoney: {error:#}"));
                attempts.push(NewsFetchAttempt {
                    source: "Eastmoney 公告".to_string(),
                    query: Some(ts_code.to_string()),
                    success: false,
                    item_count: 0,
                    error: Some(error.to_string()),
                });
            }
        }
        Self::push_unique_news(&mut merged, &mut seen, google_items);
        attempts.append(&mut google_attempts);

        if merged.is_empty() {
            bail!(
                "no A-share news available for {} from merged upstreams: {}",
                ts_code,
                errors.join(" | ")
            );
        }

        let target_limit = limit.max(8);
        let ranked = super::news_filter::merge_ranked_news(merged, target_limit, None, None, &keywords);
        let has_official = ranked.iter().any(|item| item.source.contains("Eastmoney"));
        let web_quota = if has_official {
            ranked
                .iter()
                .filter(|item| !item.source.contains("Eastmoney"))
                .count()
                .min(if target_limit >= 6 { 2 } else { 1 })
        } else {
            0
        };
        let mut items = Vec::with_capacity(target_limit);
        let mut web_taken = 0usize;
        let mut seen = HashSet::new();
        for item in ranked
            .iter()
            .filter(|item| !item.source.contains("Eastmoney"))
            .take(web_quota)
        {
            let key = format!(
                "{}|{}|{}",
                item.title.trim(),
                item.published_at.trim(),
                item.url.as_deref().unwrap_or_default().trim()
            );
            if seen.insert(key) {
                items.push(item.clone());
                web_taken += 1;
            }
        }
        for item in ranked {
            if items.len() >= target_limit {
                break;
            }
            let key = format!(
                "{}|{}|{}",
                item.title.trim(),
                item.published_at.trim(),
                item.url.as_deref().unwrap_or_default().trim()
            );
            if seen.insert(key) {
                items.push(item);
            }
        }
        if web_taken == 0 && has_official {
            tracing::info!(symbol = %ts_code, "a-share news merge produced only official announcement items");
        }
        let cacheable = super::news_result_cacheable(&items, &attempts);
        Ok(NewsFetchResult {
            items,
            attempts,
            cacheable,
        })
    }

    fn push_unique_news(
        merged: &mut Vec<NewsItem>,
        seen: &mut HashSet<String>,
        items: Vec<NewsItem>,
    ) {
        for item in items {
            let dedupe_key = format!(
                "{}|{}|{}",
                item.title.trim(),
                item.published_at.trim(),
                item.url.as_deref().unwrap_or_default().trim()
            );
            if seen.insert(dedupe_key) {
                merged.push(item);
            }
        }
    }

    async fn fetch_a_share_individual_info(
        &self,
        symbol: &str,
    ) -> anyhow::Result<AkshareIndividualInfo> {
        let items = self
            .ak
            .stock_individual_info_em(symbol)
            .await
            .context("akshare stock_individual_info_em failed")?;
        let find_value = |label: &str| -> Option<&serde_json::Value> {
            items.iter().find(|item| item.item == label).map(|item| &item.value)
        };
        Ok(AkshareIndividualInfo {
            stock_name: find_value("股票简称").and_then(|v| v.as_str()).map(String::from),
            total_share: find_value("总股本").and_then(|v| v.as_f64()).map(|v| v as i64),
            market_cap: find_value("总市值").and_then(|v| v.as_f64()),
            industry: find_value("行业").and_then(|v| v.as_str()).map(String::from),
        })
    }
}
impl MarketDataClient {
    pub(super) async fn fetch_a_share_return_since(
        &self,
        symbol: &str,
        start_date: &str,
        holding_days: usize,
    ) -> anyhow::Result<Option<f64>> {
        let ak_candles = self.ak.a_share_candles(symbol, "qfq", 120).await?;
        let candles: Vec<CandlePoint> = ak_candles.into_iter().map(super::candle_from_akshare).collect();
        let mut items = candles
            .into_iter()
            .map(|item| (item.trade_date, item.close))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        let Some(start_index) = items.iter().position(|(date, _)| date == start_date) else {
            return Ok(None);
        };
        let end_index = (start_index + holding_days).min(items.len().saturating_sub(1));
        if end_index <= start_index {
            return Ok(None);
        }
        let start_price = items[start_index].1;
        let end_price = items[end_index].1;
        if start_price <= Decimal::ZERO {
            return Ok(None);
        }
        Ok(Some(((end_price - start_price) / start_price).to_f64().unwrap_or_default()))
    }

    async fn fetch_a_share_eastmoney_news(
        &self,
        ts_code: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let symbol = ts_code
            .split_once('.')
            .map(|(code, _)| code)
            .unwrap_or(ts_code);
        let announcements = self
            .ak
            .a_share_announcements(symbol, limit)
            .await
            .context("akshare a_share_announcements failed")?;
        let items: Vec<NewsItem> = announcements
            .into_iter()
            .map(super::news_item_from_announcement)
            .collect();
        if items.is_empty() {
            bail!("eastmoney returned no announcement items");
        }
        Ok(items)
    }

}
