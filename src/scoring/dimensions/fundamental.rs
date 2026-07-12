use crate::scoring::score_types::DimensionScore;

/// Input data for fundamental analysis scoring.
pub struct FundamentalInput {
    pub pe_like: Option<f64>,
    pub ps_like: Option<f64>,
    pub roe: Option<f64>,
    pub leverage: Option<f64>,
    pub market_cap: Option<f64>,
    pub revenues_usd: Option<f64>,
    pub net_income_usd: Option<f64>,
}

/// Score fundamental quality from PE, ROE, leverage, and revenue metrics.
pub fn score_fundamental(input: &FundamentalInput) -> DimensionScore {
    let mut total: f64 = 0.0;
    let mut weight_sum: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    // PE valuation (weight 25)
    if let Some(pe) = input.pe_like {
        weight_sum += 25.0;
        if pe < 0.0 {
            total += 12.5;
            reasons.push("PE为负，盈利承压".into());
        } else if pe < 15.0 {
            total += 25.0;
            reasons.push(format!("PE {:.1} 估值偏低", pe));
        } else if pe < 25.0 {
            total += 18.0;
        } else if pe < 40.0 {
            total += 10.0;
            reasons.push(format!("PE {:.1} 估值偏高", pe));
        } else {
            total += 3.0;
            reasons.push(format!("PE {:.1} 估值过高", pe));
        }
    }

    // ROE profitability (weight 25)
    if let Some(roe) = input.roe {
        weight_sum += 25.0;
        if roe > 20.0 {
            total += 25.0;
            reasons.push(format!("ROE {:.1}% 优秀", roe));
        } else if roe > 10.0 {
            total += 18.0;
        } else if roe > 0.0 {
            total += 10.0;
        } else {
            total += 2.0;
            reasons.push(format!("ROE {:.1}% 亏损", roe));
        }
    }

    // Leverage / debt (weight 25)
    if let Some(lev) = input.leverage {
        weight_sum += 25.0;
        if lev < 1.0 {
            total += 25.0;
            reasons.push("低负债".into());
        } else if lev < 2.0 {
            total += 18.0;
        } else if lev < 3.0 {
            total += 10.0;
            reasons.push(format!("负债率 {:.1} 偏高", lev));
        } else {
            total += 3.0;
            reasons.push(format!("负债率 {:.1} 过高", lev));
        }
    }

    // Revenue signal (weight 25)
    if let Some(rev) = input.revenues_usd {
        weight_sum += 25.0;
        if rev > 0.0 {
            if let Some(ni) = input.net_income_usd {
                if ni > 0.0 {
                    total += 22.0;
                    reasons.push("营收盈利为正".into());
                } else {
                    total += 12.0;
                    reasons.push("营收为正但净利亏损".into());
                }
            } else {
                total += 15.0;
            }
        } else {
            total += 3.0;
            reasons.push("营收数据缺失或为零".into());
        }
    }

    super::weighted_score(total, weight_sum, "基本面数据不足，给予中性评分", &reasons)
}
