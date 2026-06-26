use std::sync::{Arc, RwLock};

use super::schema::*;

/// Thread-safe collector for analysis data set by LLM tool calls.
///
/// Uses `Arc<RwLock<>>` to allow concurrent access from multiple analysis tasks.
/// Each analysis session creates its own collector, which can be shared across
/// threads if needed.
#[derive(Clone, Debug)]
pub struct AnalysisDataCollector {
    inner: Arc<RwLock<AnalysisDataInner>>,
}

#[derive(Clone, Debug, Default)]
struct AnalysisDataInner {
    // === Rating & Confidence ===
    pub rating: Option<String>,
    pub confidence: Option<f64>,
    pub action: Option<String>,

    // === Price Levels ===
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub target_price: Option<f64>,
    pub confirmation_level: Option<f64>,
    pub invalidation_level: Option<f64>,
    pub risk_reward_ratio: Option<f64>,

    // === Text Fields ===
    pub executive_summary: Option<String>,
    pub investment_thesis: Option<String>,
    pub rationale: Option<String>,
    pub risk_assessment: Option<String>,
    pub summary: Option<String>,
    pub detail: Option<String>,
    pub strategic_actions: Option<String>,
    pub trader_plan: Option<String>,

    // === Evidence & Lists ===
    pub evidence_points: Vec<String>,
    pub key_risks: Vec<String>,
    pub trigger_checklist: Vec<String>,
    pub next_steps: Vec<String>,
    pub blocking_gaps: Vec<String>,
    pub tolerable_gaps: Vec<String>,
    pub manageable_gaps: Vec<String>,
    pub key_numbers: Vec<String>,
    pub references: Vec<String>,

    // === Probability ===
    pub up_probability: Option<f64>,
    pub down_probability: Option<f64>,
    pub sideways_probability: Option<f64>,

    // === Scores ===
    pub scores: std::collections::HashMap<String, f64>,

    // === Scenario Paths ===
    pub scenario_paths: Vec<ScenarioPathData>,

    // === Meta ===
    pub time_horizon: Option<String>,
    pub position_sizing: Option<String>,
    pub time_stop: Option<TimeStopData>,
    pub reflection: Option<ReflectionData>,
    pub accounting_scope_hypothesis: Option<String>,

    // === Debate Fields ===
    pub speaker: Option<String>,
    pub stance: Option<String>,
    pub response: Option<String>,
}

