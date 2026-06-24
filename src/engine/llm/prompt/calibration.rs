use anyhow::Context;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_memo_no_history() {
        let ctx = MemoryContextSnapshot::default();
        let memo = LlmClient::calibration_memo(&ctx, "us_equity", "2026-06-23");
        assert!(memo.contains("us_equity"));
        assert!(memo.contains("2026-06-23"));
        assert!(memo.contains("no historical calibration data"));
        assert!(memo.contains("disabled"));
    }

    #[test]
    fn calibration_memo_with_history() {
        let mut ctx = MemoryContextSnapshot::default();
        ctx.used_setup_filtered_retrieval = true;
        ctx.setup_match_count = 10;
        ctx.setup_resolved_match_count = 5;
        ctx.setup_match_hit_rate = 0.6;
        ctx.setup_match_avg_alpha_return = 0.05;
        ctx.setup_long_match_count = 3;
        ctx.setup_short_match_count = 2;
        ctx.setup_neutral_match_count = 0;
        ctx.setup_tags = vec!["trend_confirmed".to_string(), "event_driven".to_string()];
        let memo = LlmClient::calibration_memo(&ctx, "a_share", "2026-06-23");
        assert!(memo.contains("enabled"));
        assert!(memo.contains("trend_confirmed, event_driven"));
        assert!(memo.contains("total=10"));
        assert!(memo.contains("resolved=5"));
        assert!(memo.contains("hit_rate=60%"));
        assert!(memo.contains("skew bullish"));
    }

    #[test]
    fn calibration_memo_bearish() {
        let mut ctx = MemoryContextSnapshot::default();
        ctx.setup_resolved_match_count = 5;
        ctx.setup_long_match_count = 1;
        ctx.setup_short_match_count = 4;
        let memo = LlmClient::calibration_memo(&ctx, "us_equity", "2026-06-23");
        assert!(memo.contains("skew bearish"));
    }

    #[test]
    fn calibration_memo_mixed() {
        let mut ctx = MemoryContextSnapshot::default();
        ctx.setup_resolved_match_count = 5;
        ctx.setup_long_match_count = 3;
        ctx.setup_short_match_count = 3;
        let memo = LlmClient::calibration_memo(&ctx, "us_equity", "2026-06-23");
        assert!(memo.contains("mixed or neutral"));
    }
}
