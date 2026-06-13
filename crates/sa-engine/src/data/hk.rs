use super::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use super::news_item_from_stock_news;
use anyhow::Context;

impl MarketDataClient {
    pub(super) fn hk_standard_code(&self, symbol: &str) -> anyhow::Result<String> {
        let normalized = self
            .normalize_hk_symbol(symbol)
            .context("invalid HK symbol")?;
        Ok(normalized.trim_end_matches(".HK").to_string())
    }

    fn hk_json_f64(row: &std::collections::HashMap<String, serde_json::Value>, key: &str) -> Option<f64> {
        row.get(key).and_then(|v| v.as_f64())
    }

    fn hk_json_string(row: &std::collections::HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
        row.get(key).and_then(|v| v.as_str().map(|s| s.to_string()))
    }

    pub(super) async fn fetch_hk_fundamentals(
        &self,
        symbol: &str,
    ) -> anyhow::Result<FundamentalsSnapshot> {
        let search_code = self.hk_standard_code(symbol)?;

        // Fetch main financial indicators via akshare-rs
        let indicators = self
            .ak
            .stock_financial_hk_analysis_indicator_em(&search_code, "报告期")
            .await
            .unwrap_or_default();
        let main = indicators.first();

        // Fetch balance sheet for cash_and_equivalents and long_term_debt
        let balance_items = self
            .ak
            .stock_financial_hk_report_em(&search_code, "资产负债表", "报告期")
            .await
            .unwrap_or_default();

        // Fetch income statement for operating_expenses breakdown
        let income_items = self
            .ak
            .stock_financial_hk_report_em(&search_code, "利润表", "报告期")
            .await
            .unwrap_or_default();

        // Fetch cashflow for detailed operating_cash_flow and capital_expenditure
        let cashflow_items = self
            .ak
            .stock_financial_hk_report_em(&search_code, "现金流量表", "报告期")
            .await
            .unwrap_or_default();

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

        // Extract values from main indicator
        let currency = main
            .and_then(|m| Self::hk_json_string(m, "CURRENCY"))
            .unwrap_or_else(|| "HKD".to_string());
        let fiscal_year_end = main.and_then(|m| {
            Self::hk_json_string(m, "REPORT_DATE")
                .or_else(|| Self::hk_json_string(m, "STD_REPORT_DATE"))
        });
        let operate_income = main.and_then(|m| Self::hk_json_f64(m, "OPERATE_INCOME"));
        let holder_profit = main.and_then(|m| Self::hk_json_f64(m, "HOLDER_PROFIT"));
        let gross_profit = main.and_then(|m| Self::hk_json_f64(m, "GROSS_PROFIT"));
        let total_assets = main.and_then(|m| Self::hk_json_f64(m, "TOTAL_ASSETS"));
        let total_liabilities = main.and_then(|m| Self::hk_json_f64(m, "TOTAL_LIABILITIES"));
        let total_parent_equity = main.and_then(|m| Self::hk_json_f64(m, "TOTAL_PARENT_EQUITY"));
        let netcash_operate = main.and_then(|m| Self::hk_json_f64(m, "NETCASH_OPERATE"));
        let capital_expenditure = main.and_then(|m| Self::hk_json_f64(m, "CAPITAL_EXPENDITURE"));
        let total_share = main.and_then(|m| Self::hk_json_f64(m, "TOTAL_SHARE"));
        let current_liability = main.and_then(|m| Self::hk_json_f64(m, "CURRENT_LIABILITY"));
        let noncurrent_liab_1year = main.and_then(|m| Self::hk_json_f64(m, "NONCURRENT_LIAB_1YEAR"));

        // Extract from detailed balance sheet: cash_and_equivalents, long_term_debt
        let (cash_and_equivalents, long_term_debt) = Self::extract_hk_balance_items(&balance_items);
        let operating_income = main.and_then(|m| Self::hk_json_f64(m, "OPERATE_INCOME"));

        // Extract operating_expenses from income items
        let operating_expenses =
            Self::extract_hk_operating_expenses(&income_items, gross_profit, operating_income);

        // Extract cashflow details if main indicator lacks them
        let netcash_operate = netcash_operate.or_else(|| {
            Self::extract_hk_cashflow_item(
                &cashflow_items,
                &["经营活动产生的现金流量净额", "经营业务现金流量净额"],
            )
        });
        let capital_expenditure_detail = capital_expenditure.or_else(|| {
            Self::extract_hk_cashflow_item(
                &cashflow_items,
                &[
                    "购建固定资产、无形资产和其他长期资产支付的现金",
                    "购买固定资产、无形资产及其他长期资产的款项",
                ],
            )
        });

        let short_term_debt = balance_items.iter().find_map(|row| {
            let name = Self::hk_json_string(row, "STD_ITEM_NAME")?;
            if name == "短期贷款" {
                Self::hk_json_f64(row, "AMOUNT")
            } else {
                None
            }
        });

        let noncurrent_liabilities = balance_items.iter().find_map(|row| {
            let name = Self::hk_json_string(row, "STD_ITEM_NAME")?;
            if name == "非流动负债合计" {
                Self::hk_json_f64(row, "AMOUNT")
            } else {
                None
            }
        });

        let shares_outstanding = total_share.map(|value| (value * 10_000.0).round() as i64);

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

        let snapshot = FundamentalsSnapshot {
            symbol: symbol.to_uppercase(),
            company_name,
            cik: String::new(),
            industry: None,
            currency,
            fiscal_year_end,
            shares_outstanding,
            market_cap: None,
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

    /// Extract cash_and_equivalents and long_term_debt from balance sheet items.
    fn extract_hk_balance_items(
        items: &[std::collections::HashMap<String, serde_json::Value>],
    ) -> (Option<f64>, Option<f64>) {
        let mut cash = None;
        let mut ltd = None;
        for row in items {
            let name = match Self::hk_json_string(row, "STD_ITEM_NAME") {
                Some(n) => n,
                None => continue,
            };
            let amount = Self::hk_json_f64(row, "AMOUNT");
            if (name == "现金及等价物" || name == "现金及现金等价物")
                && cash.is_none() {
                    cash = amount;
                }
            if name == "长期贷款"
                && ltd.is_none() {
                    ltd = amount;
                }
        }
        (cash, ltd)
    }

    /// Extract operating_expenses from income items, with cross-validation.
    fn extract_hk_operating_expenses(
        items: &[std::collections::HashMap<String, serde_json::Value>],
        gross_profit: Option<f64>,
        operating_income: Option<f64>,
    ) -> Option<f64> {
        let mut sales = None;
        let mut rnd = None;
        let mut admin = None;
        for row in items {
            let name = match Self::hk_json_string(row, "STD_ITEM_NAME") {
                Some(n) => n,
                None => continue,
            };
            let amount = Self::hk_json_f64(row, "AMOUNT");
            if name == "销售及分销费用" && sales.is_none() {
                sales = amount;
            }
            if name == "研发费用" && rnd.is_none() {
                rnd = amount;
            }
            if (name == "管理费用" || name == "行政费用") && admin.is_none() {
                admin = amount;
            }
        }
        let direct_sum = match (sales, rnd, admin) {
            (Some(s), Some(r), Some(a)) => Some(s + r + a),
            (Some(s), Some(r), None) => Some(s + r),
            (Some(s), None, Some(a)) => Some(s + a),
            (None, Some(r), Some(a)) => Some(r + a),
            (Some(s), None, None) => Some(s),
            (None, Some(r), None) => Some(r),
            (None, None, Some(a)) => Some(a),
            (None, None, None) => None,
        };
        // Cross-validate with gross_profit and operating_income
        if let (Some(gp), Some(oi)) = (gross_profit, operating_income) {
            let derived = gp - oi;
            if derived > 0.0 {
                return Some(derived);
            }
        }
        direct_sum
    }

    /// Extract a cashflow item by matching STD_ITEM_NAME.
    fn extract_hk_cashflow_item(
        items: &[std::collections::HashMap<String, serde_json::Value>],
        names: &[&str],
    ) -> Option<f64> {
        for row in items {
            let name = match Self::hk_json_string(row, "STD_ITEM_NAME") {
                Some(n) => n,
                None => continue,
            };
            if names.contains(&name.as_str())
                && let Some(amount) = Self::hk_json_f64(row, "AMOUNT") {
                    return Some(amount);
                }
        }
        None
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
}
