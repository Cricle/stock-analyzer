impl StructuredTraderPlan {
    /// Compute Render_markdown.
    pub fn render_markdown(&self) -> String {
        let mut parts = vec![
            "# Trader Execution Plan".to_string(),
            String::new(),
            "## Proposed Action".to_string(),
            format!("**Action**: {}", self.action),
            String::new(),
            "## Execution Logic".to_string(),
            format!("**Reasoning**: {}", self.reasoning),
        ];

        let mut level_lines = Vec::new();
        if !self.entry_price.trim().is_empty() {
            level_lines.push(format!("**Entry Price**: {}", self.entry_price));
        }
        if !self.stop_loss.trim().is_empty() {
            level_lines.push(format!("**Stop Loss**: {}", self.stop_loss));
        }
        if !self.confirmation_level.trim().is_empty() {
            level_lines.push(format!("**Confirmation Level**: {}", self.confirmation_level));
        }
        if !self.target_reference.trim().is_empty() {
            level_lines.push(format!("**Target Reference**: {}", self.target_reference));
        }
        if !self.target_condition.trim().is_empty() {
            level_lines.push(format!("**Target Condition**: {}", self.target_condition));
        }
        if !self.time_horizon.trim().is_empty() {
            level_lines.push(format!("**Time Horizon**: {}", self.time_horizon));
        }
        if !self.position_sizing.trim().is_empty() {
            level_lines.push(format!("**Position Sizing**: {}", self.position_sizing));
        }
        if !level_lines.is_empty() {
            parts.push(String::new());
            parts.push("## Trade Levels".to_string());
            parts.extend(level_lines);
        }
        parts.extend([
            String::new(),
            "## Final Transaction Proposal".to_string(),
            format!("**Action**: {}", self.action),
        ]);
        parts.join("\n")
    }
}
