use stock_analyzer::scoring::dimensions::llm_analysis::{LlmAnalysisInput, score_llm_analysis};

fn base_input() -> LlmAnalysisInput {
    LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 75.0,
        momentum_score: 60.0,
        hit_rate: Some(0.6),
        catalyst_count: 3,
        hard_negative_count: 0,
        volume_ratio: Some(1.2),
        period_return_pct: Some(5.0),
    }
}

#[test]
fn test_all_signals_agree() {
    // All signals around 65-70
    let input = LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 70.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 6,
        hard_negative_count: 0,
        volume_ratio: Some(1.2),
        period_return_pct: Some(3.0),
    };
    let result = score_llm_analysis(&input);
    // High consensus, score should be close to average
    assert!(
        result.score >= 55,
        "expected high score with consensus, got {}",
        result.score
    );
    assert!(
        result.reason.contains("共识度"),
        "expected consensus in reason"
    );
}

#[test]
fn test_signals_disagree() {
    // LLM says high, tech says low
    let input = LlmAnalysisInput {
        confidence: 90.0,
        objective_final_score: 85.0,
        momentum_score: 20.0,
        hit_rate: Some(0.2),
        catalyst_count: 0,
        hard_negative_count: 3,
        volume_ratio: Some(0.3),
        period_return_pct: Some(-10.0),
    };
    let result = score_llm_analysis(&input);
    // Big spread = low consensus = penalty
    assert!(
        result.score <= 45,
        "expected low score with disagreement, got {}",
        result.score
    );
}

#[test]
fn test_no_history_neutral() {
    let input = LlmAnalysisInput {
        confidence: 60.0,
        objective_final_score: 60.0,
        momentum_score: 60.0,
        hit_rate: None,
        catalyst_count: 5,
        hard_negative_count: 0,
        volume_ratio: Some(1.0),
        period_return_pct: Some(2.0),
    };
    let result = score_llm_analysis(&input);
    // Should not crash, history defaults to 50
    assert!(
        result.score > 0,
        "expected non-zero score, got {}",
        result.score
    );
}

#[test]
fn test_hard_negatives_reduce_score() {
    let with_neg = LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 70.0,
        momentum_score: 60.0,
        hit_rate: Some(0.6),
        catalyst_count: 3,
        hard_negative_count: 3,
        volume_ratio: Some(1.0),
        period_return_pct: Some(2.0),
    };
    let without_neg = LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 70.0,
        momentum_score: 60.0,
        hit_rate: Some(0.6),
        catalyst_count: 3,
        hard_negative_count: 0,
        volume_ratio: Some(1.0),
        period_return_pct: Some(2.0),
    };
    let r1 = score_llm_analysis(&with_neg);
    let r2 = score_llm_analysis(&without_neg);
    assert!(
        r1.score < r2.score,
        "hard negatives should reduce score: {} vs {}",
        r1.score,
        r2.score
    );
}

#[test]
fn test_high_volume_bullish() {
    let input = LlmAnalysisInput {
        confidence: 60.0,
        objective_final_score: 60.0,
        momentum_score: 60.0,
        hit_rate: Some(0.6),
        catalyst_count: 5,
        hard_negative_count: 0,
        volume_ratio: Some(2.0),
        period_return_pct: Some(8.0),
    };
    let result = score_llm_analysis(&input);
    assert!(
        result.score >= 55,
        "expected decent score, got {}",
        result.score
    );
}

#[test]
fn test_reason_format() {
    let input = base_input();
    let result = score_llm_analysis(&input);
    assert!(result.reason.contains("LLM:"), "expected LLM in reason");
    assert!(result.reason.contains("技术:"), "expected tech in reason");
    assert!(
        result.reason.contains("历史:"),
        "expected history in reason"
    );
    assert!(result.reason.contains("新闻:"), "expected news in reason");
    assert!(result.reason.contains("市场:"), "expected market in reason");
}

#[test]
fn test_all_signals_high() {
    let input = LlmAnalysisInput {
        confidence: 95.0,
        objective_final_score: 90.0,
        momentum_score: 85.0,
        hit_rate: Some(0.9),
        catalyst_count: 8,
        hard_negative_count: 0,
        volume_ratio: Some(2.0),
        period_return_pct: Some(15.0),
    };
    let result = score_llm_analysis(&input);
    assert!(
        result.score >= 70,
        "expected high score when all signals high, got {}",
        result.score
    );
}

#[test]
fn test_all_signals_low() {
    let input = LlmAnalysisInput {
        confidence: 10.0,
        objective_final_score: 15.0,
        momentum_score: 20.0,
        hit_rate: Some(0.1),
        catalyst_count: 0,
        hard_negative_count: 4,
        volume_ratio: Some(0.3),
        period_return_pct: Some(-15.0),
    };
    let result = score_llm_analysis(&input);
    assert!(
        result.score <= 20,
        "expected low score when all signals low, got {}",
        result.score
    );
}

#[test]
fn test_one_extreme_others_neutral() {
    // LLM very bullish, others neutral
    let input = LlmAnalysisInput {
        confidence: 95.0,
        objective_final_score: 50.0,
        momentum_score: 50.0,
        hit_rate: Some(0.5),
        catalyst_count: 5,
        hard_negative_count: 0,
        volume_ratio: Some(1.0),
        period_return_pct: Some(0.0),
    };
    let result = score_llm_analysis(&input);
    // Spread is large, consensus should penalize
    assert!(
        result.score < 70,
        "extreme signal with neutral others should be penalized, got {}",
        result.score
    );
}

#[test]
fn test_consensus_vs_no_consensus() {
    let consensus = LlmAnalysisInput {
        confidence: 65.0,
        objective_final_score: 65.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 6,
        hard_negative_count: 0,
        volume_ratio: Some(1.2),
        period_return_pct: Some(3.0),
    };
    let no_consensus = LlmAnalysisInput {
        confidence: 90.0,
        objective_final_score: 30.0,
        momentum_score: 80.0,
        hit_rate: Some(0.1),
        catalyst_count: 0,
        hard_negative_count: 5,
        volume_ratio: Some(2.0),
        period_return_pct: Some(-10.0),
    };
    let r1 = score_llm_analysis(&consensus);
    let r2 = score_llm_analysis(&no_consensus);
    // Even though avg might be similar, consensus should score higher
    assert!(
        r1.score > r2.score,
        "consensus should score higher: consensus={} vs no_consensus={}",
        r1.score,
        r2.score
    );
}

#[test]
fn test_score_never_exceeds_100() {
    let input = LlmAnalysisInput {
        confidence: 100.0,
        objective_final_score: 100.0,
        momentum_score: 100.0,
        hit_rate: Some(1.0),
        catalyst_count: 10,
        hard_negative_count: 0,
        volume_ratio: Some(3.0),
        period_return_pct: Some(50.0),
    };
    let result = score_llm_analysis(&input);
    assert!(
        result.score <= 100,
        "score should never exceed 100, got {}",
        result.score
    );
}

#[test]
fn test_score_never_below_0() {
    let input = LlmAnalysisInput {
        confidence: 0.0,
        objective_final_score: 0.0,
        momentum_score: 0.0,
        hit_rate: Some(0.0),
        catalyst_count: 0,
        hard_negative_count: 10,
        volume_ratio: Some(0.1),
        period_return_pct: Some(-50.0),
    };
    let result = score_llm_analysis(&input);
    let _ = result.score; // score is u8, always >= 0
}
