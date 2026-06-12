use std::collections::HashSet;

use anyhow::{Context, bail};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::{
    CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
    StockSearchResult, f64_to_dec, opt_f64_to_dec,
    akshare_conv, wire::AkshareIndividualInfo,
};

impl MarketDataClient {
    /// Fallback search: try to look up a code directly via akshare a_share_search.
    pub(super) async fn search_eastmoney_direct_lookup(
        &self,
        code: &str,
    ) -> Option<StockSearchResult> {
        let trimmed = code.trim();
        if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        self.ak
            .a_share_search(trimmed, None, 1)
            .await
            .ok()
            .and_then(|mut items| items.drain(..).next())
    }

    pub(crate) async fn fetch_a_share_quote_from_tushare(
        &self,
        symbol: &str,
        ts_code: &str,
    ) -> anyhow::Result<QuoteSnapshot> {
        let today = chrono::Utc::now().format("%Y%m%d").to_string();
        let start_date = (chrono::Utc::now() - chrono::Duration::days(14))
            .format("%Y%m%d")
            .to_string();
        let rows = self
            .tushare_query(
                "daily",
                serde_json::json!({
                    "ts_code": ts_code,
                    "start_date": start_date,
                    "end_date": today
                }),
                "ts_code,trade_date,open,high,low,close,vol,amount",
            )
            .await?;
        let row = rows.first().context("tushare daily returned no rows")?;
        Ok(QuoteSnapshot {
            symbol: symbol.trim().to_uppercase(),
            date: row.string("trade_date")?,
            open: f64_to_dec(row.f64("open")?),
            high: f64_to_dec(row.f64("high")?),
            low: f64_to_dec(row.f64("low")?),
            close: f64_to_dec(row.f64("close")?),
            volume: (row.f64("vol")? * 100.0).round() as i64,
        })
    }

    pub(crate) async fn fetch_a_share_quote_from_eastmoney(
        &self,
        symbol: &str,
    ) -> anyhow::Result<QuoteSnapshot> {
        let q = self
            .ak
            .a_share_quote(symbol)
            .await
            .context("akshare a_share_quote failed")?;
        Ok(akshare_conv::quote_from_akshare(q))
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
            .map(akshare_conv::candle_from_akshare)
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
        let response = self
            .http
            .get("https://datacenter-web.eastmoney.com/api/data/v1/get")
            .query(&[
                ("reportName", "RPT_F10_FINANCE_MAINFINADATA"),
                ("columns", "ALL"),
                ("filter", &format!("(SECUCODE=\"{secucode}\")")),
                ("pageNumber", "1"),
                ("pageSize", "1"),
                ("sortTypes", "-1"),
                ("sortColumns", "REPORT_DATE"),
                ("source", "WEB"),
                ("client", "WEB"),
            ])
            .send()
            .await
            .context("failed to fetch Eastmoney main finance indicator")?
            .error_for_status()
            .context("eastmoney main finance indicator request failed")?;
        let payload: super::wire::EastmoneyDatacenterEnvelope<
            super::wire::EastmoneyMainFinanceIndicatorItem,
        > = response
            .json()
            .await
            .context("failed to decode eastmoney main finance indicator response")?;
        payload
            .result
            .and_then(|result| result.data)
            .and_then(|mut items| items.drain(..).next())
            .context("eastmoney main finance indicator returned no rows")
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
        Ok(akshare_conv::balance_sheet_to_wire(first))
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
        Ok(akshare_conv::cashflow_to_wire(first))
    }

