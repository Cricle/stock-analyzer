#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisScenarioMarket {
    AShare,
    HongKong,
    UsEquity,
    #[default]
    Unknown,
}

impl AnalysisScenarioMarket {
    pub fn from_market_type(market_type: &str) -> Self {
        match market_type.trim().to_ascii_lowercase().as_str() {
            "a股" | "a_share" | "a-share" | "cn" | "china" => Self::AShare,
            "港股" | "hk" | "hk_equity" | "hongkong" | "hong_kong" => Self::HongKong,
            "美股" | "us" | "us_equity" | "usa" | "us-stock" => Self::UsEquity,
            _ => Self::Unknown,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::AShare => "a_share",
            Self::HongKong => "hk_equity",
            Self::UsEquity => "us_equity",
            Self::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AShare => "A股",
            Self::HongKong => "港股",
            Self::UsEquity => "美股",
            Self::Unknown => "未知市场",
        }
    }

    pub fn supports_company_news(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    pub fn supports_global_news(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    pub fn supports_insider_transactions(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisScenarioContext {
    #[serde(default)]
    pub market: AnalysisScenarioMarket,
    #[serde(default)]
    pub market_key: String,
    #[serde(default)]
    pub market_label: String,
    #[serde(default)]
    pub supports_company_news: bool,
    #[serde(default)]
    pub supports_global_news: bool,
    #[serde(default)]
    pub supports_insider_transactions: bool,
}

impl Default for AnalysisScenarioContext {
    fn default() -> Self {
        Self::from_market_type("")
    }
}

impl AnalysisScenarioContext {
    pub fn from_market_type(market_type: &str) -> Self {
        let market = AnalysisScenarioMarket::from_market_type(market_type);
        Self {
            market,
            market_key: market.key().to_string(),
            market_label: market.label().to_string(),
            supports_company_news: market.supports_company_news(),
            supports_global_news: market.supports_global_news(),
            supports_insider_transactions: market.supports_insider_transactions(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisScenarioIssue {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisScenarioData {
    #[serde(default)]
    pub prefetched_at: String,
    #[serde(default)]
    pub quote_status: String,
    #[serde(default)]
    pub fundamentals_status: String,
    #[serde(default)]
    pub company_news_status: String,
    #[serde(default)]
    pub candles_status: String,
    #[serde(default)]
    pub company_news_start_date: Option<String>,
    #[serde(default)]
    pub company_news_end_date: Option<String>,
    #[serde(default)]
    pub candle_adjust: String,
    #[serde(default)]
    pub quote: Option<crate::types::QuoteSnapshot>,
    #[serde(default)]
    pub fundamentals: Option<crate::types::FundamentalsSnapshot>,
    #[serde(default)]
    pub company_news: Vec<crate::types::NewsItem>,
    #[serde(default)]
    pub candles: Vec<crate::types::CandlePoint>,
    #[serde(default)]
    pub issues: Vec<AnalysisScenarioIssue>,
    /// Per-data-type fetch diagnosis (quote, candles, etc.) from provider rotation.
    #[serde(default)]
    pub fetch_diagnosis: Vec<serde_json::Value>,

    // Additional market data summaries (compact text for LLM context)
    /// Fund flow summary (资金流向)
    #[serde(default)]
    pub fund_flow_summary: String,
    /// Billboard / Dragon Tiger List summary (龙虎榜)
    #[serde(default)]
    pub billboard_summary: String,
    /// Margin trading summary (融资融券)
    #[serde(default)]
    pub margin_summary: String,
    /// Hot rank summary (热度排名)
    #[serde(default)]
    pub hot_rank_summary: String,
    /// Limit-up pool summary (涨停池)
    #[serde(default)]
    pub limit_pool_summary: String,
    /// Earnings forecast summary (业绩预告)
    #[serde(default)]
    pub earnings_forecast_summary: String,
    /// Shareholder analysis summary (股东分析)
    #[serde(default)]
    pub shareholder_summary: String,
    /// Technical indicators summary
    #[serde(default)]
    pub technical_summary: String,
}

impl AnalysisScenarioData {
    pub fn add_issue(
        &mut self,
        domain: impl Into<String>,
        code: impl Into<String>,
        severity: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(AnalysisScenarioIssue {
            domain: domain.into(),
            code: code.into(),
            severity: severity.into(),
            message: message.into(),
        });
    }

    /// Convert to a [`crate::types::ScenarioData`] for use with the trading toolbox.
    pub fn to_scenario_data(&self) -> crate::types::ScenarioData {
        crate::types::ScenarioData {
            candles: self.candles.clone(),
            fundamentals: self.fundamentals.clone(),
            company_news: self.company_news.clone(),
            quote: self.quote.clone(),
        }
    }
}

#[cfg(test)]
mod scenario_types_tests {
    use super::*;

    // --- AnalysisScenarioMarket::from_market_type ---

    #[test]
    fn analysis_scenario_market_a_share() {
        assert_eq!(AnalysisScenarioMarket::from_market_type("A股"), AnalysisScenarioMarket::AShare);
        assert_eq!(AnalysisScenarioMarket::from_market_type("a_share"), AnalysisScenarioMarket::AShare);
        assert_eq!(AnalysisScenarioMarket::from_market_type("a-share"), AnalysisScenarioMarket::AShare);
        assert_eq!(AnalysisScenarioMarket::from_market_type("cn"), AnalysisScenarioMarket::AShare);
        assert_eq!(AnalysisScenarioMarket::from_market_type("china"), AnalysisScenarioMarket::AShare);
    }

    #[test]
    fn analysis_scenario_market_hong_kong() {
        assert_eq!(AnalysisScenarioMarket::from_market_type("港股"), AnalysisScenarioMarket::HongKong);
        assert_eq!(AnalysisScenarioMarket::from_market_type("hk"), AnalysisScenarioMarket::HongKong);
        assert_eq!(AnalysisScenarioMarket::from_market_type("hongkong"), AnalysisScenarioMarket::HongKong);
        assert_eq!(AnalysisScenarioMarket::from_market_type("hong_kong"), AnalysisScenarioMarket::HongKong);
    }

    #[test]
    fn analysis_scenario_market_us_equity() {
        assert_eq!(AnalysisScenarioMarket::from_market_type("美股"), AnalysisScenarioMarket::UsEquity);
        assert_eq!(AnalysisScenarioMarket::from_market_type("us"), AnalysisScenarioMarket::UsEquity);
        assert_eq!(AnalysisScenarioMarket::from_market_type("us_equity"), AnalysisScenarioMarket::UsEquity);
        assert_eq!(AnalysisScenarioMarket::from_market_type("usa"), AnalysisScenarioMarket::UsEquity);
    }

    #[test]
    fn analysis_scenario_market_unknown() {
        assert_eq!(AnalysisScenarioMarket::from_market_type("unknown"), AnalysisScenarioMarket::Unknown);
        assert_eq!(AnalysisScenarioMarket::from_market_type(""), AnalysisScenarioMarket::Unknown);
    }

    // --- AnalysisScenarioMarket::key ---

    #[test]
    fn analysis_scenario_market_key() {
        assert_eq!(AnalysisScenarioMarket::AShare.key(), "a_share");
        assert_eq!(AnalysisScenarioMarket::HongKong.key(), "hk_equity");
        assert_eq!(AnalysisScenarioMarket::UsEquity.key(), "us_equity");
        assert_eq!(AnalysisScenarioMarket::Unknown.key(), "unknown");
    }

    // --- AnalysisScenarioMarket::label ---

    #[test]
    fn analysis_scenario_market_label() {
        assert_eq!(AnalysisScenarioMarket::AShare.label(), "A股");
        assert_eq!(AnalysisScenarioMarket::HongKong.label(), "港股");
        assert_eq!(AnalysisScenarioMarket::UsEquity.label(), "美股");
        assert_eq!(AnalysisScenarioMarket::Unknown.label(), "未知市场");
    }

    // --- AnalysisScenarioMarket supports_* ---

    #[test]
    fn analysis_scenario_market_supports_company_news() {
        assert!(AnalysisScenarioMarket::AShare.supports_company_news());
        assert!(AnalysisScenarioMarket::HongKong.supports_company_news());
        assert!(AnalysisScenarioMarket::UsEquity.supports_company_news());
        assert!(!AnalysisScenarioMarket::Unknown.supports_company_news());
    }

    #[test]
    fn analysis_scenario_market_supports_global_news() {
        assert!(AnalysisScenarioMarket::AShare.supports_global_news());
        assert!(AnalysisScenarioMarket::HongKong.supports_global_news());
        assert!(AnalysisScenarioMarket::UsEquity.supports_global_news());
        assert!(!AnalysisScenarioMarket::Unknown.supports_global_news());
    }

    #[test]
    fn analysis_scenario_market_supports_insider_transactions() {
        assert!(AnalysisScenarioMarket::AShare.supports_insider_transactions());
        assert!(AnalysisScenarioMarket::HongKong.supports_insider_transactions());
        assert!(AnalysisScenarioMarket::UsEquity.supports_insider_transactions());
        assert!(!AnalysisScenarioMarket::Unknown.supports_insider_transactions());
    }

    // --- AnalysisScenarioContext::from_market_type ---

    #[test]
    fn analysis_scenario_context_a_share() {
        let ctx = AnalysisScenarioContext::from_market_type("A股");
        assert_eq!(ctx.market, AnalysisScenarioMarket::AShare);
        assert_eq!(ctx.market_key, "a_share");
        assert_eq!(ctx.market_label, "A股");
        assert!(ctx.supports_company_news);
        assert!(ctx.supports_global_news);
        assert!(ctx.supports_insider_transactions);
    }

    #[test]
    fn analysis_scenario_context_unknown() {
        let ctx = AnalysisScenarioContext::from_market_type("");
        assert_eq!(ctx.market, AnalysisScenarioMarket::Unknown);
        assert_eq!(ctx.market_key, "unknown");
        assert_eq!(ctx.market_label, "未知市场");
        assert!(!ctx.supports_company_news);
    }

    // --- AnalysisScenarioData::add_issue ---

    #[test]
    fn analysis_scenario_data_add_issue() {
        let mut data = AnalysisScenarioData::default();
        data.add_issue("quote", "fetch_failed", "warning", "timeout");
        assert_eq!(data.issues.len(), 1);
        assert_eq!(data.issues[0].domain, "quote");
        assert_eq!(data.issues[0].code, "fetch_failed");
        assert_eq!(data.issues[0].severity, "warning");
        assert_eq!(data.issues[0].message, "timeout");
    }

    #[test]
    fn analysis_scenario_data_add_multiple_issues() {
        let mut data = AnalysisScenarioData::default();
        data.add_issue("quote", "q1", "warning", "msg1");
        data.add_issue("candles", "c1", "error", "msg2");
        assert_eq!(data.issues.len(), 2);
    }

    // --- serde roundtrip ---

    #[test]
    fn analysis_scenario_market_serde_roundtrip() {
        let markets = [
            AnalysisScenarioMarket::AShare,
            AnalysisScenarioMarket::HongKong,
            AnalysisScenarioMarket::UsEquity,
            AnalysisScenarioMarket::Unknown,
        ];
        for market in &markets {
            let json = serde_json::to_string(market).unwrap();
            let restored: AnalysisScenarioMarket = serde_json::from_str(&json).unwrap();
            assert_eq!(*market, restored);
        }
    }

    #[test]
    fn analysis_scenario_context_serde_roundtrip() {
        let ctx = AnalysisScenarioContext::from_market_type("A股");
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: AnalysisScenarioContext = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.market, ctx.market);
        assert_eq!(restored.market_key, ctx.market_key);
    }
}
