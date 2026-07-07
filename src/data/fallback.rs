//! SEC data fallback — Yahoo Finance + Finnhub
//!
//! When US equity fundamentals fail (SEC 403), this module provides
//! fallback data sources in order: Yahoo Finance, then Finnhub.

use serde::Deserialize;

use super::FundamentalsSnapshot;

/// Client for fetching fundamentals from fallback sources.
pub struct FallbackFundamentalsClient {
    http: reqwest::Client,
    finnhub_api_keys: Vec<String>,
}

impl FallbackFundamentalsClient {
    /// Create a new fallback client with 15s timeout.
    /// `finnhub_api_keys` is a pool of API keys for rotation.
    pub fn new(finnhub_api_keys: Vec<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build reqwest client");
        Self {
            http,
            finnhub_api_keys,
        }
    }

    /// Try Yahoo Finance, then merge Finnhub financials-reported + metrics + profile.
    /// Returns None if all fail.
    pub async fn fetch(&self, symbol: &str) -> Option<FundamentalsSnapshot> {
        if let Some(result) = self.fetch_yahoo(symbol).await {
            tracing::info!(symbol, "fallback: Yahoo Finance succeeded");
            return Some(result);
        }
        // Try all Finnhub endpoints concurrently and merge results
        let (financials, metrics, profile) = tokio::join!(
            self.fetch_finnhub_financials(symbol),
            self.fetch_finnhub(symbol),
            self.fetch_finnhub_profile(symbol),
        );
        let mut result = match (financials, metrics) {
            (Some(mut fin), Some(met)) => {
                // Merge: financials has absolute values, metrics has market_cap
                if fin.market_cap.is_none() { fin.market_cap = met.market_cap; }
                tracing::info!(symbol, "fallback: Finnhub merged financials + metrics");
                fin
            }
            (Some(fin), None) => {
                tracing::info!(symbol, "fallback: Finnhub financials-reported succeeded");
                fin
            }
            (None, Some(met)) => {
                tracing::info!(symbol, "fallback: Finnhub metrics succeeded");
                met
            }
            (None, None) => {
                tracing::warn!(symbol, "fallback: all Finnhub sources failed");
                return None;
            }
        };
        // Merge profile data (company_name, industry, shares_outstanding)
        if let Some(prof) = profile {
            if result.company_name.is_empty() || result.company_name == result.cik {
                result.company_name = prof.name;
            }
            if result.industry.is_none() {
                result.industry = Some(prof.industry);
            }
            if result.shares_outstanding.is_none() {
                result.shares_outstanding = prof.shares_outstanding;
            }
            tracing::info!(symbol, "fallback: merged Finnhub profile data");
        }
        Some(result)
    }

    /// Fetch from Yahoo Finance quoteSummary API.
    async fn fetch_yahoo(&self, symbol: &str) -> Option<FundamentalsSnapshot> {
        let url = format!(
            "https://query1.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=financialData,defaultKeyStatistics",
            symbol
        );
        let resp = self.http.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            tracing::debug!(symbol, status = %resp.status(), "Yahoo Finance request failed");
            return None;
        }
        let body: YahooQuoteSummaryResponse = resp.json().await.ok()?;
        let result = body.quote_summary.result.first()?;
        let financial = &result.financial_data;
        let stats = &result.default_key_statistics;