impl Default for AnalysisDataCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisDataCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AnalysisDataInner::default())),
        }
    }

    // === Rating & Confidence ===

    pub fn set_rating(&self, rating: impl Into<String>) {
        self.inner.write().expect("lock").rating = Some(rating.into());
    }

    pub fn set_confidence(&self, confidence: f64) {
        self.inner.write().expect("lock").confidence = Some(confidence.clamp(0.0, 100.0));
    }

    pub fn set_action(&self, action: impl Into<String>) {
        self.inner.write().expect("lock").action = Some(action.into());
    }

    // === Price Levels ===

    pub fn set_entry_price(&self, price: f64) {
        self.inner.write().expect("lock").entry_price = Some(price);
    }

    pub fn set_stop_loss(&self, price: f64) {
        self.inner.write().expect("lock").stop_loss = Some(price);
    }

    pub fn set_target_price(&self, price: f64) {
        self.inner.write().expect("lock").target_price = Some(price);
    }

    pub fn set_confirmation_level(&self, price: f64) {
        self.inner.write().expect("lock").confirmation_level = Some(price);
    }

    pub fn set_invalidation_level(&self, price: f64) {
        self.inner.write().expect("lock").invalidation_level = Some(price);
    }

    pub fn set_risk_reward_ratio(&self, ratio: f64) {
        self.inner.write().expect("lock").risk_reward_ratio = Some(ratio);
    }

    // === Text Fields ===

    pub fn set_executive_summary(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").executive_summary = Some(value.into());
    }

    pub fn set_investment_thesis(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").investment_thesis = Some(value.into());
    }

    pub fn set_rationale(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").rationale = Some(value.into());
    }

    pub fn set_risk_assessment(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").risk_assessment = Some(value.into());
    }

    pub fn set_summary(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").summary = Some(value.into());
    }

    pub fn set_detail(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").detail = Some(value.into());
    }

    pub fn set_strategic_actions(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").strategic_actions = Some(value.into());
    }

    pub fn set_trader_plan(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").trader_plan = Some(value.into());
    }

    // === Evidence & Lists ===

    pub fn add_evidence_point(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").evidence_points.push(value.into());
    }

    pub fn add_key_risk(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").key_risks.push(value.into());
    }

    pub fn add_trigger(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").trigger_checklist.push(value.into());
    }

    pub fn add_next_step(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").next_steps.push(value.into());
    }

    pub fn add_blocking_gap(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").blocking_gaps.push(value.into());
    }

    pub fn add_tolerable_gap(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").tolerable_gaps.push(value.into());
    }

    pub fn add_manageable_gap(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").manageable_gaps.push(value.into());
    }

    pub fn add_key_number(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").key_numbers.push(value.into());
    }

    pub fn add_reference(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").references.push(value.into());
    }

    // === Probability ===

    pub fn set_probability(&self, up: f64, down: f64, sideways: f64) {
        let total = up + down + sideways;
        if total > 0.0 {
            let mut inner = self.inner.write().expect("lock");
            inner.up_probability = Some(up / total);
            inner.down_probability = Some(down / total);
            inner.sideways_probability = Some(sideways / total);
        }
    }

    // === Scores ===

    pub fn set_score(&self, dimension: impl Into<String>, score: f64) {
        self.inner.write().expect("lock").scores.insert(dimension.into(), score);
    }

    // === Scenario Paths ===

    pub fn add_scenario_path(&self, path: ScenarioPathData) {
        self.inner.write().expect("lock").scenario_paths.push(path);
    }

    // === Meta ===

    pub fn set_time_horizon(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").time_horizon = Some(value.into());
    }

    pub fn set_position_sizing(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").position_sizing = Some(value.into());
    }

    pub fn set_time_stop(&self, deadline: impl Into<String>, reason: impl Into<String>) {
        self.inner.write().expect("lock").time_stop = Some(TimeStopData {
            deadline: deadline.into(),
            reason: reason.into(),
        });
    }

    pub fn set_reflection(&self, data: ReflectionData) {
        self.inner.write().expect("lock").reflection = Some(data);
    }

    pub fn set_accounting_scope_hypothesis(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").accounting_scope_hypothesis = Some(value.into());
    }

    // === Debate Fields ===

    pub fn set_speaker(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").speaker = Some(value.into());
    }

    pub fn set_stance(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").stance = Some(value.into());
    }

    pub fn set_response(&self, value: impl Into<String>) {
        self.inner.write().expect("lock").response = Some(value.into());
    }

    // === Build & Snapshot ===

    /// Build the final analysis data from collected tool calls.
    pub fn build(&self) -> AnalysisData {
        self.snapshot()
    }

    /// Get a snapshot of current data.
    pub fn snapshot(&self) -> AnalysisData {
        let inner = self.inner.read().expect("lock");
        AnalysisData {
            rating: inner.rating.clone().unwrap_or_else(|| "Hold".to_string()),
            confidence: inner.confidence.unwrap_or(50.0),
            action: inner.action.clone(),
            entry_price: inner.entry_price,
            stop_loss: inner.stop_loss,
            target_price: inner.target_price,
            confirmation_level: inner.confirmation_level,
            invalidation_level: inner.invalidation_level,
            risk_reward_ratio: inner.risk_reward_ratio,
            executive_summary: inner.executive_summary.clone().unwrap_or_default(),
            investment_thesis: inner.investment_thesis.clone().unwrap_or_default(),
            rationale: inner.rationale.clone().unwrap_or_default(),
            risk_assessment: inner.risk_assessment.clone().unwrap_or_default(),
            summary: inner.summary.clone().unwrap_or_default(),
            detail: inner.detail.clone().unwrap_or_default(),
            strategic_actions: inner.strategic_actions.clone().unwrap_or_default(),
            trader_plan: inner.trader_plan.clone().unwrap_or_default(),
            evidence_points: inner.evidence_points.clone(),
            key_risks: inner.key_risks.clone(),
            trigger_checklist: inner.trigger_checklist.clone(),
            next_steps: inner.next_steps.clone(),
            blocking_gaps: inner.blocking_gaps.clone(),
            tolerable_gaps: inner.tolerable_gaps.clone(),
            manageable_gaps: inner.manageable_gaps.clone(),
            key_numbers: inner.key_numbers.clone(),
            references: inner.references.clone(),
            up_probability: inner.up_probability.unwrap_or(0.33),
            down_probability: inner.down_probability.unwrap_or(0.33),
            sideways_probability: inner.sideways_probability.unwrap_or(0.34),
            scores: inner.scores.clone(),
            scenario_paths: inner.scenario_paths.clone(),
            time_horizon: inner.time_horizon.clone(),
            position_sizing: inner.position_sizing.clone(),
            time_stop: inner.time_stop.clone(),
            reflection: inner.reflection.clone(),
            accounting_scope_hypothesis: inner.accounting_scope_hypothesis.clone(),
            speaker: inner.speaker.clone(),
            stance: inner.stance.clone(),
            response: inner.response.clone(),
        }
    }
}

