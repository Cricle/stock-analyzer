use crate::scoring::score_types::{DimensionScore, ScoreReliability};

/// Input for LLM analysis scoring — cross-validates multiple independent signals.
pub struct LlmAnalysisInput {
    // Signal 1: LLM judgment
    pub confidence: f64,
    pub objective_final_score: f64,
    // Signal 2: Technical
    pub momentum_score: f64,
    // Signal 3: History
    pub hit_rate: Option<f64>,
    // Signal 4: News
    pub catalyst_count: usize,
    pub hard_negative_count: usize,
    // Signal 5: Market
    pub volume_ratio: Option<f64>,
    pub period_return_pct: Option<f64>,
}

pub fn score_llm_analysis(input: &LlmAnalysisInput) -> DimensionScore {
    let signals = [
        signal_llm(input.confidence, input.objective_final_score),
        signal_technical(input.momentum_score),
        signal_history(input.hit_rate),
        signal_news(input.catalyst_count, input.hard_negative_count),
        signal_market(input.volume_ratio, input.period_return_pct),
    ];

    // Count how many signals have real data vs defaults
    let missing_count = [
        input.hit_rate.is_none(),
        input.volume_ratio.is_none(),
        input.period_return_pct.is_none(),
    ]
    .iter()
    .filter(|&&m| m)
    .count();

    let avg = signals.iter().sum::<f64>() / signals.len() as f64;
    let min = signals.iter().cloned().fold(f64::MAX, f64::min);
    let max = signals.iter().cloned().fold(f64::MIN, f64::max);
    let spread = max - min;
    let consensus = 1.0 - (spread / 100.0).clamp(0.0, 1.0);

    let raw = avg * (0.6 + 0.4 * consensus);
    let score = raw.clamp(0.0, 100.0) as u8;

    let reliability = if missing_count >= 1 {
        ScoreReliability::Low
    } else {
        ScoreReliability::High
    };

    let signal_names = ["LLM", "技术", "历史", "新闻", "市场"];
    let detail: Vec<String> = signal_names
        .iter()
        .zip(signals.iter())
        .map(|(name, val)| format!("{}:{:.0}", name, val))
        .collect();

    DimensionScore {
        score,
        reason: format!("共识度 {:.0}%，{}", consensus * 100.0, detail.join(" ")),
        reliability,
    }
}

fn signal_llm(confidence: f64, objective: f64) -> f64 {
    (confidence.clamp(0.0, 100.0) + objective.clamp(0.0, 100.0)) / 2.0
}

fn signal_technical(momentum: f64) -> f64 {
    momentum.clamp(0.0, 100.0)
}

fn signal_history(hit_rate: Option<f64>) -> f64 {
    match hit_rate {
        Some(hr) => hr.clamp(0.0, 1.0) * 100.0,
        None => 50.0, // no history = neutral
    }
}

fn signal_news(catalyst_count: usize, hard_negative_count: usize) -> f64 {
    let base = (catalyst_count.min(10) as f64 / 10.0) * 100.0;
    let penalty = (hard_negative_count.min(5) as f64) * 20.0;
    (base - penalty).clamp(0.0, 100.0)
}

fn signal_market(volume_ratio: Option<f64>, period_return: Option<f64>) -> f64 {
    let vol_score = match volume_ratio {
        Some(v) if v > 1.5 => 70.0,
        Some(v) if v > 1.0 => 55.0,
        Some(v) if v > 0.5 => 40.0,
        Some(_) => 25.0,
        None => 50.0,
    };
    let ret_score = match period_return {
        Some(r) if r > 10.0 => 80.0,
        Some(r) if r > 3.0 => 65.0,
        Some(r) if r > 0.0 => 55.0,
        Some(r) if r > -5.0 => 40.0,
        Some(_) => 20.0,
        None => 50.0,
    };
    (vol_score + ret_score) / 2.0
}
