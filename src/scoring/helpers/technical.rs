fn select_analyst<'a>(
    result: &'a AnalysisResult,
    candidates: &[&str],
) -> Option<&'a AgentReportNode> {
    result.graph.analysts.iter().find(|item| analyst_matches(item, candidates))
}

pub fn analyst_matches(item: &AgentReportNode, candidates: &[&str]) -> bool {
    let key = normalized_key(&item.key);
    let title = normalized_key(&item.title);
    let agent = normalized_key(&item.agent);

    candidates.iter().any(|candidate| {
        let candidate = normalized_key(candidate);
        if candidate.is_empty() {
            return false;
        }

        key == candidate
            || title == candidate
            || agent == candidate
            || key.contains(&candidate)
            || title.contains(&candidate)
            || agent.contains(&candidate)
            || candidate.contains(&key)
            || candidate.contains(&title)
            || candidate.contains(&agent)
            || matches_semantic_alias(&candidate, &key, &title, &agent)
    })
}

pub fn matches_semantic_alias(candidate: &str, key: &str, title: &str, agent: &str) -> bool {
    // Prefer structured key/agent matching; fall back to Chinese title matching
    // only when key and agent are empty (legacy data).
    if key == candidate || agent.contains(candidate) {
        return true;
    }
    match candidate {
        "market" => {
            title.contains("市场") || title.contains("技术")
        }
        "fundamentals" | "fundamental" => {
            title.contains("基本面")
        }
        "news" => title.contains("新闻"),
        "sentiment" => {
            title.contains("情绪") || title.contains("资金")
        }
        _ => false,
    }
}

pub fn normalized_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || is_cjk(*ch))
        .collect()
}

pub fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

pub fn average_evidence_density(analysts: &[AgentReportNode]) -> f64 {
    if analysts.is_empty() {
        return 0.0;
    }
    analysts
        .iter()
        .map(|item| item.evidence_points.len() as f64)
        .sum::<f64>()
        / analysts.len() as f64
}

pub fn has_execution_boundary(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> bool {
    let has_target = !portfolio_decision.price_target.trim().is_empty();
    let has_confirmation = !portfolio_decision.confirmation_level.trim().is_empty();
    // When trader says Hold, entry_price/stop_loss are empty by design.
    // Fall back to PM's confirmation_level (entry reference) and invalidation_level (stop reference).
    let effective_entry = if trader_plan.entry_price.trim().is_empty() {
        &portfolio_decision.confirmation_level
    } else {
        &trader_plan.entry_price
    };
    let effective_stop = if trader_plan.stop_loss.trim().is_empty() {
        &portfolio_decision.invalidation_level
    } else {
        &trader_plan.stop_loss
    };
    !effective_entry.trim().is_empty()
        && !effective_stop.trim().is_empty()
        && (has_target || has_confirmation)
        && !portfolio_decision.time_horizon.trim().is_empty()
}

pub fn analyst_probability_quality(analyst: Option<&AgentReportNode>) -> i32 {
    let Some(analyst) = analyst else {
        return 0;
    };
    let sum = analyst.up_probability + analyst.down_probability + analyst.sideways_probability;
    let gap = (sum - 1.0).abs();
    if gap <= 0.05 {
        6
    } else if gap <= 0.15 {
        4
    } else if gap <= 0.25 {
        2
    } else {
        0
    }
}

pub fn analyst_net_probability(analyst: &AgentReportNode) -> f64 {
    (analyst.up_probability - analyst.down_probability).clamp(-1.0, 1.0)
}

pub fn score_analyst_net(analyst: Option<&AgentReportNode>, max_abs: i32) -> i32 {
    analyst
        .map(|item| ((analyst_net_probability(item) * max_abs as f64).round()) as i32)
        .unwrap_or(0)
        .clamp(-max_abs, max_abs)
}

pub fn rating_bias(rating: &Rating, magnitude: i32) -> i32 {
    match rating {
        Rating::Buy => magnitude,
        Rating::Overweight => (magnitude * 3) / 4,
        Rating::Hold | Rating::Unknown => 0,
        Rating::Underweight => -((magnitude * 3) / 4),
        Rating::Sell => -magnitude,
    }
}

pub fn map_direction_score_to_rating(score: i32) -> Rating {
    match score {
        60..=100 => Rating::Buy,
        25..=59 => Rating::Overweight,
        -24..=24 => Rating::Hold,
        -59..=-25 => Rating::Underweight,
        _ => Rating::Sell,
    }
}

pub fn direction_score_to_evidence_score(score: i32) -> i32 {
    match score {
        60..=100 => 2,
        25..=59 => 1,
        -24..=24 => 0,
        -59..=-25 => -1,
        _ => -2,
    }
}

fn rating_to_score(rating: &Rating) -> i32 {
    rating.to_score()
}

pub fn score_to_rating(score: i32) -> Rating {
    match score {
        2 => Rating::Buy,
        1 => Rating::Overweight,
        0 => Rating::Hold,
        -1 => Rating::Underweight,
        _ => Rating::Sell,
    }
}

fn rating_to_action(rating: &Rating) -> &'static str {
    rating.to_action_group()
}

fn semantic_direction(rating: &Rating) -> i32 {
    rating.bias(1)
}
