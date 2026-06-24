use std::{fs, path::PathBuf};

use crate::models::AnalysisResult;

impl crate::TaskManager {
    pub(super) fn write_full_state_log(&self, result: &AnalysisResult) -> anyhow::Result<String> {
        let safe_symbol = crate::engine::shared::safe_ticker_component(&result.symbol, 32)
            .unwrap_or_else(|_| result.symbol.replace('/', "_"));
        let log_dir = PathBuf::from(&self.data_dir)
            .join("results")
            .join(if safe_symbol.is_empty() {
                "unknown"
            } else {
                &safe_symbol
            })
            .join("TradingAgentsStrategy_logs");
        fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("full_states_log_{}.json", result.analysis_date));
        let body = serde_json::json!({
            "company_of_interest": result.agent_state.company_of_interest,
            "trade_date": result.agent_state.trade_date,
            "sender": result.agent_state.sender,
            "market_report": result.agent_state.market_report,
            "sentiment_report": result.agent_state.sentiment_report,
            "news_report": result.agent_state.news_report,
            "fundamentals_report": result.agent_state.fundamentals_report,
            "investment_debate_state": result.agent_state.investment_debate_state,
            "investment_plan": result.agent_state.investment_plan,
            "trader_investment_plan": result.agent_state.trader_investment_plan,
            "risk_debate_state": result.agent_state.risk_debate_state,
            "final_trade_decision": result.agent_state.final_trade_decision,
            "past_context": result.agent_state.past_context,
            "artifacts": result.artifacts,
            "market_chart": result.report.market_chart,
            "price_context": result.report.price_context,
            "probability_view": result.report.probability_view,
            "profit_risk": result.report.profit_risk,
            "ic_navigator": result.report.ic_navigator,
            "report": result.report,
            "ic_report": result.ic_report
        });
        fs::write(&log_path, serde_json::to_vec_pretty(&body)?)?;
        Ok(log_path.display().to_string())
    }
}
