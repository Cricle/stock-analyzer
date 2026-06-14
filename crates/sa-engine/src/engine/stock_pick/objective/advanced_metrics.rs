//! Advanced financial metrics computed from [`FundamentalsSnapshot`].
//!
//! Includes Piotroski F-Score (single-period variant), ROIC, Graham Number,
//! and industry-relative z-scores for PE/PS/ROE.

#![allow(dead_code)] // Will be used once integrated into scoring pipeline

use crate::data::FundamentalsSnapshot;
use crate::engine::math_utils::z_score;
use crate::engine::stock_pick::objective::IndustryAverages;

/// Advanced financial metrics derived from fundamental data.
#[derive(Debug, Clone, Default)]
pub(crate) struct AdvancedMetrics {
    /// Piotroski F-Score (0-9), single-period variant.
    pub piotroski_f_score: Option<u8>,
    /// Return on Invested Capital: NOPAT / invested_capital.
    pub roic: Option<f64>,
    /// Graham Number: sqrt(22.5 * EPS * BVPS).
    pub graham_number: Option<f64>,
    /// Free Cash Flow Yield: FCF / market_cap.
    pub fcf_yield: Option<f64>,
    /// Earnings Yield: 1 / PE.
    pub earnings_yield: Option<f64>,
    /// Debt to Equity ratio.
    pub debt_to_equity: Option<f64>,
    /// Gross margin: gross_profit / revenue.
    pub gross_margin: Option<f64>,
    /// Net margin: net_income / revenue.
    pub net_margin: Option<f64>,
    /// Return on Assets: net_income / total_assets.
    pub roa: Option<f64>,
    /// Return on Equity (from snapshot).
    pub roe: Option<f64>,
    /// Cash conversion: operating_cash_flow / net_income.
    pub cash_conversion: Option<f64>,
    /// Asset turnover: revenue / total_assets.
    pub asset_turnover: Option<f64>,
    /// PE z-score relative to industry mean (negative = cheaper).
    pub pe_deviation_z: Option<f64>,
    /// PS z-score relative to industry mean (negative = cheaper).
    pub ps_deviation_z: Option<f64>,
    /// ROE z-score relative to industry mean (positive = better).
    pub roe_deviation_z: Option<f64>,
}

impl AdvancedMetrics {
    /// Compute advanced metrics from fundamental data and industry averages.
    pub fn compute(
        f: &FundamentalsSnapshot,
        pe_like: Option<f64>,
        ps_like: Option<f64>,
        roe: Option<f64>,
        industry_avg: Option<&IndustryAverages>,
    ) -> Self {
        let net_income = f.net_income_usd;
        let revenue = f.revenues_usd;
        let assets = f.assets_usd;
        let liabilities = f.liabilities_usd;
        let equity = f.stockholders_equity_usd;
        let cash = f.cash_and_equivalents_usd;
        let gross_profit = f.gross_profit_usd;
        let operating_income = f.operating_income_usd;
        let ocf = f.operating_cash_flow_usd;
        let fcf = f.free_cash_flow_usd;
        let long_term_debt = f.long_term_debt_usd;
        let market_cap = f.market_cap;
        let shares = f.diluted_shares_outstanding.or(f.shares_outstanding);

        // --- Piotroski F-Score (single-period variant, 7 of 9 signals) ---
        let f_score = compute_piotroski(net_income, ocf, gross_profit, revenue, assets, equity, long_term_debt, liabilities);

        // --- ROIC ---
        let roic = compute_roic(operating_income, equity, long_term_debt, cash);

        // --- Graham Number ---
        let graham_number = compute_graham_number(net_income, equity, shares);

        // --- FCF Yield ---
        let fcf_yield = match (fcf, market_cap) {
            (Some(f), Some(m)) if m > 0.0 => Some(f / m),
            _ => None,
        };

        // --- Earnings Yield ---
        let earnings_yield = pe_like.and_then(|pe| if pe > 0.0 { Some(1.0 / pe) } else { None });

        // --- Debt to Equity ---
        let debt_to_equity = match (liabilities, equity) {
            (Some(l), Some(e)) if e > 0.0 => Some(l / e),
            _ => None,
        };

        // --- Margins ---
        let gross_margin = match (gross_profit, revenue) {
            (Some(gp), Some(r)) if r > 0.0 => Some(gp / r),
            _ => None,
        };
        let net_margin = match (net_income, revenue) {
            (Some(ni), Some(r)) if r > 0.0 => Some(ni / r),
            _ => None,
        };

        // --- ROA ---
        let roa = match (net_income, assets) {
            (Some(ni), Some(a)) if a > 0.0 => Some(ni / a),
            _ => None,
        };

        // --- Cash Conversion ---
        let cash_conversion = match (ocf, net_income) {
            (Some(o), Some(ni)) if ni.abs() > 1e-10 => Some(o / ni),
            _ => None,
        };

        // --- Asset Turnover ---
        let asset_turnover = match (revenue, assets) {
            (Some(r), Some(a)) if a > 0.0 => Some(r / a),
            _ => None,
        };

        // --- Industry z-scores ---
        let (pe_z, ps_z, roe_z) = if let Some(avg) = industry_avg {
            (
                pe_like.map(|pe| z_score(pe, avg.pe_avg, avg.pe_std)),
                ps_like.map(|ps| z_score(ps, avg.ps_avg, avg.ps_std)),
                roe.map(|r| z_score(r, avg.roe_avg, avg.roe_std)),
            )
        } else {
            (None, None, None)
        };

        AdvancedMetrics {
            piotroski_f_score: f_score,
            roic,
            graham_number,
            fcf_yield,
            earnings_yield,
            debt_to_equity,
            gross_margin,
            net_margin,
            roa,
            roe,
            cash_conversion,
            asset_turnover,
            pe_deviation_z: pe_z,
            ps_deviation_z: ps_z,
            roe_deviation_z: roe_z,
        }
    }
}