/// Final analysis data collected from tool calls.
#[derive(Clone, Debug, Default)]
pub struct AnalysisData {
    pub rating: String,
    pub confidence: f64,
    pub action: Option<String>,
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub target_price: Option<f64>,
    pub confirmation_level: Option<f64>,
    pub invalidation_level: Option<f64>,
    pub risk_reward_ratio: Option<f64>,
    pub executive_summary: String,
    pub investment_thesis: String,
    pub rationale: String,
    pub risk_assessment: String,
    pub summary: String,
    pub detail: String,
    pub strategic_actions: String,
    pub trader_plan: String,
    pub evidence_points: Vec<String>,
    pub key_risks: Vec<String>,
    pub trigger_checklist: Vec<String>,
    pub next_steps: Vec<String>,
    pub blocking_gaps: Vec<String>,
    pub tolerable_gaps: Vec<String>,
    pub manageable_gaps: Vec<String>,
    pub key_numbers: Vec<String>,
    pub references: Vec<String>,
    pub up_probability: f64,
    pub down_probability: f64,
    pub sideways_probability: f64,
    pub scores: std::collections::HashMap<String, f64>,
    pub scenario_paths: Vec<ScenarioPathData>,
    pub time_horizon: Option<String>,
    pub position_sizing: Option<String>,
    pub time_stop: Option<TimeStopData>,
    pub reflection: Option<ReflectionData>,
    pub accounting_scope_hypothesis: Option<String>,
    pub speaker: Option<String>,
    pub stance: Option<String>,
    pub response: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_full_workflow() {
        let c = AnalysisDataCollector::new();

        // Rating
        c.set_rating("Buy");
        c.set_confidence(75.0);
        c.set_action("accumulate");

        // Prices
        c.set_entry_price(100.0);
        c.set_stop_loss(95.0);
        c.set_target_price(120.0);
        c.set_confirmation_level(105.0);
        c.set_invalidation_level(92.0);

        // Text
        c.set_executive_summary("Buy signal with strong fundamentals");
        c.set_rationale("Technical breakout confirmed by volume");

        // Evidence
        c.add_evidence_point("Revenue growth 20% YoY");
        c.add_key_risk("Market downturn risk");
        c.add_trigger("Close above 105 with volume");

        // Probability
        c.set_probability(0.5, 0.3, 0.2);

        // Scenario
        c.add_scenario_path(ScenarioPathData {
            key: "breakout".to_string(),
            name: "Breakout above resistance".to_string(),
            trigger: "Close above 105".to_string(),
            action: "buy".to_string(),
            ..Default::default()
        });

        let data = c.build();
        assert_eq!(data.rating, "Buy");
        assert_eq!(data.confidence, 75.0);
        assert_eq!(data.entry_price, Some(100.0));
        assert_eq!(data.evidence_points.len(), 1);
        assert_eq!(data.scenario_paths.len(), 1);
    }

    #[test]
    fn test_collector_thread_safety() {
        let collector = AnalysisDataCollector::new();
        let mut handles = vec![];

        for i in 0..10 {
            let c = collector.clone();
            handles.push(std::thread::spawn(move || {
                c.set_confidence(i as f64 * 10.0);
                c.add_evidence_point(format!("evidence {i}"));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let data = collector.snapshot();
        assert!((0.0..=90.0).contains(&data.confidence));
        assert_eq!(data.evidence_points.len(), 10);
    }
}
