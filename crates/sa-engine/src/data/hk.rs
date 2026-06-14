use anyhow::Context;
use super::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use super::news_item_from_stock_news;

impl MarketDataClient {
    pub(super) fn hk_standard_code(&self, symbol: &str) -> anyhow::Result<String> {
        let normalized = self
            .normalize_hk_symbol(symbol)
            .context("invalid HK symbol")?;
        Ok(normalized.trim_end_matches(".HK").to_string())
    }

    pub(super) async fn fetch_hk_fundamentals(
        &self,
        symbol: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let search_code = self.hk_standard_code(symbol)?;

        // Fetch typed APIs
        let indicators = self
            .ak
            .stock_financial_hk_analysis_indicator_em_typed(&search_code, "报告期")
            .await
            .unwrap_or_default();
        let main = indicators.first();

        let balance_sheets = self
            .ak
            .stock_financial_hk_balance_sheet_typed(&search_code, "报告期")
            .await
            .unwrap_or_default();
        let bs = balance_sheets.first();

        let income_sheets = self
            .ak
            .stock_financial_hk_income_sheet_typed(&search_code, "报告期")
            .await
            .unwrap_or_default();
        let is = income_sheets.first();

        let cashflow_sheets = self
            .ak
            .stock_financial_hk_cashflow_sheet_typed(&search_code, "报告期")
            .await
            .unwrap_or_default();
        let cf = cashflow_sheets.first();

        // Resolve company name via search
        let mut items = self
            .ak
            .a_share_search(&search_code, Some("港股"), 8)
            .await?;
        let matched = items
            .drain(..)
            .find(|item| item.symbol.eq_ignore_ascii_case(&search_code));

        let company_name = matched
            .as_ref()
            .map(|item| item.name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| symbol.to_uppercase());

        // Extract from main indicator with income sheet fallbacks
        let currency = main
            .and_then(|m| m.currency.clone())
            .unwrap_or_else(|| "HKD".to_string());
        let fiscal_year_end = main.and_then(|m| {
            m.report_date
                .clone()
                .or_else(|| m.std_report_date.clone())
        });
        let operate_income = main
            .and_then(|m| m.operate_income)
            .or_else(|| is.and_then(|s| s.total_revenue));
        let holder_profit = main
            .and_then(|m| m.holder_profit)
            .or_else(|| is.and_then(|s| s.net_profit));
        let gross_profit = main
            .and_then(|m| m.gross_profit)
            .or_else(|| is.and_then(|s| s.gross_profit));
        let total_assets = main
            .and_then(|m| m.total_assets)
            .or_else(|| bs.and_then(|b| b.total_assets));
        let total_liabilities = main
            .and_then(|m| m.total_liabilities)
            .or_else(|| bs.and_then(|b| b.total_liabilities));
        let total_parent_equity = main
            .and_then(|m| m.total_parent_equity)
            .or_else(|| bs.and_then(|b| b.equity))
            .or_else(|| {
                // Derive equity from total_assets - total_liabilities
                let assets = main
                    .and_then(|m| m.total_assets)
                    .or_else(|| bs.and_then(|b| b.total_assets))?;
                let liabilities = main
                    .and_then(|m| m.total_liabilities)
                    .or_else(|| bs.and_then(|b| b.total_liabilities))?;
                let eq = assets - liabilities;
                if eq > 0.0 { Some(eq) } else { None }
            });
        let netcash_operate = main.and_then(|m| m.netcash_operate);
        let capital_expenditure = main.and_then(|m| m.capital_expenditure);
        let total_share = main.and_then(|m| m.total_share);
        let current_liability = main
            .and_then(|m| m.current_liability)
            .or_else(|| bs.and_then(|b| b.short_term_debt));
        let noncurrent_liab_1year = main
            .and_then(|m| m.noncurrent_liab_1year)
            .or_else(|| bs.and_then(|b| b.long_term_debt));

        // From typed balance sheet
        let cash_and_equivalents = bs.and_then(|b| b.cash);
        let long_term_debt = bs.and_then(|b| b.long_term_debt);
        let short_term_debt = bs.and_then(|b| b.short_term_debt);

        // Operating expenses from income sheet
        let operating_income = operate_income;
        let operating_expenses = is.and_then(|s| s.operating_expenses).or_else(|| {
            // Cross-validate with gross_profit and operating_income
            if let (Some(gp), Some(oi)) = (gross_profit, operating_income) {
                let derived = gp - oi;
                if derived > 0.0 {
                    return Some(derived);
                }
            }
            None
        });

        // Cashflow fallbacks
        let netcash_operate = netcash_operate.or_else(|| cf.and_then(|c| c.operating_cash_flow));
        let capital_expenditure_detail =
            capital_expenditure.or_else(|| cf.and_then(|c| c.capital_expenditure));

        let shares_outstanding = total_share.map(|value| (value * 10_000.0).round() as i64);

        // Compute market cap: try Eastmoney famous stocks API first, then Tencent, then quote fallback
        let market_cap = match self.ak.stock_hk_famous_spot_em().await {
            Ok(stocks) => stocks
                .iter()
                .find(|s| s.code.eq_ignore_ascii_case(&search_code))
                .and_then(|s| s.market_cap),
            Err(_) => None,
        };
        let market_cap = if market_cap.is_some() {
            market_cap
        } else {
            // Fallback: try Tencent API
            match self.ak.hk_market_cap_from_tencent(&search_code).await {
                Ok(Some(cap)) => Some(cap),
                _ => {
                    // Fallback: compute from quote and shares_outstanding
                    self.fetch_hk_quote(symbol).await.ok().and_then(|(q, _)| {
                        shares_outstanding.map(|shares| q.close * shares as f64)
                    })
                }
            }
        };

        let noncurrent_liabilities: Option<f64> = None;

        let total_debt = match (current_liability, noncurrent_liab_1year) {
            (Some(current), Some(noncurrent)) => Some(current + noncurrent),
            (Some(current), None) => Some(current),
            (None, Some(noncurrent)) => Some(noncurrent),
            (None, None) => match (short_term_debt, long_term_debt.or(noncurrent_liabilities)) {
                (Some(current), Some(noncurrent)) => Some(current + noncurrent),
                (Some(current), None) => Some(current),
                (None, Some(noncurrent)) => Some(noncurrent),
                (None, None) => None,
            },
        };

        // Resolve industry from Eastmoney individual stock info API
        let industry = self.resolve_hk_industry(&search_code).await;

        let snapshot = FundamentalsSnapshot {
            symbol: symbol.to_uppercase(),
            company_name,
            cik: String::new(),
            industry,
            currency,
            fiscal_year_end,
            shares_outstanding,
            market_cap,
            net_income_usd: holder_profit,
            revenues_usd: operate_income,
            assets_usd: total_assets,
            liabilities_usd: total_liabilities,
            stockholders_equity_usd: total_parent_equity,
            cash_and_equivalents_usd: cash_and_equivalents,
            gross_profit_usd: gross_profit,
            operating_income_usd: operating_income,
            operating_expenses_usd: operating_expenses,
            operating_cash_flow_usd: netcash_operate,
            capital_expenditure_usd: capital_expenditure_detail.map(f64::abs),
            free_cash_flow_usd: {
                let ocf = netcash_operate;
                let capex = capital_expenditure_detail.map(f64::abs);
                match (ocf, capex) {
                    (Some(o), Some(c)) => Some(o - c),
                    _ => None,
                }
            },
            long_term_debt_usd: long_term_debt,
            current_debt_usd: current_liability.or(short_term_debt),
            total_debt_usd: total_debt,
            diluted_shares_outstanding: shares_outstanding,
        };
        Ok(snapshot)
    }

