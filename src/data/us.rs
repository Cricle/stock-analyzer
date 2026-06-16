use anyhow::{Context, bail};
use chrono::NaiveDate;

use super::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::types::{NewsFetchAttempt, NewsFetchResult};
use super::news::within_date_window;

impl MarketDataClient {
    /// Fetch US company name from Eastmoney using secid format.
    /// Tries NASDAQ (105) then NYSE (106). Has a 5-second timeout.
    async fn fetch_us_company_name(&self, symbol: &str) -> Option<String> {
        let timeout = std::time::Duration::from_secs(5);
        for market_code in &["105", "106"] {
            let secid = format!("{}.{}", market_code, symbol.to_uppercase());
            let result = tokio::time::timeout(
                timeout,
                self.ak.stock_individual_info_em(&secid),
            ).await;
            if let Ok(Ok(info)) = result {
                for item in &info {
                    if item.item == "股票简称"
                        && let Some(name) = item.value.as_str()
                        && !name.is_empty()
                    {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    pub(super) async fn fetch_us_fundamentals(
        &self,
        symbol: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let t0 = std::time::Instant::now();

        // Fetch typed APIs + company name in parallel
        let (indicators, balance_sheets, income_sheets, cashflow_sheets, company_name) =
            tokio::join!(
                self.ak.stock_financial_us_analysis_indicator_em_typed(symbol, "年报"),
                self.ak.stock_financial_us_balance_sheet_typed(symbol, "年报"),
                self.ak.stock_financial_us_income_sheet_typed(symbol, "年报"),
                self.ak.stock_financial_us_cashflow_sheet_typed(symbol, "年报"),
                self.fetch_us_company_name(symbol),
            );

        let indicators = indicators.unwrap_or_default();
        let main = indicators.first();
        let balance_sheets = balance_sheets.unwrap_or_default();
        let bs = balance_sheets.first();
        let income_sheets = income_sheets.unwrap_or_default();
        let is = income_sheets.first();
        let cashflow_sheets = cashflow_sheets.unwrap_or_default();
        let cf = cashflow_sheets.first();

        tracing::debug!(
            symbol,
            indicators = indicators.len(),
            balance_sheets = balance_sheets.len(),
            income_sheets = income_sheets.len(),
            "fetch_us_fundamentals: akshare reports took {}ms",
            t0.elapsed().as_millis()
        );

        // Use Eastmoney name if available, otherwise fall back to symbol
        let company_name = company_name.unwrap_or_else(|| symbol.to_string());

        // From main indicator (may be empty for some US stocks)
        let net_income = main
            .and_then(|m| m.holder_profit)
            .or_else(|| is.and_then(|s| s.net_profit));
        let revenue = main
            .and_then(|m| m.operate_income)
            .or_else(|| is.and_then(|s| s.total_revenue));
        // shares_outstanding: try indicator TOTAL_SHARE, then compute from equity / BPS
        let shares_outstanding = main
            .and_then(|m| m.total_share)
            .map(|v| v as i64)
            .or_else(|| {
                let bps = main.and_then(|m| m.bps)?;
                let equity = bs.and_then(|b| b.equity)?;
                if bps > 0.0 { Some((equity / bps).round() as i64) } else { None }
            });
        // From typed balance sheet
        let assets = bs.and_then(|b| b.total_assets);
        let liabilities = bs.and_then(|b| b.total_liabilities);
        let stockholders_equity = bs.and_then(|b| b.equity);
        let cash = bs.and_then(|b| b.cash);
        let long_term_debt = bs.and_then(|b| b.long_term_debt);
        let current_debt = bs.and_then(|b| b.short_term_debt);
        let total_debt = liabilities;

        // From typed income sheet
        let gross_profit = is.and_then(|s| s.gross_profit).or_else(|| main.and_then(|m| m.gross_profit));
        let operating_income = is.and_then(|s| s.operating_profit).or_else(|| main.and_then(|m| m.operate_income));
        let operating_expenses = is.and_then(|s| s.operating_expenses);

        // From typed cashflow sheet
        let operating_cash_flow = cf.and_then(|c| c.operating_cash_flow);
        let capital_expenditure = cf.and_then(|c| c.capital_expenditure);

        let free_cash_flow = match (operating_cash_flow, capital_expenditure) {
            (Some(ocf), Some(capex)) => Some(ocf - capex),
            _ => None,
        };

        // Get quote for market cap calculation
        let quote = self.fetch_quote(symbol).await.ok();
        let market_cap = quote.as_ref().and_then(|q| {
            shares_outstanding.map(|shares| q.close * shares as f64)
        });
        // Fallback: try Sina API for market cap
        let market_cap = if market_cap.is_some() {
            market_cap
        } else {
            match self.ak.us_market_cap_from_sina(symbol).await {
                Ok(Some(cap)) => {
                    tracing::debug!("got market_cap from Sina for {}: {}", symbol, cap);
                    Some(cap)
                }
                Ok(None) => {
                    tracing::debug!("no market_cap from Sina for {}", symbol);
                    None
                }
                Err(e) => {
                    tracing::debug!("Sina market_cap error for {}: {}", symbol, e);
                    None
                }
            }
        };

        // Fetch Yahoo Finance key stats for additional data
        let yf_stats = self.ak.us_stock_key_stats(symbol).await.ok();

        // Fill in missing data from Yahoo Finance
        let shares_outstanding = shares_outstanding.or_else(|| {
            yf_stats.as_ref().and_then(|s| s.shares_outstanding.map(|v| v as i64))
        });
        let diluted_shares = shares_outstanding;
        let market_cap = market_cap.or_else(|| yf_stats.as_ref().and_then(|s| s.market_cap));
        let net_income = net_income.or_else(|| yf_stats.as_ref().and_then(|s| s.net_income));
        let revenue = revenue.or_else(|| yf_stats.as_ref().and_then(|s| s.revenue));

        // Resolve industry from Yahoo Finance with static fallback
        let industry = self.resolve_us_industry(symbol).await;

        Ok(FundamentalsSnapshot {
            symbol: symbol.to_uppercase(),
            company_name,
            cik: String::new(),
            industry,
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding,
            market_cap,
            net_income_usd: net_income,
            revenues_usd: revenue,
            assets_usd: assets,
            liabilities_usd: liabilities,
            stockholders_equity_usd: stockholders_equity,
            cash_and_equivalents_usd: cash,
            gross_profit_usd: gross_profit,
            operating_income_usd: operating_income,
            operating_expenses_usd: operating_expenses,
            operating_cash_flow_usd: operating_cash_flow,
            capital_expenditure_usd: capital_expenditure,
            free_cash_flow_usd: free_cash_flow,
            long_term_debt_usd: long_term_debt,
            current_debt_usd: current_debt,
            total_debt_usd: total_debt,
            diluted_shares_outstanding: diluted_shares,
        })
    }

    /// Fetch enrichment data for US stocks.
    /// Fetches Yahoo Finance stats and Eastmoney income sheet data in parallel.
    pub(crate) async fn fetch_us_enrichment(
        &self,
        symbol: &str,
    ) -> anyhow::Result<super::a_share::AShareEnrichmentData> {
        // Fetch Yahoo Finance stats, income sheet, and analysis indicators in parallel
        let (yf_stats, income_sheets, analysis_indicators) = tokio::join!(
            self.ak.us_stock_key_stats(symbol),
            self.ak.stock_financial_us_income_sheet_typed(symbol, "年报"),
            self.ak.stock_financial_us_analysis_indicator_em_typed(symbol, "年报"),
        );

        // Yahoo Finance: PE, PB, gross_margin, dividend_yield
        let (pe_ttm, pb, gross_margin_yf, dividend_yield) = match yf_stats {
            Ok(stats) => {
                tracing::debug!(symbol, "us_enrichment from yahoo finance");
                (stats.trailing_pe, stats.price_to_book, stats.gross_margin, stats.dividend_yield)
            }
            Err(e) => {
                tracing::debug!(symbol, error = %e, "yahoo finance key_stats failed");
                (None, None, None, None)
            }
        };

        // Income sheet: revenue_yoy, net_profit_yoy, gross_margin
        let (revenue_yoy, net_profit_yoy, gross_margin_em) = match income_sheets {
            Ok(sheets) => {
                let first = sheets.first();
                // Eastmoney returns YoY as percentages (15.0 = 15%), convert to decimal
                let rev_yoy = first.and_then(|s| s.total_revenue_yoy).filter(|v| *v != 0.0).map(|v| v / 100.0);
                let np_yoy = first.and_then(|s| s.net_profit_yoy).filter(|v| *v != 0.0).map(|v| v / 100.0);
                let gm = first.and_then(|s| s.gross_margin).filter(|v| *v != 0.0).map(|v| v / 100.0);
                (rev_yoy, np_yoy, gm)
            }
            Err(e) => {
                tracing::debug!(symbol, error = %e, "us income sheet failed");
                (None, None, None)
            }
        };

        // Fallback: compute YoY from analysis indicators if income sheet didn't provide it
        let revenue_yoy = if revenue_yoy.is_some() {
            revenue_yoy
        } else {
            analysis_indicators.as_ref().ok().and_then(|indicators| {
                if indicators.len() < 2 { return None; }
                let curr = &indicators[0];
                let curr_date_str = curr.std_report_date.as_deref()
                    .or(curr.report_date.as_deref());
                let prev = curr_date_str.and_then(|curr_date| {
                    let curr_md = curr_date.get(5..10)?;
                    indicators.iter().skip(1).find(|ind| {
                        let ind_date = ind.std_report_date.as_deref()
                            .or(ind.report_date.as_deref());
                        ind_date.and_then(|d| d.get(5..10))
                            .is_some_and(|md| md == curr_md)
                    })
                }).unwrap_or(&indicators[1]);
                match (curr.operate_income, prev.operate_income) {
                    (Some(c), Some(p)) if p > 0.0 && c != 0.0 => Some((c - p) / p),
                    _ => None,
                }
            })
        };
        let net_profit_yoy = if net_profit_yoy.is_some() {
            net_profit_yoy
        } else {
            analysis_indicators.as_ref().ok().and_then(|indicators| {
                if indicators.len() < 2 { return None; }
                let curr = &indicators[0];
                let curr_date_str = curr.std_report_date.as_deref()
                    .or(curr.report_date.as_deref());
                let prev = curr_date_str.and_then(|curr_date| {
                    let curr_md = curr_date.get(5..10)?;
                    indicators.iter().skip(1).find(|ind| {
                        let ind_date = ind.std_report_date.as_deref()
                            .or(ind.report_date.as_deref());
                        ind_date.and_then(|d| d.get(5..10))
                            .is_some_and(|md| md == curr_md)
                    })
                }).unwrap_or(&indicators[1]);
                let np_curr = curr.holder_profit;
                let np_prev = prev.holder_profit;
                match (np_curr, np_prev) {
                    (Some(c), Some(p)) if p > 0.0 && c != 0.0 => Some((c - p) / p),
                    _ => None,
                }
            })
        };

        // Use Yahoo gross_margin if available, otherwise Eastmoney
        let gross_margin = gross_margin_yf.or(gross_margin_em);

        // Fallback: if PE still missing, try Baidu then fundamentals
        let pe_ttm = if pe_ttm.is_some() {
            pe_ttm
        } else {
            let baidu_pe = self
                .ak
                .stock_us_valuation_baidu(symbol, "pe_ttm", "monthly")
                .await
                .ok()
                .and_then(|items| items.last().map(|v| v.value))
                .filter(|v| *v != 0.0);
            if baidu_pe.is_some() {
                tracing::debug!(symbol, ?baidu_pe, "us_enrichment pe from baidu");
                baidu_pe
            } else {
                let fundamentals = self.fetch_us_fundamentals(symbol).await.ok();
                fundamentals.as_ref().and_then(|f| {
                    let mc = f.market_cap?;
                    let ni = f.net_income_usd?;
                    if ni > 0.0 { Some(mc / ni) } else { None }
                })
            }
        };

        // Fallback: if PB still missing, try Baidu
        let pb = if pb.is_some() {
            pb
        } else {
            let baidu_pb = self
                .ak
                .stock_us_valuation_baidu(symbol, "pb", "monthly")
                .await
                .ok()
                .and_then(|items| items.last().map(|v| v.value))
                .filter(|v| *v != 0.0);
            if baidu_pb.is_some() {
                tracing::debug!(symbol, ?baidu_pb, "us_enrichment pb from baidu");
            }
            baidu_pb
        };

        // Industry from Yahoo Finance
        let industry = self.resolve_us_industry(symbol).await;

        Ok(super::a_share::AShareEnrichmentData {
            pe_ttm,
            pb,
            revenue_yoy,
            net_profit_yoy,
            gross_margin,
            dividend_yield,
            industry,
            ..super::a_share::AShareEnrichmentData::default()
        })
    }

    pub(super) async fn fetch_us_news(
        &self,
        symbol: &str,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> anyhow::Result<NewsFetchResult> {
        use super::news::is_junk_news;

        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();
        let mut existing_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Primary source: Finnhub
        if let Some(api_key) = self.api_keys.next_finnhub_key() {
            let from = start_date.unwrap_or("2020-01-01");
            let to = end_date.unwrap_or("2099-12-31");
            match self.ak.finnhub_company_news(symbol, from, to, api_key).await {
                Ok(finnhub_items) => {
                    let count = finnhub_items.len();
                    for item in finnhub_items.into_iter() {
                        if !existing_titles.contains(&item.title.to_lowercase()) {
                            existing_titles.insert(item.title.to_lowercase());
                            items.push(item);
                        }
                    }
                    let added = items.len();
                    attempts.push(NewsFetchAttempt {
                        source: "finnhub".to_string(),
                        query: Some(symbol.to_string()),
                        success: added > 0,
                        item_count: count,
                        error: if added == 0 {
                            Some("no Finnhub results".to_string())
                        } else {
                            None
                        },
                    });
                }
                Err(error) => {
                    tracing::warn!(symbol, error = %error, "finnhub_company_news failed");
                    attempts.push(NewsFetchAttempt {
                        source: "finnhub".to_string(),
                        query: Some(symbol.to_string()),
                        success: false,
                        item_count: 0,
                        error: Some(error.to_string()),
                    });
                }
            }
        } else {
            tracing::debug!("no Finnhub API key configured, skipping");
        }

        // Secondary: Eastmoney US stock news
        if items.len() < limit.min(6) {
            match self.ak.stock_news_em_us(symbol).await {
                Ok(ak_news) => {
                    let count = ak_news.len();
                    let converted: Vec<NewsItem> = ak_news
                        .into_iter()
                        .map(super::news_item_from_stock_news)
                        .filter(|item| {
                            within_date_window(&item.published_at, start_date, end_date)
                                && !existing_titles.contains(&item.title.to_lowercase())
                                && !is_junk_news(item)
                        })
                        .collect();
                    let added = converted.len();
                    for item in &converted {
                        existing_titles.insert(item.title.to_lowercase());
                    }
                    attempts.push(NewsFetchAttempt {
                        source: "eastmoney_us".to_string(),
                        query: Some(symbol.to_string()),
                        success: added > 0,
                        item_count: count,
                        error: if added == 0 {
                            Some("no usable Eastmoney US results".to_string())
                        } else {
                            None
                        },
                    });
                    items.extend(converted);
                }
                Err(error) => {
                    attempts.push(NewsFetchAttempt {
                        source: "eastmoney_us".to_string(),
                        query: Some(symbol.to_string()),
                        success: false,
                        item_count: 0,
                        error: Some(error.to_string()),
                    });
                }
            }
        }

        // Fallback: Bing RSS
        if items.len() < limit.min(4) {
            let queries = [
                format!("{} stock news today", symbol),
                format!("{} earnings report", symbol),
            ];
            let mut bing_added = 0usize;
            for query in &queries {
                if let Ok(rss_items) =
                    self.ak.bing_news_rss_with_lang(query, 10, Some("en")).await
                {
                    for item in rss_items.into_iter() {
                        if is_junk_news(&item) {
                            continue;
                        }
                        if within_date_window(&item.published_at, start_date, end_date)
                            && !existing_titles.contains(&item.title.to_lowercase())
                        {
                            existing_titles.insert(item.title.to_lowercase());
                            items.push(item);
                            bing_added += 1;
                        }
                    }
                }
            }
            attempts.push(NewsFetchAttempt {
                source: "bing_rss".to_string(),
                query: Some(format!("{} stock news/earnings", symbol)),
                success: bing_added > 0,
                item_count: bing_added,
                error: if bing_added == 0 {
                    Some("no usable Bing RSS results".to_string())
                } else {
                    None
                },
            });
        }

        if items.is_empty() {
            bail!("no US company news available from current upstreams");
        }

        items.truncate(limit.max(8));
        let cacheable = super::news_result_cacheable(&items, &attempts);
        Ok(NewsFetchResult {
            items,
            attempts,
            cacheable,
        })
    }

    pub(super) async fn fetch_us_global_news(
        &self,
        curr_date: &str,
        look_back_days: usize,
        limit: usize,
    ) -> anyhow::Result<NewsFetchResult> {
        use super::news::is_junk_news;

        let end = NaiveDate::parse_from_str(curr_date, "%Y-%m-%d")
            .context("invalid curr_date for global news")?;
        let start = end - chrono::Days::new(look_back_days as u64);
        let start_text = start.to_string();

        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();
        let mut existing_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Primary: Finnhub for major market ETFs
        if let Some(api_key) = self.api_keys.next_finnhub_key() {
            let major_symbols = ["SPY", "QQQ", "DIA"];
            for sym in &major_symbols {
                match self
                    .ak
                    .finnhub_company_news(sym, &start_text, curr_date, api_key)
                    .await
                {
                    Ok(finnhub_items) => {
                        let count = finnhub_items.len();
                        let mut added = 0usize;
                        for item in finnhub_items.into_iter() {
                            if !existing_titles.contains(&item.title.to_lowercase()) {
                                existing_titles.insert(item.title.to_lowercase());
                                items.push(item);
                                added += 1;
                            }
                        }
                        attempts.push(NewsFetchAttempt {
                            source: "finnhub".to_string(),
                            query: Some(sym.to_string()),
                            success: added > 0,
                            item_count: count,
                            error: None,
                        });
                    }
                    Err(error) => {
                        tracing::warn!(symbol = sym, error = %error, "finnhub global news failed");
                        attempts.push(NewsFetchAttempt {
                            source: "finnhub".to_string(),
                            query: Some(sym.to_string()),
                            success: false,
                            item_count: 0,
                            error: Some(error.to_string()),
                        });
                    }
                }
            }
        }

        // Fallback: Bing RSS
        if items.len() < limit.min(8) {
            let queries = [
                "stock market economy",
                "Federal Reserve interest rates",
                "global markets trading",
            ];
            for query in &queries {
                if let Ok(rss_items) = self.ak.bing_news_rss(query, 10).await {
                    let count = rss_items.len();
                    let mut kept = 0usize;
                    for item in rss_items.into_iter() {
                        if is_junk_news(&item) {
                            continue;
                        }
                        if within_date_window(
                            &item.published_at,
                            Some(&start_text),
                            Some(curr_date),
                        ) && !existing_titles.contains(&item.title.to_lowercase())
                        {
                            existing_titles.insert(item.title.to_lowercase());
                            items.push(item);
                            kept += 1;
                        }
                    }
                    attempts.push(NewsFetchAttempt {
                        source: "bing_rss".to_string(),
                        query: Some(query.to_string()),
                        success: kept > 0,
                        item_count: count,
                        error: if kept == 0 {
                            Some("all items filtered as junk".to_string())
                        } else {
                            None
                        },
                    });
                }
            }
        }

        if items.is_empty() {
            bail!("no US global/macro news available from current upstreams");
        }

        items.truncate(limit.max(8));
        let cacheable = super::news_result_cacheable(&items, &attempts);
        Ok(NewsFetchResult {
            items,
            attempts,
            cacheable,
        })
    }

    pub(super) async fn fetch_us_quote(
        &self,
        symbol: &str,
    ) -> anyhow::Result<(super::QuoteSnapshot, String)> {
        match self.ak.us_quote(symbol).await {
            Ok(quote) => Ok((quote, "akshare".to_string())),
            Err(e) => {
                tracing::debug!(symbol, error = %e, "us_quote failed, falling back to us_candles(2)");
                let mut candles = self.ak.us_candles(symbol, 2).await?;
                let last = candles
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("no US candle data for quote fallback"))?;
                Ok((
                    super::QuoteSnapshot {
                        symbol: symbol.to_uppercase(),
                        date: last.trade_date,
                        open: last.open,
                        high: last.high,
                        low: last.low,
                        close: last.close,
                        volume: last.volume,
                    },
                    "akshare-candles".to_string(),
                ))
            }
        }
    }

    pub(super) async fn fetch_us_candles(
        &self,
        symbol: &str,
        limit: usize,
    ) -> anyhow::Result<(Vec<super::CandlePoint>, String)> {
        let ak_candles = self.ak.us_candles(symbol, limit).await?;
        Ok((ak_candles, "akshare".to_string()))
    }

    pub(super) async fn fetch_us_insider_transactions(
        &self,
        _symbol: &str,
    ) -> anyhow::Result<Vec<NewsItem>> {
        bail!("US insider transactions require SEC EDGAR access which is no longer available");
    }

    pub(super) async fn fetch_us_return_since(
        &self,
        symbol: &str,
        start_date: &str,
        holding_days: usize,
    ) -> anyhow::Result<Option<f64>> {
        NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
            .with_context(|| format!("invalid start_date for return_since: {start_date}"))?;
        let limit = holding_days + 15;
        let ak_candles = self
            .ak
            .us_candles(symbol, limit)
            .await
            .with_context(|| format!("failed to fetch US candles for {symbol}"))?;
        if ak_candles.is_empty() {
            bail!("historical quote request returned no rows for {}", symbol);
        }
        let items: Vec<(String, f64)> = ak_candles
            .into_iter()
            .map(|c| (c.trade_date, c.close))
            .collect();
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

    /// Try to resolve US stock industry.
    ///
    /// Uses Yahoo Finance assetProfile API with static fallback for major stocks.
    /// Returns "sector / industry" format, or just industry if sector is unavailable.
    async fn resolve_us_industry(&self, symbol: &str) -> Option<String> {
        let (sector, industry) = self.ak.us_stock_industry(symbol).await;
        match (sector, industry) {
            (Some(s), Some(i)) => Some(format!("{s} / {i}")),
            (Some(s), None) => Some(s),
            (None, Some(i)) => Some(i),
            (None, None) => None,
        }
    }
}
