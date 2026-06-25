use super::FundamentalsSnapshot;

/// Data quality report for a single data source.
#[derive(Debug, Clone)]
pub struct DataQualityReport {
    /// Quality score (0.0 - 100.0)
    pub score: f64,
    /// List of missing fields
    pub missing_fields: Vec<String>,
    /// List of warnings
    pub warnings: Vec<String>,
}

impl DataQualityReport {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for DataQualityReport {
    fn default() -> Self {
        Self {
            score: 0.0,
            missing_fields: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Validates data quality and completeness.
pub struct DataValidator;

impl DataValidator {
    /// Validate fundamentals data quality.
    pub fn validate_fundamentals(&self, fundamentals: &FundamentalsSnapshot) -> DataQualityReport {
        let mut report = DataQualityReport::new();
        let mut score = 0.0;

        if fundamentals.market_cap.is_some() {
            score += 20.0;
        } else {
            report.missing_fields.push("market_cap".to_string());
        }

        if fundamentals.revenues_usd.is_some() {
            score += 20.0;
        } else {
            report.missing_fields.push("revenues_usd".to_string());
        }

        if fundamentals.net_income_usd.is_some() {
            score += 20.0;
        } else {
            report.missing_fields.push("net_income_usd".to_string());
        }

        if fundamentals.gross_profit_usd.is_some() {
            score += 20.0;
        } else {
            report.missing_fields.push("gross_profit_usd".to_string());
        }

        if fundamentals.operating_income_usd.is_some() {
            score += 20.0;
        } else {
            report
                .missing_fields
                .push("operating_income_usd".to_string());
        }

        report.score = score;
        report
    }

    /// Calculate overall data quality score.
    pub fn overall_score(
        &self,
        quote_score: f64,
        fundamentals_score: f64,
        news_score: f64,
        candles_score: f64,
    ) -> f64 {
        (quote_score * 0.3 + fundamentals_score * 0.3 + news_score * 0.2 + candles_score * 0.2)
            .clamp(0.0, 100.0)
    }
}
