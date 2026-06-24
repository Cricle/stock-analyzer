use chrono::NaiveDate;
use regex::Regex;
use reqwest::Url;

fn sanitize_scenario_paths_for_no_attack(action_guides: &mut ReportActionGuides) {
    for guide in [&mut action_guides.buyers, &mut action_guides.watchers] {
        for path in &mut guide.scenario_paths {
            path.position_sizing = LocalText::default();
            path.sizing_blocked = true;
        }
    }
}

fn compute_reward_risk_hint(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Option<f64> {
    let entry = extract_first_price(&trader_plan.entry_price)?;
    let stop = extract_first_price(&trader_plan.stop_loss)?;
    let target = extract_first_price(&portfolio_decision.price_target)
        .or_else(|| extract_first_price(&portfolio_decision.confirmation_level))?;
    if entry > stop && target > entry {
        Some((target - entry) / (entry - stop))
    } else {
        None
    }
}

fn extract_first_price(text: &str) -> Option<f64> {
    let Ok(re) = Regex::new(r"(\d{1,6}(?:\.\d{1,4})?)") else {
        return None;
    };
    re.captures_iter(text)
        .filter_map(|caps| {
            let m = caps.get(1)?;
            let start = m.start();
            if start > 0 {
                let prev = text.as_bytes()[start - 1] as char;
                if prev.is_ascii_alphabetic() || prev == '%' {
                    return None;
                }
            }
            // Skip numbers followed by period/MA indicator characters
            // (e.g. "200日均线" where 200 is a period, not a price)
            let end = m.end();
            if end < text.len() {
                let next = text[end..].chars().next().unwrap_or('\0');
                if matches!(next, '日' | '天' | '周' | '月' | '年' | '均' | '线') {
                    return None;
                }
            }
            m.as_str().parse::<f64>().ok()
        })
        .find(|v| v.is_finite() && *v > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_first_price ---

    #[test]
    fn extract_price_simple() {
        assert_eq!(extract_first_price("100"), Some(100.0));
    }

    #[test]
    fn extract_price_decimal() {
        assert_eq!(extract_first_price("105.50"), Some(105.5));
    }

    #[test]
    fn extract_price_with_text() {
        assert_eq!(extract_first_price("目标价120元"), Some(120.0));
    }

    #[test]
    fn extract_price_skip_ma_period() {
        assert_eq!(extract_first_price("200日均线"), None);
    }

    #[test]
    fn extract_price_skip_week_period() {
        assert_eq!(extract_first_price("52周高点"), None);
    }

    #[test]
    fn extract_price_skip_alpha_prefix() {
        assert_eq!(extract_first_price("x100"), None);
    }

    #[test]
    fn extract_price_skip_percent() {
        assert_eq!(extract_first_price("%50"), None);
    }

    #[test]
    fn extract_price_empty() {
        assert_eq!(extract_first_price(""), None);
    }

    #[test]
    fn extract_price_no_number() {
        assert_eq!(extract_first_price("no numbers here"), None);
    }

    #[test]
    fn extract_price_multiple_numbers() {
        assert_eq!(extract_first_price("从100涨到120"), Some(100.0));
    }

    // --- compute_reward_risk_hint ---

    #[test]
    fn reward_risk_valid() {
        let tp = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let pd = StructuredPortfolioDecision {
            price_target: "120".into(),
            ..Default::default()
        };
        let result = compute_reward_risk_hint(&tp, &pd);
        assert_eq!(result, Some((120.0 - 100.0) / (100.0 - 95.0)));
    }

    #[test]
    fn reward_risk_entry_below_stop() {
        let tp = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "105".into(),
            ..Default::default()
        };
        let pd = StructuredPortfolioDecision {
            price_target: "120".into(),
            ..Default::default()
        };
        assert_eq!(compute_reward_risk_hint(&tp, &pd), None);
    }

    #[test]
    fn reward_risk_target_below_entry() {
        let tp = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let pd = StructuredPortfolioDecision {
            price_target: "90".into(),
            ..Default::default()
        };
        assert_eq!(compute_reward_risk_hint(&tp, &pd), None);
    }

    #[test]
    fn reward_risk_missing_entry() {
        let tp = StructuredTraderPlan {
            entry_price: "".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let pd = StructuredPortfolioDecision {
            price_target: "120".into(),
            ..Default::default()
        };
        assert_eq!(compute_reward_risk_hint(&tp, &pd), None);
    }

    #[test]
    fn reward_risk_confirmation_fallback() {
        let tp = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let pd = StructuredPortfolioDecision {
            price_target: "".into(),
            confirmation_level: "115".into(),
            ..Default::default()
        };
        let result = compute_reward_risk_hint(&tp, &pd);
        assert_eq!(result, Some((115.0 - 100.0) / (100.0 - 95.0)));
    }

    // --- sanitize_scenario_paths_for_no_attack ---

    #[test]
    fn sanitize_clears_position_sizing() {
        let mut guides = ReportActionGuides::default();
        guides.buyers.scenario_paths.push(ActionScenarioPath {
            key: "test".into(),
            position_sizing: LocalText::new("50%仓位"),
            sizing_blocked: false,
            ..Default::default()
        });
        guides.watchers.scenario_paths.push(ActionScenarioPath {
            key: "test2".into(),
            position_sizing: LocalText::new("30%仓位"),
            sizing_blocked: false,
            ..Default::default()
        });
        sanitize_scenario_paths_for_no_attack(&mut guides);
        assert!(guides.buyers.scenario_paths[0].position_sizing.key.is_empty());
        assert!(guides.buyers.scenario_paths[0].sizing_blocked);
        assert!(guides.watchers.scenario_paths[0].position_sizing.key.is_empty());
        assert!(guides.watchers.scenario_paths[0].sizing_blocked);
    }
}
