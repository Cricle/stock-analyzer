
pub fn compute_reward_risk_hint(
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

pub fn extract_first_price(text: &str) -> Option<f64> {
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
