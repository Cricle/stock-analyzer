impl LlmClient {
    pub fn calibration_memo(
        memory_context: &crate::models::MemoryContextSnapshot,
        market_type: &str,
        analysis_date: &str,
    ) -> String {
        let direction_bias = if memory_context.setup_resolved_match_count == 0 {
            "no historical calibration data available; current evidence drives the recommendation"
        } else if memory_context.setup_long_match_count > memory_context.setup_short_match_count {
            "historical resolved setups skew bullish"
        } else if memory_context.setup_short_match_count > memory_context.setup_long_match_count {
            "historical resolved setups skew bearish"
        } else {
            "historical resolved setups are mixed or neutral"
        };
        let setup_filter = if memory_context.used_setup_filtered_retrieval {
            "enabled"
        } else {
            "disabled"
        };
        format!(
            "Market: {market_type}. Analysis date: {analysis_date}. \
Setup-filtered retrieval: {setup_filter}. \
Setup tags: {}. \
Matched setups: total={}, resolved={}, hit_rate={:.0}%, avg_alpha={:.1}%. \
Resolved setup direction mix: bullish={}, bearish={}, neutral={}. \
Direction bias summary: {direction_bias}. \
If historical setup support is thin, weak, or directionally misaligned, avoid aggressive directional upgrades and explicitly require stronger confirmation before Buy/Sell style actions. When no historical calibration data is available (resolved=0), base the recommendation entirely on current market evidence, technicals, fundamentals, and news. Lack of history does NOT mean the stock should be held -- it means the decision must be grounded in fresh evidence alone.",
            if memory_context.setup_tags.is_empty() {
                "none".to_string()
            } else {
                memory_context.setup_tags.join(", ")
            },
            memory_context.setup_match_count,
            memory_context.setup_resolved_match_count,
            memory_context.setup_match_hit_rate * 100.0,
            memory_context.setup_match_avg_alpha_return * 100.0,
            memory_context.setup_long_match_count,
            memory_context.setup_short_match_count,
            memory_context.setup_neutral_match_count,
        )
    }
}