        Some(FundamentalsSnapshot {
            symbol: symbol.to_string(),
            company_name: String::new(),
            cik: String::new(),
            industry: None,
            currency: financial
                .financial_currency
                .clone()
                .unwrap_or_else(|| "USD".to_string()),
            fiscal_year_end: None,
            shares_outstanding: stats.shares_outstanding.map(|v| v as i64),
            market_cap: financial.market_cap.as_ref().and_then(|v| v.raw),
            net_income_usd: financial.net_income_to_common.as_ref().and_then(|v| v.raw),
            revenues_usd: financial.total_revenue.as_ref().and_then(|v| v.raw),
            assets_usd: financial.total_assets.as_ref().and_then(|v| v.raw),
            liabilities_usd: financial.total_liabilities.as_ref().and_then(|v| v.raw),
            stockholders_equity_usd: financial.total_stockholder_equity.as_ref().and_then(|v| v.raw),
            cash_and_equivalents_usd: None,
            gross_profit_usd: financial.gross_profit.as_ref().and_then(|v| v.raw),
            operating_income_usd: financial.operating_income.as_ref().and_then(|v| v.raw),
            operating_expenses_usd: None,
            operating_cash_flow_usd: financial.operating_cashflow.as_ref().and_then(|v| v.raw),
            capital_expenditure_usd: None,
            free_cash_flow_usd: financial.free_cashflow.as_ref().and_then(|v| v.raw),
            long_term_debt_usd: None,
            current_debt_usd: None,
            total_debt_usd: None,
            diluted_shares_outstanding: None,
        })
    }

    /// Fetch from Finnhub /stock/financials-reported API, rotating through API keys.
    /// Returns SEC-reported financial data with absolute values.
    async fn fetch_finnhub_financials(&self, symbol: &str) -> Option<FundamentalsSnapshot> {
        if self.finnhub_api_keys.is_empty() {
            return None;
        }
        for api_key in &self.finnhub_api_keys {
            let url = format!(
                "https://finnhub.io/api/v1/stock/financials-reported?symbol={}&token={}",
                symbol, api_key
            );
            let resp = match self.http.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(symbol, error = %e, "Finnhub financials-reported request error");
                    continue;
                }
            };
            let status = resp.status();
            if status.as_u16() == 429 || status.as_u16() == 401 {
                tracing::debug!(symbol, status = %status, key_prefix = &api_key[..8], "Finnhub financials-reported rate limited or unauthorized, trying next key");
                continue;
            }
            if !status.is_success() {
                tracing::debug!(symbol, status = %status, "Finnhub financials-reported request failed");
                return None;
            }
            let body: FinnhubFinancialsReportedResponse = resp.json().await.ok()?;
            let report = body.data.first()?.report.as_ref()?;

            // Extract balance sheet fields
            let assets_usd = find_report_value(&report.bs, "us-gaap_Assets");
            let liabilities_usd = find_report_value(&report.bs, "us-gaap_Liabilities");
            let stockholders_equity_usd = find_report_value(&report.bs, "us-gaap_StockholdersEquity");
            let cash_and_equivalents_usd = find_report_value(&report.bs, "us-gaap_CashAndCashEquivalentsAtCarryingValue");
            let long_term_debt_usd = find_report_value(&report.bs, "us-gaap_LongTermDebtNoncurrent");
            let current_debt_usd = find_report_value(&report.bs, "us-gaap_LongTermDebtCurrent");
            let total_debt_usd = match (long_term_debt_usd, current_debt_usd) {
                (Some(lt), Some(ct)) => Some(lt + ct),
                (Some(lt), None) => Some(lt),
                (None, Some(ct)) => Some(ct),
                _ => None,
            };

            // Extract cash flow fields
            let operating_cash_flow_usd = find_report_value(&report.cf, "us-gaap_NetCashProvidedByUsedInOperatingActivities");
            let capital_expenditure_usd = find_report_value(&report.cf, "us-gaap_PaymentsToAcquirePropertyPlantAndEquipment");
            let free_cash_flow_usd = match (operating_cash_flow_usd, capital_expenditure_usd) {
                (Some(ocf), Some(capex)) => Some(ocf - capex),
                _ => None,
            };

            // Extract income statement fields
            let revenues_usd = find_report_value(&report.ic, "us-gaap_RevenueFromContractWithCustomerExcludingAssessedTax");
            let gross_profit_usd = find_report_value(&report.ic, "us-gaap_GrossProfit");
            let operating_income_usd = find_report_value(&report.ic, "us-gaap_OperatingIncomeLoss");
            let net_income_usd = find_report_value(&report.ic, "us-gaap_NetIncomeLoss");

            tracing::info!(
                symbol,
                assets = ?assets_usd,
                liabilities = ?liabilities_usd,
                equity = ?stockholders_equity_usd,
                cash = ?cash_and_equivalents_usd,
                ocf = ?operating_cash_flow_usd,
                capex = ?capital_expenditure_usd,
                revenue = ?revenues_usd,
                "Finnhub financials-reported extracted"
            );

            return Some(FundamentalsSnapshot {
                symbol: symbol.to_string(),
                company_name: body.cik.clone().unwrap_or_default(),
                cik: body.cik.clone().unwrap_or_default(),
                industry: None,
                currency: "USD".to_string(),
                fiscal_year_end: None,
                shares_outstanding: None,
                market_cap: None,
                net_income_usd,
                revenues_usd,
                assets_usd,
                liabilities_usd,
                stockholders_equity_usd,
                cash_and_equivalents_usd,
                gross_profit_usd,
                operating_income_usd,
                operating_expenses_usd: None,
                operating_cash_flow_usd,
                capital_expenditure_usd,
                free_cash_flow_usd,
                long_term_debt_usd,
                current_debt_usd,
                total_debt_usd,
                diluted_shares_outstanding: None,
            });
        }
        None
    }

    /// Fetch from Finnhub stock/metric API, rotating through API keys.
    /// Extracts as many FundamentalsSnapshot fields as possible from metrics.
    async fn fetch_finnhub(&self, symbol: &str) -> Option<FundamentalsSnapshot> {
        if self.finnhub_api_keys.is_empty() {
            return None;
        }
        // Try each key in order; on 429 (rate limit) or 401, try next key
        for api_key in &self.finnhub_api_keys {
            let url = format!(
                "https://finnhub.io/api/v1/stock/metric?symbol={}&metric=all&token={}",
                symbol, api_key
            );
            let resp = match self.http.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(symbol, error = %e, "Finnhub request error");
                    continue;
                }
            };
            let status = resp.status();
            if status.as_u16() == 429 || status.as_u16() == 401 {
                tracing::debug!(symbol, status = %status, key_prefix = &api_key[..8], "Finnhub rate limited or unauthorized, trying next key");
                continue;
            }
            if !status.is_success() {
                tracing::debug!(symbol, status = %status, "Finnhub request failed");
                return None;
            }
            let body: FinnhubMetricResponse = resp.json().await.ok()?;
            let m = &body.metric;

            // Derive fields from Finnhub metrics:
            // market_cap is in millions
            let market_cap = m.market_cap; // already in millions
            // Derive shares_outstanding from market_cap / price (not available here)
            // Use peTTM and market_cap to derive net_income: net_income = market_cap / peTTM
            let net_income_usd = match (market_cap, m.pe_ttm) {
                (Some(mc), Some(pe)) if pe > 0.0 => Some(mc / pe),
                _ => None,
            };
            // Derive revenues from market_cap / psTTM
            let revenues_usd = match (market_cap, m.ps_ttm) {
                (Some(mc), Some(ps)) if ps > 0.0 => Some(mc / ps),
                _ => None,
            };
            // Derive gross_profit from revenues * gross_margin
            let gross_profit_usd = match (revenues_usd, m.gross_margin_ttm) {
                (Some(rev), Some(margin)) => Some(rev * margin / 100.0),
                _ => None,
            };
            // Derive operating_income from revenues * operating_margin
            let operating_income_usd = match (revenues_usd, m.operating_margin_ttm) {
                (Some(rev), Some(margin)) => Some(rev * margin / 100.0),
                _ => None,
            };
            // Derive stockholders_equity from book_value_per_share * shares
            // We don't have shares, but we can use pb ratio: equity = market_cap / pb
            let stockholders_equity_usd = match (market_cap, m.pb) {
                (Some(mc), Some(pb)) if pb > 0.0 => Some(mc / pb),
                _ => None,
            };
            // Derive total_debt from debt/equity ratio * equity
            let total_debt_usd = match (stockholders_equity_usd, m.total_debt_total_equity_quarterly) {
                (Some(eq), Some(ratio)) => Some(eq * ratio),
                _ => None,
            };

            tracing::info!(
                symbol,
                market_cap = ?market_cap,
                net_income = ?net_income_usd,
                revenues = ?revenues_usd,
                "Finnhub metrics derived"
            );

            return Some(FundamentalsSnapshot {
                symbol: symbol.to_string(),
                company_name: String::new(),
                cik: String::new(),
                industry: None,
                currency: "USD".to_string(),
                fiscal_year_end: None,
                shares_outstanding: None,
                market_cap,
                net_income_usd,
                revenues_usd,
                assets_usd: None,
                liabilities_usd: None,
                stockholders_equity_usd,
                cash_and_equivalents_usd: None,
                gross_profit_usd,
                operating_income_usd,
                operating_expenses_usd: None,
                operating_cash_flow_usd: None,
                capital_expenditure_usd: None,
                free_cash_flow_usd: None,
                long_term_debt_usd: None,
                current_debt_usd: None,
                total_debt_usd,
                diluted_shares_outstanding: None,
            });
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Yahoo Finance response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct YahooQuoteSummaryResponse {
    #[serde(rename = "quoteSummary")]
    quote_summary: YahooQuoteSummary,
}