/// Compute Piotroski F-Score (single-period variant).
///
/// Uses 7 of 9 signals that can be computed from a single period:
/// 1. Net income > 0
/// 2. Operating cash flow > 0
/// 3. ROA > 0 (net_income / assets)
/// 4. Cash quality (OCF > net_income)
/// 5. No new equity issuance (equity > 0 as proxy)
/// 6. Gross margin > 0
/// 7. Asset turnover > industry median (simplified: > 0.5)
///
/// Signals 3 (ROA rising) and 8 (margin rising) require prior period data.
#[allow(clippy::too_many_arguments)]
fn compute_piotroski(
    net_income: Option<f64>,
    ocf: Option<f64>,
    gross_profit: Option<f64>,
    revenue: Option<f64>,
    assets: Option<f64>,
    equity: Option<f64>,
    _long_term_debt: Option<f64>,
    _liabilities: Option<f64>,
) -> Option<u8> {
    let mut score: u8 = 0;
    let mut has_any = false;

    // 1. Profitability: net income > 0
    if let Some(ni) = net_income {
        has_any = true;
        if ni > 0.0 {
            score += 1;
        }
    }

    // 2. Operating cash flow > 0
    if let Some(o) = ocf {
        has_any = true;
        if o > 0.0 {
            score += 1;
        }
    }

    // 3. ROA > 0
    if let (Some(ni), Some(a)) = (net_income, assets)
        && a > 0.0
    {
        has_any = true;
        if ni / a > 0.0 {
            score += 1;
        }
    }

    // 4. Cash quality: OCF > net_income
    if let (Some(o), Some(ni)) = (ocf, net_income) {
        has_any = true;
        if o > ni {
            score += 1;
        }
    }

    // 5. No new equity (equity positive as proxy)
    if let Some(e) = equity {
        has_any = true;
        if e > 0.0 {
            score += 1;
        }
    }

    // 6. Gross margin > 0
    if let (Some(gp), Some(r)) = (gross_profit, revenue)
        && r > 0.0
    {
        has_any = true;
        if gp / r > 0.0 {
            score += 1;
        }
    }

    // 7. Asset turnover (simplified: revenue / assets > 0.5)
    if let (Some(r), Some(a)) = (revenue, assets)
        && a > 0.0
    {
        has_any = true;
        if r / a > 0.5 {
            score += 1;
        }
    }

    if has_any { Some(score) } else { None }
}

/// Compute ROIC: NOPAT / invested_capital.
/// NOPAT = operating_income * (1 - 0.25) (assumed 25% tax rate).
/// invested_capital = equity + long_term_debt - cash.
fn compute_roic(
    operating_income: Option<f64>,
    equity: Option<f64>,
    long_term_debt: Option<f64>,
    cash: Option<f64>,
) -> Option<f64> {
    let oi = operating_income?;
    let eq = equity.unwrap_or(0.0);
    let ltd = long_term_debt.unwrap_or(0.0);
    let c = cash.unwrap_or(0.0);
    let invested_capital = eq + ltd - c;
    if invested_capital <= 0.0 {
        return None;
    }
    let nopat = oi * 0.75; // assumed 25% tax rate
    Some(nopat / invested_capital)
}

