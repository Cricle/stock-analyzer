fn select_analyst<'a>(
    result: &'a AnalysisResult,
    candidates: &[&str],
) -> Option<&'a AgentReportNode> {
    result.graph.analysts.iter().find(|item| analyst_matches(item, candidates))
}

fn analyst_matches(item: &AgentReportNode, candidates: &[&str]) -> bool {
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

fn matches_semantic_alias(candidate: &str, key: &str, title: &str, agent: &str) -> bool {
    // Prefer structured key/agent matching; fall back to Chinese title matching
    // only when key and agent are empty (legacy data).
    if key == candidate || agent.contains(candidate) {
        return true;
    }
    match candidate {
        "market" => {
            let t = title.to_lowercase();
            t.contains("market") || t.contains("technical") || t.contains("technical") || t.contains("market")
        }
        "fundamentals" | "fundamental" => {
            let t = title.to_lowercase();
            t.contains("fundamental") || t.contains("fundamental")
        }
        "news" => {
            let t = title.to_lowercase();
            t.contains("news") || t.contains("news")
        }
        "sentiment" => {
            let t = title.to_lowercase();
            t.contains("sentiment") || t.contains("capital flow") || t.contains("sentiment") || t.contains("capital")
        }
        _ => false,
    }
}

fn normalized_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || is_cjk(*ch))
        .collect()
}

fn is_cjk(ch: char) -> bool {
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

fn average_evidence_density(analysts: &[AgentReportNode]) -> f64 {
    if analysts.is_empty() {
        return 0.0;
    }
    analysts
        .iter()
        .map(|item| item.evidence_points.len() as f64)
        .sum::<f64>()
        / analysts.len() as f64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionBoundaryLevel {
    Complete,
    Partial,
    Missing,
}

impl ExecutionBoundaryLevel {
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
    pub fn is_at_least_partial(self) -> bool {
        self != Self::Missing
    }
}

pub fn has_execution_boundary(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ExecutionBoundaryLevel {
    let has_entry = !trader_plan.entry_price.trim().is_empty();
    let has_stop = !trader_plan.stop_loss.trim().is_empty();
    let has_target = !portfolio_decision.price_target.trim().is_empty();
    let has_confirmation = !portfolio_decision.confirmation_level.trim().is_empty();
    let has_horizon = !portfolio_decision.time_horizon.trim().is_empty();

    if has_entry && has_stop && (has_target || has_confirmation) && has_horizon {
        ExecutionBoundaryLevel::Complete
    } else if has_entry && has_stop {
        ExecutionBoundaryLevel::Partial
    } else {
        ExecutionBoundaryLevel::Missing
    }
}

fn analyst_probability_quality(analyst: Option<&AgentReportNode>) -> i32 {
    let Some(analyst) = analyst else {
        return 0;
    };
    let sum = analyst.up_probability + analyst.down_probability + analyst.sideways_probability;
    let gap = (sum - 1.0).abs();
    if gap <= 0.05 {
        4
    } else if gap <= 0.15 {
        2
    } else {
        0
    }
}

fn analyst_net_probability(analyst: &AgentReportNode) -> f64 {
    (analyst.up_probability - analyst.down_probability).clamp(-1.0, 1.0)
}

fn score_analyst_net(analyst: Option<&AgentReportNode>, max_abs: i32) -> i32 {
    analyst
        .map(|item| ((analyst_net_probability(item) * max_abs as f64).round()) as i32)
        .unwrap_or(0)
        .clamp(-max_abs, max_abs)
}

fn rating_bias(rating: &Rating, magnitude: i32) -> i32 {
    match rating {
        Rating::Buy => magnitude,
        Rating::Overweight => (magnitude * 3) / 4,
        Rating::Hold => 0,
        Rating::Underweight => -((magnitude * 3) / 4),
        Rating::Sell => -magnitude,
    }
}

fn map_direction_score_to_rating(score: i32) -> Rating {
    match score {
        60..=100 => Rating::Buy,
        20..=59 => Rating::Overweight,
        -19..=19 => Rating::Hold,
        -59..=-20 => Rating::Underweight,
        _ => Rating::Sell,
    }
}

fn direction_score_to_evidence_score(score: i32) -> i32 {
    match score {
        60..=100 => 2,
        20..=59 => 1,
        -19..=19 => 0,
        -59..=-20 => -1,
        _ => -2,
    }
}

fn rating_to_score(rating: &Rating) -> i32 {
    rating.to_score()
}

fn score_to_rating(score: i32) -> Rating {
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

fn bool_text(value: bool) -> &'static str {
    if value { "common.yes" } else { "common.no" }
}

fn count_numeric_levels(text: &str) -> i32 {
    numeric_tokens(text)
        .into_iter()
        .filter(|token| {
            let integer_digits = token
                .split_once('.')
                .map(|(left, _)| left)
                .unwrap_or(token.as_str())
                .trim_start_matches('-')
                .len();
            (2..=5).contains(&integer_digits)
        })
        .count() as i32
}

fn count_numeric_dates(text: &str) -> i32 {
    text.split_whitespace()
        .filter(|token| {
            looks_like_ymd_date(
                token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '-' && ch != '/'),
            )
        })
        .count() as i32
}

fn parse_first_number(text: &str) -> Option<f64> {
    numeric_tokens(text)
        .into_iter()
        .find_map(|token| token.parse::<f64>().ok())
}

fn parse_position_percentage(text: &str) -> Option<f64> {
    let value = parse_first_number(text)?;
    if text.chars().any(|ch| ch == '%') {
        Some((value / 100.0).clamp(0.0, 1.0))
    } else if (0.0..=1.0).contains(&value) {
        Some(value)
    } else if (1.0..=100.0).contains(&value) {
        Some(value / 100.0)
    } else {
        None
    }
}

fn numeric_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        let allowed = ch.is_ascii_digit()
            || (ch == '.' && current.chars().any(|inner| inner.is_ascii_digit()))
            || (ch == '-' && current.is_empty());
        if allowed {
            current.push(ch);
        } else if current.chars().any(|inner| inner.is_ascii_digit()) {
            tokens.push(current.clone());
            current.clear();
        } else {
            current.clear();
        }
    }
    if current.chars().any(|inner| inner.is_ascii_digit()) {
        tokens.push(current);
    }
    tokens
}

fn looks_like_ymd_date(token: &str) -> bool {
    let separator = if token.contains('-') {
        '-'
    } else if token.contains('/') {
        '/'
    } else {
        return false;
    };
    let parts = token.split(separator).collect::<Vec<_>>();
    if parts.len() != 3 {
        return false;
    }
    let year = parts[0];
    let month = parts[1];
    let day = parts[2];
    year.len() == 4
        && (1..=2).contains(&month.len())
        && (1..=2).contains(&day.len())
        && year.chars().all(|ch| ch.is_ascii_digit())
        && month.chars().all(|ch| ch.is_ascii_digit())
        && day.chars().all(|ch| ch.is_ascii_digit())
}

trait NumericFieldExt {
    fn numeric_count(&self) -> i32;
}

impl NumericFieldExt for str {
    fn numeric_count(&self) -> i32 {
        numeric_tokens(self).len() as i32
    }
}
