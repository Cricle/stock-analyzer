use crate::engine::score::types::DimensionScore;

pub struct FundamentalInput {
    pub pe_like: Option<f64>,
    pub ps_like: Option<f64>,
    pub roe: Option<f64>,
    pub leverage: Option<f64>,
    pub market_cap: Option<f64>,
    pub revenues_usd: Option<f64>,
    pub net_income_usd: Option<f64>,
}

pub fn score_fundamental(input: &FundamentalInput) -> DimensionScore {
    let mut total: f64 = 0.0;
    let mut weight_sum: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();
    let mut reason_keys: Vec<String> = Vec::new();

    // PE valuation (weight 25)
    if let Some(pe) = input.pe_like {
        weight_sum += 25.0;
        if pe < 0.0 {
            total += 12.5;
            reasons.push("Negative PE, earnings under pressure".into());
            reason_keys.push("score.fundamental.negative_pe".into());
        } else if pe < 15.0 {
            total += 25.0;
            reasons.push(format!("PE {:.1} undervalued", pe));
            reason_keys.push("score.fundamental.pe_low".into());
        } else if pe < 25.0 {
            total += 18.0;
        } else if pe < 40.0 {
            total += 10.0;
            reasons.push(format!("PE {:.1} overvalued", pe));
            reason_keys.push("score.fundamental.pe_high".into());
        } else {
            total += 3.0;
            reasons.push(format!("PE {:.1} extremely overvalued", pe));
            reason_keys.push("score.fundamental.pe_too_high".into());
        }
    }

    // ROE profitability (weight 25)
    if let Some(roe) = input.roe {
        weight_sum += 25.0;
        if roe > 20.0 {
            total += 25.0;
            reasons.push(format!("ROE {:.1}% excellent", roe));
            reason_keys.push("score.fundamental.roe_excellent".into());
        } else if roe > 10.0 {
            total += 18.0;
        } else if roe > 0.0 {
            total += 10.0;
        } else {
            total += 2.0;
            reasons.push(format!("ROE {:.1}% loss", roe));
            reason_keys.push("score.fundamental.roe_loss".into());
        }
    }

    // Leverage / debt (weight 25)
    if let Some(lev) = input.leverage {
        weight_sum += 25.0;
        if lev < 1.0 {
            total += 25.0;
            reasons.push("Low leverage".into());
            reason_keys.push("score.fundamental.low_leverage".into());
        } else if lev < 2.0 {
            total += 18.0;
        } else if lev < 3.0 {
            total += 10.0;
            reasons.push(format!("Leverage {:.1} elevated", lev));
            reason_keys.push("score.fundamental.leverage_high".into());
        } else {
            total += 3.0;
            reasons.push(format!("Leverage {:.1} excessive", lev));
            reason_keys.push("score.fundamental.leverage_too_high".into());
        }
    }

    // Revenue signal (weight 25)
    if let Some(rev) = input.revenues_usd {
        weight_sum += 25.0;
        if rev > 0.0 {
            if let Some(ni) = input.net_income_usd {
                if ni > 0.0 {
                    total += 22.0;
                    reasons.push("Revenue and profit positive".into());
                    reason_keys.push("score.fundamental.revenue_profit_positive".into());
                } else {
                    total += 12.0;
                    reasons.push("Revenue positive but net loss".into());
                    reason_keys.push("score.fundamental.revenue_positive_net_loss".into());
                }
            } else {
                total += 15.0;
            }
        } else {
            total += 3.0;
            reasons.push("Revenue missing or zero".into());
            reason_keys.push("score.fundamental.revenue_missing".into());
        }
    }

    let score = if weight_sum > 0.0 {
        (total / weight_sum * 100.0).clamp(0.0, 100.0) as u8
    } else {
        50
    };

    let reason = if reasons.is_empty() {
        "Insufficient fundamental data, neutral score".into()
    } else {
        reasons.join("；")
    };

    let reason_key = if reason_keys.is_empty() {
        Some("score.fundamental.insufficient_data".into())
    } else {
        Some(reason_keys.join("；"))
    };

    DimensionScore {
        score,
        reason,
        reason_key,
    }
}

#[cfg(test)]
mod fundamental_tests;