    pub(super) async fn fetch_a_share_fundamentals(
        &self,
        symbol: &str,
        ts_code: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let basic_rows = self
            .tushare_query(
                "stock_basic",
                serde_json::json!({ "ts_code": ts_code, "list_status": "L" }),
                "ts_code,symbol,name,industry,list_date",
            )
            .await
            .unwrap_or_default();
        let basic = basic_rows.first();
        let mut search_items = self
            .ak.a_share_search(symbol.trim(), Some("A股"), 8)
            .await
            .unwrap_or_default();
        let search_match = search_items
            .drain(..)
            .find(|item| item.symbol == symbol.trim())
            .or(None);
        let info = self.fetch_a_share_individual_info(symbol).await.ok();
        let quote = self.fetch_a_share_quote_from_eastmoney(symbol).await.ok();
        let eastmoney_main = self.fetch_eastmoney_main_finance_indicator(ts_code).await.ok();
        let eastmoney_balance = self.fetch_eastmoney_balance_sheet(ts_code).await.ok();
        let eastmoney_cashflow = self.fetch_eastmoney_cashflow(ts_code).await.ok();

        let today = chrono::Utc::now().format("%Y%m%d").to_string();
        let recent_start = (chrono::Utc::now() - chrono::Duration::days(30))
            .format("%Y%m%d")
            .to_string();
        let daily_basic_rows = self
            .tushare_query(
                "daily_basic",
                serde_json::json!({
                    "ts_code": ts_code,
                    "start_date": recent_start,
                    "end_date": today
                }),
                "ts_code,trade_date,total_share,float_share,total_mv,circ_mv,pe,pb",
            )
            .await
            .unwrap_or_default();
        let daily_basic = daily_basic_rows.first();

        let income_rows = self
            .tushare_query(
                "income",
                serde_json::json!({ "ts_code": ts_code }),
                "ts_code,ann_date,end_date,total_revenue,revenue,n_income,n_income_attr_p",
            )
            .await
            .unwrap_or_default();
        let income = income_rows.first();

        let balance_rows = self
            .tushare_query(
                "balancesheet",
                serde_json::json!({ "ts_code": ts_code }),
                "ts_code,ann_date,end_date,total_assets,total_liab,total_hldr_eqy_exc_min_int,total_share,money_cap,lt_borr,st_borr",
            )
            .await
            .unwrap_or_default();
        let balance = balance_rows.first();

        let cashflow_rows = self
            .tushare_query(
                "cashflow",
                serde_json::json!({ "ts_code": ts_code }),
                "ts_code,ann_date,end_date,n_cashflow_act,free_cashflow",
            )
            .await
            .unwrap_or_default();
        let cashflow = cashflow_rows.first();

        let fina_indicator_rows = self
            .tushare_query(
                "fina_indicator",
                serde_json::json!({ "ts_code": ts_code }),
                "ts_code,ann_date,end_date,total_profit,op_of_gr,profit_dedt,ocfps,fcff",
            )
            .await
            .unwrap_or_default();
        let fina_indicator = fina_indicator_rows.first();

        let fiscal_year_end = Self::a_share_fiscal_year_end_candidate(
            income
                .and_then(|row| row.optional_string("end_date"))
                .or_else(|| balance.and_then(|row| row.optional_string("end_date")))
                .or_else(|| {
                    eastmoney_main.as_ref().and_then(|row| {
                        row.report_date
                            .clone()
                            .or_else(|| row.std_report_date.clone())
                    })
                }),
        );
        let provisional_shares_outstanding = daily_basic
            .and_then(|row| row.optional_f64("total_share"))
            .map(|value| (value * 10_000.0).round() as i64)
            .or_else(|| {
                balance
                    .and_then(|row| row.optional_f64("total_share"))
                    .map(|value| (value * 10_000.0).round() as i64)
            })
            .or_else(|| {
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.total_share)
                    .map(|value| value.round() as i64)
            })
            .or_else(|| info.as_ref().and_then(|value| value.total_share));
        let provisional_market_cap: Option<Decimal> = daily_basic
            .and_then(|row| row.optional_f64("total_mv"))
            .map(|v| f64_to_dec(v * 10_000.0))
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
                .or_else(|| basic.and_then(|row| row.optional_string("name")))
                .or_else(|| search_match.as_ref().map(|item| item.name.clone()))
                .unwrap_or_else(|| symbol.to_string()),
            cik: ts_code.to_string(),
            industry: info
                .as_ref()
                .and_then(|value| value.industry.clone())
                .or_else(|| basic.and_then(|row| row.optional_string("industry"))),
            currency: eastmoney_main
                .as_ref()
                .and_then(|item| item.currency.clone())
                .unwrap_or_else(|| "CNY".to_string()),
            fiscal_year_end,
            shares_outstanding,
            market_cap: provisional_market_cap,
            net_income_usd: opt_f64_to_dec(income.and_then(|row| {
                row.optional_f64("n_income_attr_p")
                    .or_else(|| row.optional_f64("n_income"))
            }).or_else(|| {
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.parent_net_profit.or(item.holder_profit))
            })),
            revenues_usd: opt_f64_to_dec(income.and_then(|row| {
                row.optional_f64("total_revenue")
                    .or_else(|| row.optional_f64("revenue"))
            }).or_else(|| {
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.total_operate_reve.or(item.operate_income))
            })),
            assets_usd: balance
                .and_then(|row| row.optional_f64("total_assets"))
                .map(f64_to_dec)
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.totalassets.or(item.total_assets))
                        .map(f64_to_dec)
                })
                .or(eastmoney_assets),
            liabilities_usd: balance
                .and_then(|row| row.optional_f64("total_liab"))
                .map(f64_to_dec)
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.totliab.or(item.total_liabilities))
                        .map(f64_to_dec)
                })
                .or(eastmoney_liabilities),
            stockholders_equity_usd: balance
                .and_then(|row| row.optional_f64("total_hldr_eqy_exc_min_int"))
                .map(f64_to_dec)
                .or_else(|| eastmoney_main.as_ref().and_then(|item| item.total_parent_equity).map(f64_to_dec))
                .or(eastmoney_equity),
            cash_and_equivalents_usd: opt_f64_to_dec(balance
                .and_then(|row| row.optional_f64("money_cap"))
                .or_else(|| eastmoney_balance.as_ref().and_then(|item| item.monetary_funds))
                .or_else(|| eastmoney_cashflow.as_ref().and_then(|item| item.end_cce))),
            gross_profit_usd: opt_f64_to_dec(eastmoney_main
                .as_ref()
                .and_then(|item| item.gross_profit.or(item.mlr))),
            operating_income_usd: opt_f64_to_dec(fina_indicator.and_then(|row| row.optional_f64("op_of_gr"))),
            operating_expenses_usd: None,
            operating_cash_flow_usd: opt_f64_to_dec(cashflow
                .and_then(|row| row.optional_f64("n_cashflow_act"))
                .or_else(|| {
                    eastmoney_main.as_ref().and_then(|item| {
                        item.netcash_operate
                            .or(item.mgjyxjje.map(|per_share| {
                                item.total_share
                                    .map(|shares| per_share * shares)
                                    .unwrap_or(per_share)
                            }))
                    })
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
            free_cash_flow_usd: opt_f64_to_dec(cashflow
                .and_then(|row| row.optional_f64("free_cashflow"))
                .or_else(|| fina_indicator.and_then(|row| row.optional_f64("fcff")))
                .or_else(|| {
                    eastmoney_main.as_ref().and_then(|item| match (
                        item.netcash_operate,
                        item.capital_expenditure.map(f64::abs),
                    ) {
                        (Some(ocf), Some(capex)) => Some(ocf - capex),
                        _ => None,
                    })
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
            long_term_debt_usd: opt_f64_to_dec(balance.and_then(|row| row.optional_f64("lt_borr"))),
            current_debt_usd: opt_f64_to_dec(balance
                .and_then(|row| row.optional_f64("st_borr"))
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.current_liab.or(item.current_liability))
                })
                .or_else(|| eastmoney_balance.as_ref().and_then(|item| item.current_liab))),
            total_debt_usd: opt_f64_to_dec(balance.and_then(|row| {
                let current = row.optional_f64("st_borr");
                let long_term = row.optional_f64("lt_borr");
                match (current, long_term) {
                    (Some(current), Some(long_term)) => Some(current + long_term),
                    (Some(current), None) => Some(current),
                    (None, Some(long_term)) => Some(long_term),
                    (None, None) => None,
                }
            }).or_else(|| {
                eastmoney_main.as_ref().and_then(|item| match (
                    item.current_liab.or(item.current_liability),
                    item.totalnoncliab.or(item.noncurrent_liab_1year),
                ) {
                    (Some(current), Some(noncurrent)) => Some(current + noncurrent),
                    (Some(current), None) => Some(current),
                    (None, Some(noncurrent)) => Some(noncurrent),
                    (None, None) => None,
                })
            }).or_else(|| {
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
    ) -> anyhow::Result<super::NewsFetchResult> {
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

        // Fetch eastmoney announcements first (fast, usually < 2s).
        // Then attempt web search with a bounded timeout so slow providers
        // don't block the entire pipeline.
        let eastmoney_result = self.fetch_a_share_eastmoney_news(ts_code, limit).await;
        let has_eastmoney = eastmoney_result.as_ref().is_ok_and(|items| !items.is_empty());

        let search_timeout_secs = if has_eastmoney { 6 } else { 12 };
        let (google_items, google_attempts) = match tokio::time::timeout(
            std::time::Duration::from_secs(search_timeout_secs),
            self.fetch_news_search_queries_with_attempts(
                &query_terms,
                "zh-CN",
                Some("month"),
                None,
                None,
                super::GeneralSearchIntent::CompanyEvidence,
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                tracing::info!(
                    symbol = %ts_code,
                    timeout_secs = search_timeout_secs,
                    "A-share web search timed out, using eastmoney results only"
                );
                (Vec::new(), Vec::new())
            }
        };
        let mut result = Self::merge_a_share_news(
            ts_code,
            limit,
            eastmoney_result,
            google_items,
            google_attempts,
            query_terms,
        )?;

        // If per-symbol news is sparse, fetch macro/industry context
        if result.items.len() < 10 {
            let company = &search_match;
            let macro_queries = vec![
                format!("{} 行业 政策", company),
                "中国 宏观经济 货币政策".to_string(),
                "A股 市场 资金面".to_string(),
            ];
            let existing_titles: std::collections::HashSet<String> =
                result.items.iter().map(|i| i.title.to_lowercase()).collect();
            if let Ok((macro_items, macro_attempts)) = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                self.fetch_news_search_queries_with_attempts(
                    &macro_queries,
                    "zh-CN",
                    Some("month"),
                    None,
                    None,
                    super::GeneralSearchIntent::MacroEvidence,
                ),
            )
            .await
            {
                for item in macro_items {
                    if !existing_titles.contains(&item.title.to_lowercase()) {
                        result.items.push(item);
                    }
                }
                result.attempts.extend(macro_attempts);
            }
        }

        Ok(result)
    }

    pub(super) async fn fetch_a_share_global_news_diagnostics(
        &self,
        curr_date: &str,
        _look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<super::NewsFetchResult> {
        // Fetch from akshare Chinese financial news sources in parallel.
        let (cls_res, ths_res, sina_res, futu_res) = tokio::join!(
            super::akshare_rust::a_share::fetch_global_news_cls(self),
            super::akshare_rust::a_share::fetch_global_news_ths(self),
            super::akshare_rust::a_share::fetch_global_news_sina(self),
            super::akshare_rust::a_share::fetch_global_news_futu(self),
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
                    akshare_items.extend(items);
                    attempts.push(super::NewsFetchAttempt {
                        source: name.to_string(),
                        query: None,
                        success: true,
                        item_count: count,
                        error: None,
                    });
                }
                Err(e) => {
                    attempts.push(super::NewsFetchAttempt {
                        source: name.to_string(),
                        query: None,
                        success: false,
                        item_count: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let queries = vec![
            "A股 市场 宏观".to_string(),
            "中国 经济 政策".to_string(),
            "A股 资金面".to_string(),
            "A股 银行 券商 基金".to_string(),
            "China stock market economy policy".to_string(),
            "A股 央行 降息 利率".to_string(),
            "中国 GDP PMI 经济数据".to_string(),
            "沪深 北向资金 外资".to_string(),
        ];
        let (web_merged, web_attempts) = self
            .fetch_news_search_queries_with_attempts(
                &queries,
                "zh-CN",
                Some("month"),
                None,
                None,
                super::GeneralSearchIntent::MacroEvidence,
            )
            .await;
        attempts.extend(web_attempts);
        let mut merged = akshare_items;
        merged.extend(web_merged);
        let original_merged = merged.clone();
        let year = curr_date.get(0..4).unwrap_or_default();
        let filtered_by_year = merged
            .into_iter()
            .filter(|item| {
            super::normalized_news_date(&item.published_at)
                .is_some_and(|date| date.starts_with(year))
            })
            .collect::<Vec<_>>();
        let selected_items = if filtered_by_year.is_empty() {
            tracing::info!(
                curr_date = %curr_date,
                "a-share global news year filter removed all searxng results; keeping unfiltered recent items"
            );
            original_merged
        } else {
            filtered_by_year
        };
        let merged = super::merge_ranked_news(
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
            attempts.push(super::NewsFetchAttempt {
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
        Ok(super::NewsFetchResult {
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
        mut google_attempts: Vec<super::NewsFetchAttempt>,
        keywords: Vec<String>,
    ) -> anyhow::Result<super::NewsFetchResult> {
        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        let mut errors = Vec::new();
        let mut attempts = Vec::new();

        match eastmoney_result {
            Ok(items) => {
                attempts.push(super::NewsFetchAttempt {
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
                attempts.push(super::NewsFetchAttempt {
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
        let ranked = super::merge_ranked_news(merged, target_limit, None, None, &keywords);
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
        Ok(super::NewsFetchResult {
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
        let candles: Vec<CandlePoint> = ak_candles.into_iter().map(super::akshare_conv::candle_from_akshare).collect();
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
            .map(akshare_conv::news_item_from_announcement)
            .collect();
        if items.is_empty() {
            bail!("eastmoney returned no announcement items");
        }
        Ok(items)
    }

}
