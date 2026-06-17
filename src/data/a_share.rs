use std::collections::HashSet;

use anyhow::{Context, bail};
use serde::Deserialize;

use super::{
    CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
};
use crate::types::{NewsFetchAttempt, NewsFetchResult};

#[derive(Debug, Clone)]
struct AkshareIndividualInfo {
    stock_name: Option<String>,
    total_share: Option<i64>,
    market_cap: Option<f64>,
    industry: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EastmoneyMainFinanceIndicatorItem {
    #[serde(rename = "REPORT_DATE")]
    report_date: Option<String>,
    #[serde(rename = "STD_REPORT_DATE")]
    std_report_date: Option<String>,
    #[serde(rename = "CURRENCY")]
    currency: Option<String>,
    #[serde(rename = "OPERATE_INCOME")]
    operate_income: Option<f64>,
    #[serde(rename = "TOTALOPERATEREVE")]
    total_operate_reve: Option<f64>,
    #[serde(rename = "GROSS_PROFIT")]
    gross_profit: Option<f64>,
    #[serde(rename = "MLR")]
    mlr: Option<f64>,
    #[serde(rename = "HOLDER_PROFIT")]
    holder_profit: Option<f64>,
    #[serde(rename = "PARENTNETPROFIT")]
    parent_net_profit: Option<f64>,
    #[serde(rename = "NETCASH_OPERATE")]
    netcash_operate: Option<f64>,
    #[serde(rename = "MGJYXJJE")]
    mgjyxjje: Option<f64>,
    #[serde(rename = "BPS")]
    bps: Option<f64>,
    #[serde(rename = "ZCFZL")]
    zcfzl: Option<f64>,
    #[serde(rename = "CURRENT_LIABILITY")]
    current_liability: Option<f64>,
    #[serde(rename = "CURRENT_LIAB")]
    current_liab: Option<f64>,
    #[serde(rename = "NONCURRENT_LIAB_1YEAR")]
    noncurrent_liab_1year: Option<f64>,
    #[serde(rename = "TOTALNONCLIAB")]
    totalnoncliab: Option<f64>,
    #[serde(rename = "CAPITAL_EXPENDITURE")]
    capital_expenditure: Option<f64>,
    #[serde(rename = "TOTAL_SHARE")]
    total_share: Option<f64>,
}

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
        Ok(q)
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
        Ok(items)
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
                let direction_label = if item.direction.to_uppercase().contains("IN") || item.direction.contains("增") {
                    "Insider Buy"
                } else {
                    "Insider Sell"
                };
                NewsItem {
                    published_at: item.notice_date,
                    title: format!("{} {} {}", item.holder_name, direction_label, item.name),
                    summary: format!(
                        "Changed {} shares, {:.4}% of total, {:.4}% of float, holding {}",
                        item.change_amount, item.change_total_ratio, item.change_circulating_ratio, item.holding_count
                    ),
                    source: "Eastmoney Insider".to_string(),
                    url: None,
                }
            })
            .collect())
    }
}

/// Enrichment data fetched from akshare-rs for scoring.
#[derive(Debug, Clone, Default)]
pub(crate) struct AShareEnrichmentData {
    pub pe_ttm: Option<f64>,
    pub pb: Option<f64>,
    pub peg: Option<f64>,
    pub ps: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub net_profit_yoy: Option<f64>,
    pub gross_margin: Option<f64>,
    pub fund_flow_net_ratio: Option<f64>,
    // Chip distribution
    pub chip_benefit_ratio: Option<f64>,
    pub chip_avg_cost: Option<f64>,
    pub chip_concentration_90: Option<f64>,
    // Dividend
    pub dividend_yield: Option<f64>,
    // Analyst coverage
    pub analyst_report_count: Option<i64>,
    pub analyst_buy_ratio: Option<f64>,
    // Industry from earnings report (fallback)
    pub industry: Option<String>,
}