    pub(super) async fn fetch_hk_news(
        &self,
        symbol: &str,
        limit: usize,
        _start_date: Option<&str>,
        _end_date: Option<&str>,
    ) -> anyhow::Result<Vec<NewsItem>> {
        let standard_code = self.hk_standard_code(symbol)?;
        let stock_news = self
            .ak
            .stock_news_em_hk(&standard_code)
            .await
            .context("failed to fetch HK news from Eastmoney via akshare")?;
        let items: Vec<NewsItem> = stock_news
            .into_iter()
            .map(news_item_from_stock_news)
            .take(limit)
            .collect();
        if items.is_empty() {
            anyhow::bail!("no HK news available from Eastmoney");
        }
        Ok(items)
    }

    pub(super) async fn fetch_hk_global_news(
        &self,
        curr_date: &str,
        look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<crate::types::NewsFetchResult> {
        use super::news::within_date_window;
        use crate::types::{NewsFetchAttempt, NewsFetchResult};

        let end = chrono::NaiveDate::parse_from_str(curr_date, "%Y-%m-%d")
            .context("invalid curr_date for HK global news")?;
        let start = end - chrono::Days::new(look_back_days as u64);
        let start_text = start.to_string();

        let queries: &[(&str, Option<&str>)] = &[
            ("Hong Kong stock market", Some("en")),
            ("Hang Seng index", Some("en")),
            ("港股 恒生", None),
            ("HK stocks earnings", Some("en")),
        ];

        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();
        let mut existing_titles: std::collections::HashSet<String> =
            items.iter().map(|i| i.title.to_lowercase()).collect();

        for &(query, lang) in queries {
            if let Ok(rss_items) = self.ak.bing_news_rss_with_lang(query, 10, lang).await {
                let count = rss_items.len();
                for item in rss_items.into_iter() {
                    if within_date_window(&item.published_at, Some(&start_text), Some(curr_date))
                        && !existing_titles.contains(&item.title.to_lowercase())
                    {
                        existing_titles.insert(item.title.to_lowercase());
                        items.push(item);
                    }
                }
                attempts.push(NewsFetchAttempt {
                    source: "bing_rss".to_string(),
                    query: Some(query.to_string()),
                    success: true,
                    item_count: count,
                    error: None,
                });
            }
        }

        if items.is_empty() {
            anyhow::bail!("no HK global/macro news available from current upstreams");
        }

        items.truncate(limit.max(8));
        let cacheable = super::news_result_cacheable(&items, &attempts);
        Ok(NewsFetchResult {
            items,
            attempts,
            cacheable,
        })
    }

    pub(super) async fn fetch_hk_quote(
        &self,
        symbol: &str,
    ) -> anyhow::Result<(super::QuoteSnapshot, String)> {
        let ak_quote = self.ak.hk_quote(symbol).await?;
        Ok((ak_quote, "akshare".to_string()))
    }

    pub(super) async fn fetch_hk_candles(
        &self,
        symbol: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<super::CandlePoint>> {
        use anyhow::Context;

        let ak_candles = self
            .ak
            .hk_candles(symbol, limit)
            .await
            .context("failed to fetch HK candles from akshare")?;
        Ok(ak_candles)
    }

    pub(super) async fn fetch_hk_return_since(
        &self,
        symbol: &str,
        start_date: &str,
        holding_days: usize,
    ) -> anyhow::Result<Option<f64>> {
        let candles = self.fetch_hk_candles(symbol, holding_days + 15).await?;
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
        if start_price <= 0.0 {
            return Ok(None);
        }
        Ok(Some((end_price - start_price) / start_price))
    }

    /// Try to resolve HK stock industry from Eastmoney individual stock info API.
    /// Uses secid format: 116.{code} for Hong Kong stocks.
    pub(super) async fn resolve_hk_industry(&self, symbol: &str) -> Option<String> {
        // Normalize to 5-digit HK code
        let code = symbol.trim().trim_start_matches('0');
        let code = if code.is_empty() { "0" } else { code };
        let code = format!("{:0>5}", code);
        let secid = format!("116.{code}");
        self.ak.stock_info_by_secid(&secid).await.ok().flatten()
    }

    /// Fetch enrichment data for HK stocks.
    /// Tries Baidu valuation API for PE and PB, falls back to computing PE from fundamentals.
    pub(crate) async fn fetch_hk_enrichment(
        &self,
        symbol: &str,
    ) -> anyhow::Result<super::a_share::AShareEnrichmentData> {
        let code = self.hk_standard_code(symbol)?;
        // Try Baidu valuation API for PE and PB
        let pe = self
            .ak
            .stock_hk_valuation_baidu(&code, "pe_ttm", "monthly")
            .await
            .ok()
            .and_then(|items| items.last().map(|v| v.value))
            .filter(|v| *v != 0.0);
        let pb = self
            .ak
            .stock_hk_valuation_baidu(&code, "pb", "monthly")
            .await
            .ok()
            .and_then(|items| items.last().map(|v| v.value))
            .filter(|v| *v != 0.0);
        if pe.is_some() || pb.is_some() {
            tracing::debug!(symbol, ?pe, ?pb, "hk_enrichment from baidu");
            return Ok(super::a_share::AShareEnrichmentData {
                pe_ttm: pe,
                pb,
                ..super::a_share::AShareEnrichmentData::default()
            });
        }
        // Fallback: compute PE from fundamentals
        let fundamentals = self.fetch_hk_fundamentals(symbol).await.ok();
        let pe_ttm = fundamentals.as_ref().and_then(|f| {
            let mc = f.market_cap?;
            let ni = f.net_income_usd?;
            if ni > 0.0 { Some(mc / ni) } else { None }
        });
        tracing::debug!(symbol, ?pe_ttm, "hk_enrichment fallback from fundamentals");
        Ok(super::a_share::AShareEnrichmentData {
            pe_ttm,
            ..super::a_share::AShareEnrichmentData::default()
        })
    }
}
