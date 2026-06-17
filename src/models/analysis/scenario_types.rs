use crate::data::MarketKind;

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
        let trimmed = market_type.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
            return Self::Unknown;
        }
        match MarketKind::from_market_str(trimmed) {
            MarketKind::AShare => Self::AShare,
            MarketKind::HongKong => Self::HongKong,
            MarketKind::UsEquity => Self::UsEquity,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::AShare => MarketKind::AShare.market_key(),
            Self::HongKong => MarketKind::HongKong.market_key(),
            Self::UsEquity => MarketKind::UsEquity.market_key(),
            Self::Unknown => "unknown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AShare => MarketKind::AShare.display_label(),
            Self::HongKong => MarketKind::HongKong.display_label(),
            Self::UsEquity => MarketKind::UsEquity.display_label(),
            Self::Unknown => "Unknown",
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
}