impl MarketDataClient {
    /// Fetch enrichment data (valuation, earnings, fund flow, chip, dividend, analyst) for A-share scoring.
    pub(crate) async fn fetch_a_share_enrichment(
        &self,
        symbol: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        let code = symbol.split_once('.').map(|(c, _)| c).unwrap_or(symbol);
        // Fetch all enrichment data in parallel
        let (valuation, earnings, fund_flow, chip, dividend, analyst) = tokio::join!(
            self.fetch_a_share_valuation(code),
            self.fetch_a_share_latest_earnings(code),
            self.fetch_a_share_fund_flow(code),
            self.fetch_a_share_chip(code),
            self.fetch_a_share_dividend(code),
            self.fetch_a_share_analyst(code),
        );
        let val = valuation.unwrap_or_default();
        let earn = earnings.unwrap_or_default();
        let flow = fund_flow.unwrap_or_default();
        let chip_data = chip.unwrap_or_default();
        let div_data = dividend.unwrap_or_default();
        let analyst_data = analyst.unwrap_or_default();
        Ok(AShareEnrichmentData {
            pe_ttm: val.pe_ttm,
            pb: val.pb,
            peg: val.peg,
            ps: val.ps,
            revenue_yoy: earn.revenue_yoy,
            net_profit_yoy: earn.net_profit_yoy,
            gross_margin: earn.gross_margin,
            fund_flow_net_ratio: flow,
            chip_benefit_ratio: chip_data.chip_benefit_ratio,
            chip_avg_cost: chip_data.chip_avg_cost,
            chip_concentration_90: chip_data.chip_concentration_90,
            dividend_yield: div_data.dividend_yield,
            analyst_report_count: analyst_data.analyst_report_count,
            analyst_buy_ratio: analyst_data.analyst_buy_ratio,
            industry: earn.industry,
        })
    }

    async fn fetch_a_share_valuation(
        &self,
        code: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        let items = self.ak.stock_value_em(code).await?;
        let first = items.first().context("stock_value_em returned no rows")?;
        Ok(AShareEnrichmentData {
            pe_ttm: if first.pe_ttm != 0.0 { Some(first.pe_ttm) } else { None },
            pb: if first.pb != 0.0 { Some(first.pb) } else { None },
            peg: if first.peg != 0.0 { Some(first.peg) } else { None },
            ps: if first.ps != 0.0 { Some(first.ps) } else { None },
            ..AShareEnrichmentData::default()
        })
    }

