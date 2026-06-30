#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SingleAnalysisRequest {
    pub symbol: Option<String>,
    pub stock_code: Option<String>,
    #[serde(default)]
    pub stock_name: Option<String>,
    pub parameters: Option<AnalysisParameters>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisOutcomeRequest {
    pub ticker: String,
    pub trade_date: String,
    pub outcome_return: f64,
    pub benchmark_return: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(Default)]
pub struct AnalysisParameters {
    pub market_type: Option<String>,
    pub analysis_date: Option<String>,
    pub selected_analysts: Option<Vec<String>>,
    pub custom_prompt: Option<String>,
    pub include_sentiment: Option<bool>,
    pub include_risk: Option<bool>,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub quick_analysis_model: Option<String>,
    pub deep_analysis_model: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub user_position_state: Option<String>,
    #[serde(default)]
    pub workflow_intent: Option<String>,
    #[serde(default)]
    pub holding_cost: Option<f64>,
    #[serde(default)]
    pub holding_ratio_pct: Option<f64>,
    #[serde(default)]
    pub risk_preference: Option<String>,
    #[serde(default)]
    pub investment_horizon: Option<String>,
    #[serde(default)]
    pub user_notes: Option<String>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockPickRequest {
    pub market: String,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub candidate_symbols: Option<Vec<String>>,
    #[serde(default)]
    pub sector_type: Option<String>,
    #[serde(default)]
    pub candidate_limit: Option<usize>,
    #[serde(default)]
    pub pick_count: Option<usize>,
    #[serde(default)]
    pub analysis_date: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub target_output_mode: Option<String>,
    #[serde(default)]
    pub search_depth: Option<String>,
    #[serde(default)]
    pub history_retrieval: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickFactorBreakdown {
    #[serde(default)]
    pub momentum: f64,
    #[serde(default)]
    pub quality: f64,
    #[serde(default)]
    pub value: f64,
    #[serde(default)]
    pub profitability: f64,
    #[serde(default)]
    pub risk: f64,
    #[serde(default)]
    pub event: f64,
    #[serde(default)]
    pub evidence: f64,
    #[serde(default)]
    pub history: f64,
    #[serde(default)]
    pub penalty: f64,
    #[serde(default)]
    pub total: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickMarketSnapshot {
    #[serde(default)]
    pub current_price: Option<f64>,
    #[serde(default)]
    pub latest_change_pct: Option<f64>,
    #[serde(default)]
    pub lookback_candles: usize,
    #[serde(default)]
    pub period_return_pct: Option<f64>,
    #[serde(default)]
    pub latest_volume: Option<i64>,
    #[serde(default)]
    pub volume_ratio: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickTechnicalSnapshot {
    #[serde(default)]
    pub close_10_ema: Option<f64>,
    #[serde(default)]
    pub close_50_sma: Option<f64>,
    #[serde(default)]
    pub close_200_sma: Option<f64>,
    #[serde(default)]
    pub rsi: Option<f64>,
    #[serde(default)]
    pub atr: Option<f64>,
    #[serde(default)]
    pub macd: Option<f64>,
    #[serde(default)]
    pub macd_signal: Option<f64>,
    #[serde(default)]
    pub macd_hist: Option<f64>,
    #[serde(default)]
    pub adx: Option<f64>,
    #[serde(default)]
    pub kdj_k: Option<f64>,
    #[serde(default)]
    pub kdj_d: Option<f64>,
    #[serde(default)]
    pub kdj_j: Option<f64>,
    #[serde(default)]
    pub cci: Option<f64>,
    #[serde(default)]
    pub wr: Option<f64>,
    #[serde(default)]
    pub obv: Option<f64>,
    #[serde(default)]
    pub vwap: Option<f64>,
    #[serde(default)]
    pub vwma: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickFundamentalSnapshot {
    #[serde(default)]
    pub industry: String,
    #[serde(default)]
    pub market_cap: Option<f64>,
    #[serde(default)]
    pub revenues_usd: Option<f64>,
    #[serde(default)]
    pub net_income_usd: Option<f64>,
    #[serde(default)]
    pub free_cash_flow_usd: Option<f64>,
    #[serde(default)]
    pub total_debt_usd: Option<f64>,
    #[serde(default)]
    pub cash_and_equivalents_usd: Option<f64>,
    #[serde(default)]
    pub pe_like: Option<f64>,
    #[serde(default)]
    pub ps_like: Option<f64>,
    #[serde(default)]
    pub roe: Option<f64>,
    #[serde(default)]
    pub leverage: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickNewsSnapshot {
    #[serde(default)]
    pub light_item_count: usize,
    #[serde(default)]
    pub deep_item_count: usize,
    #[serde(default)]
    pub unique_source_count: usize,
    #[serde(default)]
    pub latest_published_at: String,
    #[serde(default)]
    pub evidence_count: usize,
    #[serde(default)]
    pub hard_negative_count: usize,
    #[serde(default)]
    pub catalyst_count: usize,
    #[serde(default)]
    pub headline_titles: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickHistoryMatchSnapshot {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sample_count: usize,
    #[serde(default)]
    pub vector_hit_count: usize,
    #[serde(default)]
    pub average_score: Option<f64>,
    #[serde(default)]
    pub hit_rate: Option<f64>,
    #[serde(default)]
    pub average_alpha_return: Option<f64>,
    #[serde(default)]
    pub top_matches: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickRiskSnapshot {
    #[serde(default)]
    pub hard_negative_news: bool,
    #[serde(default)]
    pub volatility_elevated: bool,
    #[serde(default)]
    pub liquidity_warning: bool,
    #[serde(default)]
    pub valuation_stretched: bool,
    #[serde(default)]
    pub signal_codes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockPickDataQualitySnapshot {
    #[serde(default)]
    pub quote_ready: bool,
    #[serde(default)]
    pub fundamentals_ready: bool,
    #[serde(default)]
    pub technical_ready: bool,
    #[serde(default)]
    pub news_ready: bool,
    #[serde(default)]
    pub history_ready: bool,
    #[serde(default)]
    pub qdrant_ready: bool,
    #[serde(default)]
    pub redis_ready: bool,
    #[serde(default)]
    pub completeness_score: i32,
    #[serde(default)]
    pub gaps: Vec<String>,
}