#[derive(Debug, Deserialize)]
struct YahooQuoteSummary {
    result: Vec<YahooQuoteSummaryResult>,
}

#[derive(Debug, Deserialize)]
struct YahooQuoteSummaryResult {
    #[serde(rename = "financialData")]
    financial_data: YahooFinancialData,
    #[serde(rename = "defaultKeyStatistics")]
    default_key_statistics: YahooDefaultKeyStatistics,
}

#[derive(Debug, Deserialize)]
struct YahooFinancialData {
    #[serde(rename = "financialCurrency")]
    financial_currency: Option<String>,
    #[serde(rename = "marketCap")]
    market_cap: Option<YahooRawValue>,
    #[serde(rename = "netIncomeToCommon")]
    net_income_to_common: Option<YahooRawValue>,
    #[serde(rename = "totalRevenue")]
    total_revenue: Option<YahooRawValue>,
    #[serde(rename = "totalAssets")]
    total_assets: Option<YahooRawValue>,
    #[serde(rename = "totalLiabilities")]
    total_liabilities: Option<YahooRawValue>,
    #[serde(rename = "totalStockholderEquity")]
    total_stockholder_equity: Option<YahooRawValue>,
    #[serde(rename = "grossProfit")]
    gross_profit: Option<YahooRawValue>,
    #[serde(rename = "operatingIncome")]
    operating_income: Option<YahooRawValue>,
    #[serde(rename = "operatingCashflow")]
    operating_cashflow: Option<YahooRawValue>,
    #[serde(rename = "freeCashflow")]
    free_cashflow: Option<YahooRawValue>,
}