    async fn fetch_a_share_latest_earnings(
        &self,
        code: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        // Try multiple report dates: recent quarters + annual reports
        // Dates must be in YYYYMMDD format for akshare-rs fmt_date
        let now = chrono::Utc::now();
        let year: i32 = now.format("%Y").to_string().parse().unwrap_or(2026);
        let dates = [
            format!("{year}0331"),
            format!("{year}0630"),
            format!("{year}0930"),
            format!("{year}1231"),
            format!("{}1231", year - 1),
            format!("{}0930", year - 1),
        ];
        for date in &dates {
            match self.ak.stock_yjbb_em(date).await {
                Ok(items) => {
                    if let Some(item) = items.iter().find(|i| i.code == code) {
                        // Eastmoney returns YoY as percentages (15.0 = 15%), convert to decimal
                        let revenue_yoy = if item.total_revenue_yoy != 0.0 { Some(item.total_revenue_yoy / 100.0) } else { None };
                        let net_profit_yoy = if item.net_profit_yoy != 0.0 { Some(item.net_profit_yoy / 100.0) } else { None };
                        return Ok(AShareEnrichmentData {
                            revenue_yoy,
                            net_profit_yoy,
                            gross_margin: if item.gross_margin != 0.0 { Some(item.gross_margin / 100.0) } else { None },
                            industry: item.industry.clone().filter(|s| !s.is_empty()),
                            ..AShareEnrichmentData::default()
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(code, date, error = %e, "stock_yjbb_em failed");
                }
            }
        }
        tracing::warn!(code, "no earnings data found for any date");
        Ok(AShareEnrichmentData::default())
    }

    async fn fetch_a_share_fund_flow(
        &self,
        code: &str,
    ) -> anyhow::Result<Option<f64>> {
        match self.ak.stock_main_fund_flow(code).await {
            Ok(items) => {
                let Some(last) = items.last() else {
                    return Ok(None);
                };
                // net_ratio_pct is percentage (e.g. 1.07 = 1.07%), convert to ratio
                let result = last.net_ratio_pct / 100.0;
                tracing::debug!(code, ratio = result, date = %last.date, "fund_flow parsed");
                Ok(Some(result))
            }
            Err(e) => {
                tracing::warn!(code, error = %e, "main_fund_flow failed");
                Ok(None)
            }
        }
    }

    async fn fetch_a_share_chip(
        &self,
        code: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        let items = self.ak.stock_cyq_em(code, "qfq").await?;
        let Some(latest) = items.last() else {
            return Ok(AShareEnrichmentData::default());
        };
        Ok(AShareEnrichmentData {
            chip_benefit_ratio: if latest.benefit_part != 0.0 { Some(latest.benefit_part) } else { None },
            chip_avg_cost: if latest.avg_cost != 0.0 { Some(latest.avg_cost) } else { None },
            chip_concentration_90: if latest.pct_90_concentration != 0.0 { Some(latest.pct_90_concentration) } else { None },
            ..AShareEnrichmentData::default()
        })
    }

    async fn fetch_a_share_dividend(
        &self,
        code: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        let items = self.ak.stock_fhps_detail_em(code).await?;
        // Find the most recent dividend with a yield
        let best = items.iter().find_map(|item| {
            let yield_val = item.dividend_yield?;
            if yield_val > 0.0 { Some(yield_val) } else { None }
        });
        Ok(AShareEnrichmentData {
            dividend_yield: best,
            ..AShareEnrichmentData::default()
        })
    }

    async fn fetch_a_share_analyst(
        &self,
        code: &str,
    ) -> anyhow::Result<AShareEnrichmentData> {
        let items = self.ak.stock_research_report_em(code).await?;
        let report_count = items.len() as i64;
        // Count buy/strong-buy ratings vs total ratings
        let (buy_count, total_rated) = items.iter().fold((0i64, 0i64), |(buy, total), item| {
            if let Some(ref rating) = item.rating {
                let r = rating.trim();
                if !r.is_empty() {
                    let r_lower = r.to_lowercase();
                    let is_buy = r_lower.contains("buy") || r_lower.contains("overweight") || r_lower.contains("outperform") || r_lower.contains("strong") || r.contains("买入") || r.contains("增持");
                    (buy + i64::from(is_buy), total + 1)
                } else {
                    (buy, total)
                }
            } else {
                (buy, total)
            }
        });
        let buy_ratio = if total_rated > 0 { Some(buy_count as f64 / total_rated as f64) } else { None };
        Ok(AShareEnrichmentData {
            analyst_report_count: if report_count > 0 { Some(report_count) } else { None },
            analyst_buy_ratio: buy_ratio,
            ..AShareEnrichmentData::default()
        })
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
                title: "China Macro & Policy - NBS".to_string(),
                summary: "A-share macro reference: economic data, bulletins, and statistical releases.".to_string(),
                source: "stats.gov.cn".to_string(),
                url: Some("https://www.stats.gov.cn/".to_string()),
            },
            NewsItem {
                published_at: curr_date.to_string(),
                title: "China Monetary Policy & Financial Data - PBOC".to_string(),
                summary: "A-share macro reference: interest rates, liquidity, and financial statistics.".to_string(),
                source: "pbc.gov.cn".to_string(),
                url: Some("http://www.pbc.gov.cn/".to_string()),
            },
            NewsItem {
                published_at: curr_date.to_string(),
                title: "A-share Market Overview - Eastmoney".to_string(),
                summary: "A-share market reference: indices, sectors, capital flows, and news.".to_string(),
                source: "eastmoney.com".to_string(),
                url: Some("https://www.eastmoney.com/".to_string()),
            },
        ]
    }

    async fn fetch_eastmoney_main_finance_indicator(
        &self,
        secucode: &str,
    ) -> anyhow::Result<EastmoneyMainFinanceIndicatorItem> {
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
    ) -> anyhow::Result<akshare::stock::feature::BalanceSheet> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_balance_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare balance sheet failed")?;
        let first = sheets.first().context("akshare balance sheet returned no rows")?;
        Ok(first.clone())
    }