/// Compute Graham Number: sqrt(22.5 * EPS * BVPS).
fn compute_graham_number(
    net_income: Option<f64>,
    equity: Option<f64>,
    shares: Option<i64>,
) -> Option<f64> {
    let ni = net_income?;
    let eq = equity?;
    let s = shares? as f64;
    if s <= 0.0 || eq <= 0.0 {
        return None;
    }
    let eps = ni / s;
    let bvps = eq / s;
    if eps <= 0.0 || bvps <= 0.0 {
        return None;
    }
    Some((22.5 * eps * bvps).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piotroski_all_positive() {
        let fs = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "Test".to_string(),
            cik: String::new(),
            industry: None,
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: Some(1000),
            market_cap: Some(50000.0),
            net_income_usd: Some(1000.0),
            revenues_usd: Some(5000.0),
            assets_usd: Some(10000.0),
            liabilities_usd: Some(4000.0),
            stockholders_equity_usd: Some(6000.0),
            cash_and_equivalents_usd: Some(1000.0),
            gross_profit_usd: Some(2000.0),
            operating_income_usd: Some(1500.0),
            operating_expenses_usd: None,
            operating_cash_flow_usd: Some(1800.0),
            capital_expenditure_usd: None,
            free_cash_flow_usd: Some(1200.0),
            long_term_debt_usd: Some(2000.0),
            current_debt_usd: None,
            total_debt_usd: Some(4000.0),
            diluted_shares_outstanding: Some(1000),
        };
        let metrics = AdvancedMetrics::compute(&fs, Some(15.0), Some(3.0), Some(0.15), None);
        // F-Score: NI>0, OCF>0, ROA>0, OCF>NI, equity>0, GM>0 => 6
        // (AT = 5000/10000 = 0.5, NOT > 0.5)
        assert_eq!(metrics.piotroski_f_score, Some(6));
        assert!(metrics.roic.is_some());
        assert!(metrics.graham_number.is_some());
        assert!(metrics.fcf_yield.is_some());
    }

    #[test]
    fn test_piotroski_all_negative() {
        let fs = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "Test".to_string(),
            cik: String::new(),
            industry: None,
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: None,
            market_cap: None,
            net_income_usd: Some(-500.0),
            revenues_usd: Some(1000.0),
            assets_usd: Some(5000.0),
            liabilities_usd: Some(6000.0),
            stockholders_equity_usd: Some(-1000.0),
            cash_and_equivalents_usd: None,
            gross_profit_usd: Some(-100.0),
            operating_income_usd: None,
            operating_expenses_usd: None,
            operating_cash_flow_usd: Some(-200.0),
            capital_expenditure_usd: None,
            free_cash_flow_usd: None,
            long_term_debt_usd: None,
            current_debt_usd: None,
            total_debt_usd: Some(6000.0),
            diluted_shares_outstanding: None,
        };
        let metrics = AdvancedMetrics::compute(&fs, None, None, None, None);
        // NI<0, OCF<0, ROA<0, OCF>NI (cash quality: -200 > -500), equity<0, GM<0, AT<0.5 => 1
        assert_eq!(metrics.piotroski_f_score, Some(1));
    }

    #[test]
    fn test_roic_positive() {
        let fs = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "Test".to_string(),
            cik: String::new(),
            industry: None,
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: None,
            market_cap: None,
            net_income_usd: None,
            revenues_usd: None,
            assets_usd: None,
            liabilities_usd: None,
            stockholders_equity_usd: Some(8000.0),
            cash_and_equivalents_usd: Some(2000.0),
            gross_profit_usd: None,
            operating_income_usd: Some(1500.0),
            operating_expenses_usd: None,
            operating_cash_flow_usd: None,
            capital_expenditure_usd: None,
            free_cash_flow_usd: None,
            long_term_debt_usd: Some(4000.0),
            current_debt_usd: None,
            total_debt_usd: None,
            diluted_shares_outstanding: None,
        };
        let metrics = AdvancedMetrics::compute(&fs, None, None, None, None);
        // NOPAT = 1500 * 0.75 = 1125
        // invested_capital = 8000 + 4000 - 2000 = 10000
        // ROIC = 1125 / 10000 = 0.1125
        let roic = metrics.roic.unwrap();
        assert!((roic - 0.1125).abs() < 1e-10);
    }

    #[test]
    fn test_graham_number() {
        let fs = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "Test".to_string(),
            cik: String::new(),
            industry: None,
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: Some(1000),
            market_cap: None,
            net_income_usd: Some(5000.0),
            revenues_usd: None,
            assets_usd: None,
            liabilities_usd: None,
            stockholders_equity_usd: Some(20000.0),
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
            diluted_shares_outstanding: Some(1000),
        };
        let metrics = AdvancedMetrics::compute(&fs, None, None, None, None);
        // EPS = 5000/1000 = 5, BVPS = 20000/1000 = 20
        // Graham = sqrt(22.5 * 5 * 20) = sqrt(2250) = 47.43
        let gn = metrics.graham_number.unwrap();
        assert!((gn - (2250.0_f64).sqrt()).abs() < 1e-10);
    }
}
