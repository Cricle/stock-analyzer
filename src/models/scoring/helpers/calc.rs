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

#[cfg(test)]
mod calc_tests {
    use super::*;

    // --- numeric_tokens ---

    #[test]
    fn numeric_tokens_simple() {
        let tokens = numeric_tokens("price is 123.45");
        assert_eq!(tokens, vec!["123.45"]);
    }

    #[test]
    fn numeric_tokens_negative() {
        let tokens = numeric_tokens("drop of -5.2%");
        assert_eq!(tokens, vec!["-5.2"]);
    }

    #[test]
    fn numeric_tokens_multiple() {
        let tokens = numeric_tokens("entry 100 stop 95 target 110");
        assert_eq!(tokens, vec!["100", "95", "110"]);
    }

    #[test]
    fn numeric_tokens_empty() {
        let tokens = numeric_tokens("no numbers here");
        assert!(tokens.is_empty());
    }

    #[test]
    fn numeric_tokens_only_dot() {
        let tokens = numeric_tokens("just a dot . here");
        assert!(tokens.is_empty());
    }

    #[test]
    fn numeric_tokens_leading_dot_not_supported() {
        // Parser requires a digit before the dot
        let tokens = numeric_tokens("value .5 percent");
        assert_eq!(tokens, vec!["5"]);
    }

    // --- count_numeric_levels ---

    #[test]
    fn count_numeric_levels_2_to_5_digits() {
        assert_eq!(count_numeric_levels("price at 1234"), 1);
        assert_eq!(count_numeric_levels("12 and 12345"), 2);
    }

    #[test]
    fn count_numeric_levels_too_short_or_long() {
        assert_eq!(count_numeric_levels("1 and 123456"), 0);
    }

    #[test]
    fn count_numeric_levels_mixed() {
        assert_eq!(count_numeric_levels("entry 100.50 stop 95"), 2);
    }

    #[test]
    fn count_numeric_levels_empty() {
        assert_eq!(count_numeric_levels(""), 0);
    }

    // --- count_numeric_dates ---

    #[test]
    fn count_numeric_dates_ymd() {
        assert_eq!(count_numeric_dates("report from 2026-06-21"), 1);
        assert_eq!(count_numeric_dates("2026-01-01 to 2026-12-31"), 2);
    }

    #[test]
    fn count_numeric_dates_slash() {
        assert_eq!(count_numeric_dates("date 2026/06/21"), 1);
    }

    #[test]
    fn count_numeric_dates_none() {
        assert_eq!(count_numeric_dates("no dates here"), 0);
    }

    #[test]
    fn count_numeric_dates_short_year() {
        assert_eq!(count_numeric_dates("26-06-21"), 0);
    }

    // --- parse_first_number ---

    #[test]
    fn parse_first_number_basic() {
        assert_eq!(parse_first_number("price is 123.45"), Some(123.45));
    }

    #[test]
    fn parse_first_number_negative() {
        assert_eq!(parse_first_number("drop -5.2"), Some(-5.2));
    }

    #[test]
    fn parse_first_number_none() {
        assert_eq!(parse_first_number("no numbers"), None);
    }

    #[test]
    fn parse_first_number_first_wins() {
        assert_eq!(parse_first_number("100 and 200"), Some(100.0));
    }

    // --- parse_position_percentage ---