    async fn fetch_eastmoney_cashflow(
        &self,
        secucode: &str,
    ) -> anyhow::Result<akshare::stock::feature::CashFlowSheet> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_cash_flow_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare cashflow failed")?;
        let first = sheets.first().context("akshare cashflow returned no rows")?;
        Ok(first.clone())
    }

    async fn fetch_a_share_spot_quote(
        &self,
        symbol: &str,
    ) -> anyhow::Result<AkshareIndividualInfo> {
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
        Ok(AkshareIndividualInfo {
            stock_name: Some(spot.name.clone()),
            total_share: None,
            market_cap: Some(spot.total_market_cap),
            industry: None,
        })
    }

    async fn fetch_a_share_profit_sheet(
        &self,
        secucode: &str,
    ) -> anyhow::Result<akshare::stock::feature::ProfitSheet> {
        let symbol = secucode.split_once('.').map(|(c, _)| c).unwrap_or(secucode);
        let sheets = self
            .ak
            .stock_profit_sheet_by_report_em_typed(symbol)
            .await
            .context("akshare profit sheet failed")?;
        let first = sheets.first().context("akshare profit sheet returned no rows")?;
        Ok(first.clone())
    }

    pub(super) async fn fetch_a_share_fundamentals(
        &self,
        symbol: &str,
        ts_code: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        // Strip exchange suffix for APIs that expect bare code (e.g. "600519" not "600519.SH")
        let bare_code = symbol.split_once('.').map(|(c, _)| c).unwrap_or(symbol);
        let mut search_items = self
            .ak.a_share_search(bare_code, Some("A股"), 8)
            .await
            .unwrap_or_default();
        tracing::debug!(symbol, bare_code, search_count = search_items.len(), search_symbols = ?search_items.iter().map(|i| &i.symbol).collect::<Vec<_>>(), "a_share_search results");
        let search_match = search_items
            .drain(..)
            .find(|item| item.symbol == bare_code || item.symbol == symbol.trim())
            .or(None);
        tracing::debug!(symbol, search_match = ?search_match.as_ref().map(|m| &m.name), "search_match result");
        let info = self.fetch_a_share_individual_info(bare_code).await.ok();
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
        let provisional_market_cap: Option<f64> = spot_quote
            .as_ref()
            .and_then(|q| q.market_cap)
            .or(info.as_ref().and_then(|value| value.market_cap))
            .or_else(|| {
                let shares = provisional_shares_outstanding?;
                let quote = quote.as_ref()?;
                Some(quote.close * shares as f64)
            });
        let shares_outstanding = provisional_shares_outstanding.or_else(|| {
            let market_cap = provisional_market_cap?;
            let price = quote.as_ref()?.close;
            if price > 0.0 {
                Some((market_cap / price).round() as i64)
            } else {
                None
            }
        });
        let eastmoney_equity: Option<f64> = eastmoney_balance.as_ref().and_then(|item| item.equity)
            .or_else(|| eastmoney_main.as_ref().and_then(|item| {
                let shares = item.total_share?;
                let bps = item.bps?;
                Some(shares * bps)
            }));
        let eastmoney_assets: Option<f64> = eastmoney_balance.as_ref().and_then(|item| item.total_assets)
            .or_else(|| eastmoney_main.as_ref().and_then(|item| {
                let equity = eastmoney_equity?;
                let debt_ratio_pct = item.zcfzl?;
                let equity_ratio = 1.0 - (debt_ratio_pct / 100.0);
                (equity_ratio > 0.0).then_some(equity / equity_ratio)
            }));
        let eastmoney_liabilities: Option<f64> = eastmoney_balance
            .as_ref()
            .and_then(|item| item.total_liabilities)
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
            net_income_usd: profit_sheet
                .as_ref()
                .and_then(|p| p.net_profit_deducted.or(p.net_profit))
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.parent_net_profit.or(item.holder_profit))
                }),
            revenues_usd: profit_sheet
                .as_ref()
                .and_then(|p| p.total_revenue)
                .or_else(|| {
                    eastmoney_main
                        .as_ref()
                        .and_then(|item| item.total_operate_reve.or(item.operate_income))
                }),
            assets_usd: eastmoney_assets,
            liabilities_usd: eastmoney_liabilities,
            stockholders_equity_usd: eastmoney_equity,
            cash_and_equivalents_usd:
                eastmoney_balance.as_ref().and_then(|item| item.cash)
                    .or_else(|| eastmoney_cashflow.as_ref().and_then(|item| item.cash_increase)),
            gross_profit_usd: eastmoney_main
                .as_ref()
                .and_then(|item| item.gross_profit.or(item.mlr)),
            operating_income_usd: eastmoney_main
                .as_ref()
                .and_then(|item| item.operate_income),
            operating_expenses_usd: None,
            operating_cash_flow_usd:
                eastmoney_main.as_ref().and_then(|item| {
                    item.netcash_operate
                        .or(item.mgjyxjje.map(|per_share| {
                            item.total_share
                                .map(|shares| per_share * shares)
                                .unwrap_or(per_share)
                        }))
                })
                .or_else(|| eastmoney_cashflow.as_ref().and_then(|item| item.operating_cash_flow)),
            capital_expenditure_usd: eastmoney_main
                .as_ref()
                .and_then(|item| item.capital_expenditure)
                .map(f64::abs)
                .or_else(|| {
                    eastmoney_cashflow
                        .as_ref()
                        .and_then(|item| item.investing_cash_flow)
                        .map(f64::abs)
                }),
            free_cash_flow_usd:
                eastmoney_main.as_ref().and_then(|item| match (
                    item.netcash_operate,
                    item.capital_expenditure.map(f64::abs),
                ) {
                    (Some(ocf), Some(capex)) => Some(ocf - capex),
                    _ => None,
                })
                .or_else(|| {
                    eastmoney_cashflow.as_ref().and_then(|item| match (
                        item.operating_cash_flow,
                        item.investing_cash_flow.map(f64::abs),
                    ) {
                        (Some(ocf), Some(capex)) => Some(ocf - capex),
                        _ => None,
                    })
                }),
            long_term_debt_usd: None,
            current_debt_usd:
                eastmoney_main
                    .as_ref()
                    .and_then(|item| item.current_liab.or(item.current_liability)),
            total_debt_usd:
                eastmoney_main.as_ref().and_then(|item| match (
                    item.current_liab.or(item.current_liability),
                    item.totalnoncliab.or(item.noncurrent_liab_1year),
                ) {
                    (Some(current), Some(noncurrent)) => Some(current + noncurrent),
                    (Some(current), None) => Some(current),
                    (None, Some(noncurrent)) => Some(noncurrent),
                    (None, None) => None,
                }),
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

        // Fetch eastmoney announcements + stock news (by name for much better coverage) in parallel.
        let search_match_owned = search_match.clone();
        let (eastmoney_result, em_news_result) = tokio::join!(
            self.fetch_a_share_eastmoney_news(ts_code, limit),
            async {
                self.ak.stock_news_em_by_name(&search_match_owned).await
                    .map(|items| items.into_iter().map(super::news_item_from_stock_news).collect::<Vec<_>>())
                    .unwrap_or_default()
            },
        );
        let result = Self::merge_a_share_news(
            ts_code,
            limit,
            eastmoney_result,
            em_news_result,
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
                    let source = name.to_string();
                    akshare_items.extend(
                        items
                            .into_iter()
                            .map(|n| super::news_item_from_news_entry_with_source(n, &source)),
                    );
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
                super::news::normalized_news_date(&item.published_at)
                    .is_some_and(|date| date.starts_with(year))
            })
            .cloned()
            .collect();
        let selected_items = if filtered_by_year.is_empty() {
            merged
        } else {
            filtered_by_year
        };
        let merged = super::news::merge_ranked_news(
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
        let ranked = super::news::merge_ranked_news(merged, target_limit, None, None, &keywords);
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
        let stock_name = find_value("股票简称").and_then(|v| v.as_str()).map(String::from);
        tracing::debug!(symbol, items_count = items.len(), stock_name = ?stock_name, "individual_info result");
        Ok(AkshareIndividualInfo {
            stock_name,
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
        let mut items = ak_candles
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
        if start_price <= 0.0 {
            return Ok(None);
        }
        Ok(Some((end_price - start_price) / start_price))
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
