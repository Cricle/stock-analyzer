use anyhow::{Context, bail};
use chrono::NaiveDate;

use super::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::types::{NewsFetchAttempt, NewsFetchResult};
use super::news::within_date_window;

impl MarketDataClient {
    pub(super) async fn fetch_us_fundamentals(
        &self,
        symbol: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let t0 = std::time::Instant::now();

        // Fetch typed APIs
        let indicators = self
            .ak
            .stock_financial_us_analysis_indicator_em_typed(symbol, "年报")
            .await
            .unwrap_or_default();
        let main = indicators.first();

        let balance_sheets = self
            .ak
            .stock_financial_us_balance_sheet_typed(symbol, "年报")
            .await
            .unwrap_or_default();
        let bs = balance_sheets.first();

        let income_sheets = self
            .ak
            .stock_financial_us_income_sheet_typed(symbol, "年报")
            .await
            .unwrap_or_default();
        let is = income_sheets.first();

        let cashflow_sheets = self
            .ak
            .stock_financial_us_cashflow_sheet_typed(symbol, "年报")
            .await
            .unwrap_or_default();
        let cf = cashflow_sheets.first();

        tracing::debug!(
            symbol,
            indicators = indicators.len(),
            balance_sheets = balance_sheets.len(),
            income_sheets = income_sheets.len(),
            "fetch_us_fundamentals: akshare reports took {}ms",
            t0.elapsed().as_millis()
        );

        // Extract company name
        let company_name = main
            .and_then(|m| m.report_date.as_ref())
            .map(|_| symbol) // indicators don't have company name; use symbol
            .unwrap_or(symbol)
            .to_string();

        // From main indicator (may be empty for some US stocks)
        let net_income = main
            .and_then(|m| m.holder_profit.or(m.parent_net_profit))
            .or_else(|| is.and_then(|s| s.net_profit));
        let revenue = main
            .and_then(|m| m.operate_income.or(m.total_operate_reve))
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
    /// Tries Yahoo Finance key stats first, falls back to computing PE from fundamentals.
    pub(crate) async fn fetch_us_enrichment(
        &self,
        symbol: &str,
    ) -> anyhow::Result<super::a_share::AShareEnrichmentData> {
        // Try Yahoo Finance first
        if let Ok(stats) = self.ak.us_stock_key_stats(symbol).await {
            return Ok(super::a_share::AShareEnrichmentData {
                pe_ttm: stats.trailing_pe,
                pb: stats.price_to_book,
                gross_margin: stats.gross_margin,
                dividend_yield: stats.dividend_yield,
                ..super::a_share::AShareEnrichmentData::default()
            });
        }
        // Fallback: compute PE from fundamentals (market_cap / net_income)
        let fundamentals = self.fetch_us_fundamentals(symbol).await.ok();
        let pe_ttm = fundamentals.as_ref().and_then(|f| {
            let mc = f.market_cap?;
            let ni = f.net_income_usd?;
            if ni > 0.0 { Some(mc / ni) } else { None }
        });
        tracing::debug!(symbol, ?pe_ttm, "us_enrichment fallback from fundamentals");
        Ok(super::a_share::AShareEnrichmentData {
            pe_ttm,
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
        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();

        // Primary source: Bing RSS
        // Filter out portal pages and non-news results
        let portal_domains = [
            "baike.baidu.com", "iciba.com", "eastmoney.com",
            "sina.com.cn/stock/", "finance.qq.com", "investing.com/equities",
            "github.com", "stock.sina.com.cn",
        ];
        let queries = [
            format!("{} stock news today", symbol),
            format!("{} earnings report", symbol),
        ];
        let mut bing_added = 0usize;
        for query in &queries {
            if let Ok(rss_items) = self.ak.bing_news_rss_with_lang(query, 10, Some("en")).await {
                for item in rss_items.into_iter() {
                    if !within_date_window(&item.published_at, start_date, end_date) {
                        continue;
                    }
                    // Filter out portal pages and non-news
                    let url = item.url.as_deref().unwrap_or("");
                    let is_portal = portal_domains.iter().any(|d| url.contains(d));
                    let is_dictionary = item.title.contains("是什么意思")
                        || item.title.contains("翻译")
                        || item.title.contains("的用法");
                    if is_portal || is_dictionary {
                        continue;
                    }
                    bing_added += 1;
                    items.push(item);
                }
            }
        }
        attempts.push(NewsFetchAttempt {
            source: "bing_rss".to_string(),
            query: Some(format!("{} stock news/earnings", symbol)),
            success: bing_added > 0,
            item_count: bing_added,
            error: if bing_added == 0 { Some("no usable Bing RSS results".to_string()) } else { None },
        });

        // Secondary: Google News RSS
        if items.len() < limit.min(6) {
            let existing_titles: std::collections::HashSet<String> =
                items.iter().map(|i| i.title.to_lowercase()).collect();
            let google_query = format!("{} stock", symbol);
            match self.ak.google_news_rss(&google_query, 15).await {
                Ok(google_items) => {
                    let count = google_items.len();
                    let filtered: Vec<NewsItem> = google_items
                        .into_iter()
                        .filter(|item| {
                            within_date_window(&item.published_at, start_date, end_date)
                                && !existing_titles.contains(&item.title.to_lowercase())
                                && !portal_domains.iter().any(|d| {
                                    item.url.as_deref().unwrap_or("").contains(d)
                                })
                        })
                        .collect();
                    let added = filtered.len();
                    items.extend(filtered);
                    attempts.push(NewsFetchAttempt {
                        source: "google_news_rss".to_string(),
                        query: Some(google_query),
                        success: added > 0,
                        item_count: count,
                        error: if added == 0 { Some("no usable Google News results".to_string()) } else { None },
                    });
                }
                Err(error) => {
                    attempts.push(NewsFetchAttempt {
                        source: "google_news_rss".to_string(),
                        query: Some(google_query),
                        success: false,
                        item_count: 0,
                        error: Some(error.to_string()),
                    });
                }
            }
        }

        // Fallback: Eastmoney US stock news
        if items.len() < limit.min(4) {
            match self.ak.stock_news_em_us(symbol).await {
                Ok(ak_news) => {
                    let count = ak_news.len();
                    let existing_titles: std::collections::HashSet<String> =
                        items.iter().map(|i| i.title.to_lowercase()).collect();
                    let converted: Vec<NewsItem> = ak_news
                        .into_iter()
                        .map(super::news_item_from_stock_news)
                        .filter(|item| {
                            within_date_window(&item.published_at, start_date, end_date)
                                && !existing_titles.contains(&item.title.to_lowercase())
                                && !portal_domains.iter().any(|d| {
                                    item.url.as_deref().unwrap_or("").contains(d)
                                })
                        })
                        .collect();
                    attempts.push(NewsFetchAttempt {
                        source: "eastmoney_us".to_string(),
                        query: Some(symbol.to_string()),
                        success: true,
                        item_count: count,
                        error: None,
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
        let end = NaiveDate::parse_from_str(curr_date, "%Y-%m-%d")
            .context("invalid curr_date for global news")?;
        let start = end - chrono::Days::new(look_back_days as u64);
        let start_text = start.to_string();

        let queries = [
            "stock market economy",
            "Federal Reserve interest rates",
            "inflation economic outlook",
            "global markets trading",
            "AI semiconductor megacap market outlook",
        ];

        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();
        let existing_titles: std::collections::HashSet<String> =
            items.iter().map(|i| i.title.to_lowercase()).collect();

        for query in &queries {
            if let Ok(rss_items) = self.ak.bing_news_rss(query, 10).await {
                let count = rss_items.len();
                for item in rss_items.into_iter() {
                    if within_date_window(&item.published_at, Some(&start_text), Some(curr_date))
                        && !existing_titles.contains(&item.title.to_lowercase())
                    {
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
        let ak_quote = self.ak.us_quote(symbol).await?;
        Ok((ak_quote, "akshare".to_string()))
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