#[derive(Debug, Deserialize)]
struct YahooDefaultKeyStatistics {
    #[serde(rename = "sharesOutstanding")]
    shares_outstanding: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct YahooRawValue {
    raw: Option<f64>,
}

// ---------------------------------------------------------------------------
// Finnhub response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FinnhubMetricResponse {
    metric: FinnhubMetric,
}

#[derive(Debug, Deserialize)]
struct FinnhubMetric {
    #[serde(rename = "marketCapitalization")]
    market_cap: Option<f64>,
    #[serde(rename = "peTTM")]
    pe_ttm: Option<f64>,
    #[serde(rename = "psTTM")]
    ps_ttm: Option<f64>,
    #[serde(rename = "grossMarginTTM")]
    gross_margin_ttm: Option<f64>,
    #[serde(rename = "operatingMarginTTM")]
    operating_margin_ttm: Option<f64>,
    #[serde(rename = "pb")]
    pb: Option<f64>,
    #[serde(rename = "totalDebt/totalEquityQuarterly")]
    total_debt_total_equity_quarterly: Option<f64>,
}

// ---------------------------------------------------------------------------
// Finnhub financials-reported response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FinnhubFinancialsReportedResponse {
    cik: Option<String>,
    data: Vec<FinnhubFinancialsReportedData>,
}

#[derive(Debug, Deserialize)]
struct FinnhubFinancialsReportedData {
    report: Option<FinnhubFinancialsReport>,
}

#[derive(Debug, Deserialize)]
struct FinnhubFinancialsReport {
    bs: Option<Vec<FinnhubReportEntry>>,
    cf: Option<Vec<FinnhubReportEntry>>,
    ic: Option<Vec<FinnhubReportEntry>>,
}

#[derive(Debug, Deserialize)]
struct FinnhubReportEntry {
    concept: String,
    value: Option<f64>,
}

/// Find a value in a report section by concept name.
fn find_report_value(entries: &Option<Vec<FinnhubReportEntry>>, concept: &str) -> Option<f64> {
    entries.as_ref()?.iter().find(|e| e.concept == concept)?.value
}

// ---------------------------------------------------------------------------
// Finnhub profile response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FinnhubProfile {
    name: Option<String>,
    #[serde(rename = "finnhubIndustry")]
    industry: Option<String>,
    #[serde(rename = "shareOutstanding")]
    share_outstanding: Option<f64>,
}

/// Lightweight struct for profile merge data.
struct ProfileData {
    name: String,
    industry: String,
    shares_outstanding: Option<i64>,
}

impl FallbackFundamentalsClient {
    /// Fetch company profile from Finnhub /stock/profile2 API.
    /// Returns company_name, industry, and shares_outstanding.
    async fn fetch_finnhub_profile(&self, symbol: &str) -> Option<ProfileData> {
        if self.finnhub_api_keys.is_empty() {
            return None;
        }
        for api_key in &self.finnhub_api_keys {
            let url = format!(
                "https://finnhub.io/api/v1/stock/profile2?symbol={}&token={}",
                symbol, api_key
            );
            let resp = match self.http.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(symbol, error = %e, "Finnhub profile2 request error");
                    continue;
                }
            };
            let status = resp.status();
            if status.as_u16() == 429 || status.as_u16() == 401 {
                tracing::debug!(symbol, status = %status, key_prefix = &api_key[..8], "Finnhub profile2 rate limited or unauthorized, trying next key");
                continue;
            }
            if !status.is_success() {
                tracing::debug!(symbol, status = %status, "Finnhub profile2 request failed");
                return None;
            }
            let profile: FinnhubProfile = resp.json().await.ok()?;
            return Some(ProfileData {
                name: profile.name.unwrap_or_default(),
                industry: profile.industry.unwrap_or_default(),
                shares_outstanding: profile.share_outstanding.map(|v| (v * 1_000_000.0).round() as i64),
            });
        }
        None
    }
}
