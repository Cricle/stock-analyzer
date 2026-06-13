use anyhow::{Context, bail};
use chrono::NaiveDate;
use std::collections::HashMap;

use super::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::types::{NewsFetchAttempt, NewsFetchResult};
use super::news_filter::within_date_window;

impl MarketDataClient {
    pub(super) async fn fetch_us_fundamentals(
        &self,
        symbol: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let t0 = std::time::Instant::now();

        // Fetch financial analysis indicators (annual)
        let analysis: Vec<HashMap<String, serde_json::Value>> = self
            .ak
            .stock_financial_us_analysis_indicator_em(symbol, "年报")
            .await
            .unwrap_or_default();

        // Fetch income statement (annual)
        let income: Vec<HashMap<String, serde_json::Value>> = self
            .ak
            .stock_financial_us_report_em(symbol, "综合损益表", "年报")
            .await
            .unwrap_or_default();

        // Fetch balance sheet (annual)
        let balance: Vec<HashMap<String, serde_json::Value>> = self
            .ak
            .stock_financial_us_report_em(symbol, "资产负债表", "年报")
            .await
            .unwrap_or_default();

        // Fetch cash flow statement (annual)
        let cashflow: Vec<HashMap<String, serde_json::Value>> = self
            .ak
            .stock_financial_us_report_em(symbol, "现金流量表", "年报")
            .await
            .unwrap_or_default();

        tracing::debug!(
            "fetch_us_fundamentals: akshare reports for {symbol} took {}ms",
            t0.elapsed().as_millis()
        );

        // Extract company name from analysis data
        let company_name = analysis
            .first()
            .and_then(|m| m.get("SECURITY_NAME_ABBR"))
            .and_then(|v| v.as_str())
            .unwrap_or(symbol)
            .to_string();

        // Extract key metrics from analysis indicators
        let (net_income, revenue, shares_outstanding, diluted_shares) =
            if let Some(first) = analysis.first() {
                (
                    first
                        .get("PARENT_HOLDER_NETPROFIT")
                        .and_then(serde_json::Value::as_f64),
                    first
                        .get("TOTAL_INCOME")
                        .and_then(serde_json::Value::as_f64),
                    first
                        .get("TOTAL_SHARES")
                        .and_then(serde_json::Value::as_f64)
                        .map(|v| v as i64),
                    first
                        .get("DILUTED_SHARES")
                        .and_then(serde_json::Value::as_f64)
                        .map(|v| v as i64),
                )
            } else {
                (None, None, None, None)
            };

        // Helper to find an amount by ITEM_NAME in report data
        let find_amount =
            |data: &[HashMap<String, serde_json::Value>], names: &[&str]| -> Option<f64> {
                for name in names {
                    for row in data {
                        if let Some(item_name) = row.get("ITEM_NAME").and_then(|v| v.as_str())
                            && item_name.contains(name) {
                                return row.get("AMOUNT").and_then(serde_json::Value::as_f64);
                            }
                    }
                }
                None
            };

        // Extract balance sheet items
        let assets = find_amount(&balance, &["资产总计", "总资产"]);
        let liabilities = find_amount(&balance, &["负债合计", "总负债"]);
        let stockholders_equity = find_amount(
            &balance,
            &["所有者权益合计", "股东权益合计", "归属母公司股东权益"],
        );
        let cash = find_amount(&balance, &["货币资金"]);
        let long_term_debt = find_amount(&balance, &["长期借款"]);
        let current_debt = find_amount(&balance, &["短期借款", "一年内到期的非流动负债"]);
        let total_debt = find_amount(&balance, &["负债合计"]);

        // Extract income statement items
        let gross_profit = find_amount(&income, &["毛利", "营业毛利"]);
        let operating_income = find_amount(&income, &["营业利润"]);
        let operating_expenses = find_amount(&income, &["营业总成本", "营业成本"]);

        // Extract cash flow items
        let operating_cash_flow =
            find_amount(&cashflow, &["经营活动产生的现金流量净额"]);
        let capital_expenditure = find_amount(
            &cashflow,
            &["购建固定资产、无形资产和其他长期资产支付的现金"],
        );

        let free_cash_flow = match (operating_cash_flow, capital_expenditure) {
            (Some(ocf), Some(capex)) => Some(ocf - capex),
            _ => None,
        };

        // Get quote for market cap calculation
        let quote = self.fetch_quote(symbol).await.ok();
        let market_cap = quote.as_ref().and_then(|q| {
            shares_outstanding.map(|shares| q.close * shares as f64)
        });

        Ok(FundamentalsSnapshot {
            symbol: symbol.to_uppercase(),
            company_name,
            cik: String::new(),
            industry: None,
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

    pub(super) async fn fetch_us_news(
        &self,
        symbol: &str,
        limit: usize,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> anyhow::Result<NewsFetchResult> {
        let mut attempts = Vec::new();
        let mut items: Vec<NewsItem> = Vec::new();

        // Primary source: Eastmoney US stock news
        match self.ak.stock_news_em_us(symbol).await {
            Ok(ak_news) => {
                let count = ak_news.len();
                let converted: Vec<NewsItem> = ak_news
                    .into_iter()
                    .map(super::news_item_from_stock_news)
                    .filter(|item| within_date_window(&item.published_at, start_date, end_date))
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

        // Fallback: Bing RSS news search
        if items.len() < limit.min(4) {
            let queries = [
                format!("{} stock news", symbol),
                format!("{} earnings", symbol),
            ];
            let existing_titles: std::collections::HashSet<String> =
                items.iter().map(|i| i.title.to_lowercase()).collect();
            let mut bing_added = 0usize;
            for query in &queries {
                if let Ok(rss_items) = self.ak.bing_news_rss(query, 10).await {
                    for item in rss_items.into_iter() {
                        if within_date_window(&item.published_at, start_date, end_date)
                            && !existing_titles.contains(&item.title.to_lowercase())
                        {
                            bing_added += 1;
                            items.push(item);
                        }
                    }
                }
            }
            if bing_added > 0 {
                attempts.push(NewsFetchAttempt {
                    source: "bing_rss".to_string(),
                    query: None,
                    success: true,
                    item_count: bing_added,
                    error: None,
                });
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
}
