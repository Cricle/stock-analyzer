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

    /// Try Yahoo Finance, then Finnhub. Returns None if both fail.
    pub async fn fetch(&self, symbol: &str) -> Option<FundamentalsSnapshot> {
        if let Some(result) = self.fetch_yahoo(symbol).await {
            tracing::info!(symbol, "fallback: Yahoo Finance succeeded");
            return Some(result);
        }
        if let Some(result) = self.fetch_finnhub(symbol).await {
            tracing::info!(symbol, "fallback: Finnhub succeeded");
            return Some(result);
        }
        tracing::warn!(symbol, "fallback: all sources failed");
        None
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

    /// Fetch from Finnhub stock/metric API, rotating through API keys.
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

            return Some(FundamentalsSnapshot {
                symbol: symbol.to_string(),
                company_name: String::new(),
                cik: String::new(),
                industry: None,
                currency: "USD".to_string(),
                fiscal_year_end: None,
                shares_outstanding: None,
                market_cap: m.market_cap,
                net_income_usd: None,
                revenues_usd: None,
                assets_usd: None,
                liabilities_usd: None,
                stockholders_equity_usd: None,
                cash_and_equivalents_usd: None,
                gross_profit_usd: None,
                operating_income_usd: None,
                operating_expenses_usd: None,
                operating_cash_flow_usd: None,
                capital_expenditure_usd: None,
                free_cash_flow_usd: None,
                long_term_debt_usd: None,
                current_debt_usd: None,
                total_debt_usd: None,
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
}