    #[test]
    fn parse_position_percentage_with_percent() {
        assert_eq!(parse_position_percentage("20%"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_decimal() {
        assert_eq!(parse_position_percentage("0.2"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_whole_number() {
        assert_eq!(parse_position_percentage("20"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_out_of_range() {
        assert_eq!(parse_position_percentage("150"), None);
    }

    #[test]
    fn parse_position_percentage_empty() {
        assert_eq!(parse_position_percentage(""), None);
    }

    // --- looks_like_ymd_date ---

    #[test]
    fn looks_like_ymd_valid_dash() {
        assert!(looks_like_ymd_date("2026-06-21"));
    }

    #[test]
    fn looks_like_ymd_valid_slash() {
        assert!(looks_like_ymd_date("2026/6/1"));
    }

    #[test]
    fn looks_like_ymd_invalid_short_year() {
        assert!(!looks_like_ymd_date("26-06-21"));
    }

    #[test]
    fn looks_like_ymd_invalid_two_parts() {
        assert!(!looks_like_ymd_date("2026-06"));
    }

    #[test]
    fn looks_like_ymd_invalid_no_separator() {
        assert!(!looks_like_ymd_date("hello"));
    }

    // --- normalized_key ---

    #[test]
    fn normalized_key_lowercases() {
        assert_eq!(normalized_key("Hello World"), "helloworld");
    }

    #[test]
    fn normalized_key_preserves_cjk() {
        assert_eq!(normalized_key("市场分析"), "市场分析");
    }

    #[test]
    fn normalized_key_strips_special() {
        assert_eq!(normalized_key("hello-world!"), "helloworld");
    }

    // --- is_cjk ---

    #[test]
    fn is_cjk_basic() {
        assert!(is_cjk('中'));
        assert!(is_cjk('文'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('1'));
    }

    // --- analyst_matches ---

    #[test]
    fn analyst_matches_by_key() {
        let node = AgentReportNode {
            key: "market_analysis".into(),
            title: "Market Report".into(),
            agent: "market".into(),
            ..Default::default()
        };
        assert!(analyst_matches(&node, &["market"]));
    }

    #[test]
    fn analyst_matches_by_chinese_title() {
        let node = AgentReportNode {
            key: "".into(),
            title: "NVDA 基本面分析".into(),
            agent: "".into(),
            ..Default::default()
        };
        assert!(analyst_matches(&node, &["fundamentals"]));
    }

    #[test]
    fn analyst_matches_no_match() {
        let node = AgentReportNode {
            key: "market".into(),
            title: "Market".into(),
            agent: "market".into(),
            ..Default::default()
        };
        assert!(!analyst_matches(&node, &["fundamentals"]));
    }

    #[test]
    fn analyst_matches_empty_candidate_ignored() {
        let node = AgentReportNode {
            key: "market".into(),
            ..Default::default()
        };
        assert!(!analyst_matches(&node, &[""]));
    }

    // --- matches_semantic_alias ---

    #[test]
    fn matches_semantic_alias_market() {
        assert!(matches_semantic_alias("market", "", "市场分析", ""));
        assert!(matches_semantic_alias("market", "", "技术面报告", ""));
    }

    #[test]
    fn matches_semantic_alias_fundamentals() {
        assert!(matches_semantic_alias("fundamentals", "", "基本面分析", ""));
        assert!(matches_semantic_alias("fundamental", "", "基本面", ""));
    }

    #[test]
    fn matches_semantic_alias_news() {
        assert!(matches_semantic_alias("news", "", "新闻催化", ""));
    }

    #[test]
    fn matches_semantic_alias_sentiment() {
        assert!(matches_semantic_alias("sentiment", "", "情绪分析", ""));
        assert!(matches_semantic_alias("sentiment", "", "资金面", ""));
    }

    #[test]
    fn matches_semantic_alias_unknown() {
        assert!(!matches_semantic_alias("unknown", "", "市场", ""));
    }

    // --- analyst_probability_quality ---

    #[test]
    fn analyst_probability_quality_none() {
        assert_eq!(analyst_probability_quality(None), 0);
    }

    #[test]
    fn analyst_probability_quality_perfect() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.2,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 4);
    }

    #[test]
    fn analyst_probability_quality_off_by_010() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.3,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 2);
    }

    #[test]
    fn analyst_probability_quality_off_by_more() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.5,
            sideways_probability: 0.5,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 0);
    }

    // --- analyst_net_probability ---

    #[test]
    fn analyst_net_probability_bullish() {
        let node = AgentReportNode {
            up_probability: 0.7,
            down_probability: 0.2,
            ..Default::default()
        };
        assert!((analyst_net_probability(&node) - 0.5).abs() < 0.01);
    }

    #[test]
    fn analyst_net_probability_clamped() {
        let node = AgentReportNode {
            up_probability: 0.0,
            down_probability: 1.5,
            ..Default::default()
        };
        assert!((analyst_net_probability(&node) - (-1.0)).abs() < 0.01);
    }

    // --- score_analyst_net ---

    #[test]
    fn score_analyst_net_none() {
        assert_eq!(score_analyst_net(None, 20), 0);
    }

    #[test]
    fn score_analyst_net_bullish() {
        let node = AgentReportNode {
            up_probability: 0.7,
            down_probability: 0.2,
            ..Default::default()
        };
        let score = score_analyst_net(Some(&node), 20);
        assert!(score > 0, "expected positive, got {}", score);
    }

    // --- rating_bias ---

    #[test]
    fn rating_bias_buy() {
        assert_eq!(rating_bias(&Rating::Buy, 10), 10);
    }

    #[test]
    fn rating_bias_sell() {
        assert_eq!(rating_bias(&Rating::Sell, 10), -10);
    }

    #[test]
    fn rating_bias_hold() {
        assert_eq!(rating_bias(&Rating::Hold, 10), 0);
    }

    #[test]
    fn rating_bias_overweight() {
        assert_eq!(rating_bias(&Rating::Overweight, 10), 7);
    }

    #[test]
    fn rating_bias_underweight() {
        assert_eq!(rating_bias(&Rating::Underweight, 10), -7);
    }

    // --- map_direction_score_to_rating ---

    #[test]
    fn map_direction_score_to_rating_buy() {
        assert_eq!(map_direction_score_to_rating(80), Rating::Buy);
    }

    #[test]
    fn map_direction_score_to_rating_hold() {
        assert_eq!(map_direction_score_to_rating(0), Rating::Hold);
    }

    #[test]
    fn map_direction_score_to_rating_sell() {
        assert_eq!(map_direction_score_to_rating(-80), Rating::Sell);
    }

    // --- direction_score_to_evidence_score ---

    #[test]
    fn direction_score_to_evidence_score_buy_range() {
        assert_eq!(direction_score_to_evidence_score(80), 2);
    }

    #[test]
    fn direction_score_to_evidence_score_hold_range() {
        assert_eq!(direction_score_to_evidence_score(0), 0);
    }

    #[test]
    fn direction_score_to_evidence_score_sell_range() {
        assert_eq!(direction_score_to_evidence_score(-80), -2);
    }

    // --- score_to_rating ---

    #[test]
    fn score_to_rating_all_variants() {
        assert_eq!(score_to_rating(2), Rating::Buy);
        assert_eq!(score_to_rating(1), Rating::Overweight);
        assert_eq!(score_to_rating(0), Rating::Hold);
        assert_eq!(score_to_rating(-1), Rating::Underweight);
        assert_eq!(score_to_rating(-2), Rating::Sell);
    }

    // --- has_execution_boundary ---

    #[test]
    fn has_execution_boundary_complete() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_with_confirmation() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "".into(),
            confirmation_level: "108".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_missing_stop() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(!has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_missing_horizon() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "".into(),
            ..Default::default()
        };
        assert!(!has_execution_boundary(&trader, &portfolio));
    }

    // --- bool_text ---

    #[test]
    fn bool_text_true_false() {
        assert_eq!(bool_text(true), "是");
        assert_eq!(bool_text(false), "否");
    }

    // --- NumericFieldExt ---

    #[test]
    fn numeric_field_ext_count() {
        assert_eq!("entry 100 stop 95".numeric_count(), 2);
        assert_eq!("no numbers".numeric_count(), 0);
    }

    // --- average_evidence_density ---

    #[test]
    fn average_evidence_density_empty() {
        assert_eq!(average_evidence_density(&[]), 0.0);
    }

    #[test]
    fn average_evidence_density_nonempty() {
        let analysts = vec![
            AgentReportNode {
                evidence_points: vec!["a".into(), "b".into()],
                ..Default::default()
            },
            AgentReportNode {
                evidence_points: vec!["c".into()],
                ..Default::default()
            },
        ];
        assert!((average_evidence_density(&analysts) - 1.5).abs() < 0.01);
    }
}
