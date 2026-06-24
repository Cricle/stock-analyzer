use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use crate::models::task::TaskStatus;

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
pub struct AnalysisReuseCheckRequest {
    pub symbol: Option<String>,
    pub stock_code: Option<String>,
    #[serde(default)]
    pub stock_name: Option<String>,
    pub parameters: Option<AnalysisParameters>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisReuseCandidate {
    pub available: bool,
    #[serde(default)]
    pub source_task_id: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub stock_name: String,
    #[serde(default)]
    pub market_type: String,
    #[serde(default)]
    pub analysis_date: String,
    #[serde(default)]
    pub cached_at: String,
    #[serde(default)]
    pub expires_at: String,
    #[serde(default)]
    pub reuse_credits: f64,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub semantic_candidates: Vec<AnalysisReuseSemanticMatch>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisReuseSemanticMatch {
    pub ticker: String,
    pub trade_date: String,
    pub rating: String,
    pub summary: String,
    #[serde(default)]
    pub alpha_return: Option<f64>,
    #[serde(default)]
    pub relevance_score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeAnalysisRequest {
    pub task_id: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_analysis_request_roundtrip() {
        let req = SingleAnalysisRequest {
            symbol: Some("AAPL".to_string()),
            stock_code: None,
            stock_name: Some("Apple".to_string()),
            parameters: None,
            force_refresh: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: SingleAnalysisRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.symbol, Some("AAPL".to_string()));
        assert!(!parsed.force_refresh);
    }

    #[test]
    fn analysis_parameters_default() {
        let params = AnalysisParameters::default();
        assert!(params.market_type.is_none());
        assert!(params.language.is_none());
    }

    #[test]
    fn analysis_parameters_roundtrip() {
        let params = AnalysisParameters {
            market_type: Some("us_equity".to_string()),
            analysis_date: Some("2026-06-23".to_string()),
            language: Some("zh".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: AnalysisParameters = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.market_type, Some("us_equity".to_string()));
    }

    #[test]
    fn stock_pick_request_deserialize() {
        let json = r#"{"market":"a_share","strategy":"momentum"}"#;
        let req: StockPickRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.market, "a_share");
        assert_eq!(req.strategy, Some("momentum".to_string()));
    }

    #[test]
    fn analysis_reuse_candidate_default() {
        let candidate = AnalysisReuseCandidate::default();
        assert!(!candidate.available);
        assert!(candidate.semantic_candidates.is_empty());
    }

    #[test]
    fn analysis_reuse_candidate_roundtrip() {
        let candidate = AnalysisReuseCandidate {
            available: true,
            source_task_id: "task-123".to_string(),
            symbol: "AAPL".to_string(),
            recommendation: "Buy".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&candidate).unwrap();
        let parsed: AnalysisReuseCandidate = serde_json::from_str(&json).unwrap();
        assert!(parsed.available);
        assert_eq!(parsed.recommendation, "Buy");
    }

    #[test]
    fn analysis_outcome_request_roundtrip() {
        let req = AnalysisOutcomeRequest {
            ticker: "AAPL".to_string(),
            trade_date: "2026-06-20".to_string(),
            outcome_return: 0.05,
            benchmark_return: 0.02,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: AnalysisOutcomeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.outcome_return, 0.05);
    }

    #[test]
    fn stock_pick_factor_breakdown_default() {
        let bd = StockPickFactorBreakdown::default();
        assert_eq!(bd.total, 0.0);
        assert_eq!(bd.momentum, 0.0);
    }

    #[test]
    fn stock_pick_factor_breakdown_roundtrip() {
        let bd = StockPickFactorBreakdown {
            momentum: 0.8,
            quality: 0.7,
            total: 0.75,
            ..Default::default()
        };
        let json = serde_json::to_string(&bd).unwrap();
        let parsed: StockPickFactorBreakdown = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.momentum, 0.8);
    }

    #[test]
    fn stock_pick_market_snapshot_default() {
        let snap = StockPickMarketSnapshot::default();
        assert!(snap.current_price.is_none());
        assert_eq!(snap.lookback_candles, 0);
    }

    #[test]
    fn stock_pick_technical_snapshot_default() {
        let snap = StockPickTechnicalSnapshot::default();
        assert!(snap.rsi.is_none());
        assert!(snap.macd.is_none());
    }

    #[test]
    fn stock_pick_fundamental_snapshot_default() {
        let snap = StockPickFundamentalSnapshot::default();
        assert!(snap.industry.is_empty());
        assert!(snap.market_cap.is_none());
    }

    #[test]
    fn stock_pick_news_snapshot_default() {
        let snap = StockPickNewsSnapshot::default();
        assert_eq!(snap.light_item_count, 0);
        assert!(snap.headline_titles.is_empty());
    }

    #[test]
    fn stock_pick_risk_snapshot_default() {
        let snap = StockPickRiskSnapshot::default();
        assert!(!snap.hard_negative_news);
        assert!(snap.signal_codes.is_empty());
    }

    #[test]
    fn stock_pick_data_quality_default() {
        let snap = StockPickDataQualitySnapshot::default();
        assert!(!snap.quote_ready);
        assert_eq!(snap.completeness_score, 0);
        assert!(snap.gaps.is_empty());
    }

    #[test]
    fn resume_analysis_request_roundtrip() {
        let req = ResumeAnalysisRequest {
            task_id: "task-456".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ResumeAnalysisRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, "task-456");
    }
}
