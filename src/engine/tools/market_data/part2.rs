impl TradingToolbox {

    pub(super) async fn get_cashflow(
        &self,
        symbol: &str,
        scenario_data: Option<&AnalysisScenarioData>,
    ) -> anyhow::Result<ToolExecutionResult> {
        let (item, source) = self.fetch_fundamentals_item(symbol, scenario_data).await?;
        let full = serde_json::to_value(&item)?;
        let payload = Self::fundamentals_subset(&full, &[
            "company_name", "operating_cash_flow_usd", "capital_expenditure_usd",
            "free_cash_flow_usd", "cash_and_equivalents_usd", "net_income_usd",
        ]);
        Ok(ToolExecutionResult {
            output: serde_json::to_string_pretty(&payload)?,
            meta: json!({ "kind": "cashflow", "source": source }),
        })
    }

    pub(super) async fn get_income_statement(
        &self,
        symbol: &str,
        scenario_data: Option<&AnalysisScenarioData>,
    ) -> anyhow::Result<ToolExecutionResult> {
        let (item, source) = self.fetch_fundamentals_item(symbol, scenario_data).await?;
        let full = serde_json::to_value(&item)?;
        let payload = Self::fundamentals_subset(&full, &[
            "revenues_usd", "net_income_usd", "gross_profit_usd",
            "operating_income_usd", "operating_expenses_usd", "currency",
        ]);
        Ok(ToolExecutionResult {
            output: serde_json::to_string_pretty(&payload)?,
            meta: json!({ "kind": "income_statement", "source": source }),
        })
    }
}
