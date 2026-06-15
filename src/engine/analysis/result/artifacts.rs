use crate::models::AnalysisResult;

impl crate::TaskManager {
    pub(super) async fn write_full_state_log(&self, result: &AnalysisResult) -> anyhow::Result<String> {
        let safe_symbol = crate::engine::shared::safe_ticker_component(&result.symbol, 32)
            .unwrap_or_else(|_| result.symbol.replace('/', "_"));
        let dir = if safe_symbol.is_empty() {
            "results/unknown/TradingAgentsStrategy_logs".to_string()
        } else {
            format!("results/{}/TradingAgentsStrategy_logs", safe_symbol)
        };
        let log_path = format!("{}/full_states_log_{}.json", dir, result.analysis_date);
        let mut body = serde_json::to_value(&result.agent_state)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("artifacts".into(), serde_json::to_value(&result.artifacts)?);
            obj.insert("market_chart".into(), serde_json::to_value(&result.report.market_chart)?);
            obj.insert("price_context".into(), serde_json::to_value(&result.report.price_context)?);
            obj.insert("probability_view".into(), serde_json::to_value(&result.report.probability_view)?);
            obj.insert("profit_risk".into(), serde_json::to_value(&result.report.profit_risk)?);
            obj.insert("ic_navigator".into(), serde_json::to_value(&result.report.ic_navigator)?);
            obj.insert("report".into(), serde_json::to_value(&result.report)?);
            obj.insert("ic_report".into(), serde_json::to_value(&result.ic_report)?);
        }
        self.storage
            .write_file(&log_path, &serde_json::to_vec_pretty(&body)?)
            .await?;
        Ok(log_path)
    }
}
