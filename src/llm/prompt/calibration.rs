/// Confidence calibration based on data completeness and signal consistency.
#[derive(Debug, Clone)]
pub struct ConfidenceCalibration {
    /// Base confidence score
    pub base_confidence: f64,
    /// Factor for data completeness (multiplier)
    pub data_completeness_factor: f64,
    /// Factor for signal consistency (multiplier)
    pub signal_consistency_factor: f64,
    /// Factor for historical accuracy (multiplier)
    pub historical_accuracy_factor: f64,
}

impl ConfidenceCalibration {
    /// Create a new calibration with the given base confidence.
    pub fn new(base_confidence: f64) -> Self {
        Self {
            base_confidence: base_confidence.clamp(0.0, 100.0),
            data_completeness_factor: 1.0,
            signal_consistency_factor: 1.0,
            historical_accuracy_factor: 1.0,
        }
    }

    /// Calibrate confidence based on input factors.
    pub fn calibrate(
        &self,
        data_completeness: f64,
        signal_consistency: f64,
        historical_accuracy: f64,
    ) -> f64 {
        let adjusted = self.base_confidence
            * (1.0 + (data_completeness - 0.5) * self.data_completeness_factor)
            * (1.0 + (signal_consistency - 0.5) * self.signal_consistency_factor)
            * (1.0 + (historical_accuracy - 0.5) * self.historical_accuracy_factor);
        adjusted.clamp(0.0, 100.0)
    }
}

impl Default for ConfidenceCalibration {
    fn default() -> Self {
        Self::new(50.0)
    }
}

impl LlmClient {
    /// Generate a calibration memo summarizing historical setup data for the LLM prompt.
    pub fn calibration_memo(
        memory_context: &crate::MemoryContextSnapshot,
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
