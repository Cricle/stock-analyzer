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
