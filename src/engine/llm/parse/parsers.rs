use super::super::{GeneratedResearchManager, GeneratedPortfolioDecision, GeneratedAnalystDecision, GeneratedRoleReport, GeneratedDebateTurn, GeneratedTraderDecision, GeneratedSubscriptionQaAnswer};
use super::validate::{validate_research_manager, validate_portfolio_decision, validate_analyst_decision, validate_debate_turn, validate_trader_decision};
use super::candidates::parse_object_candidates_value;
use anyhow::bail;
use serde_json::Value;

pub(crate) fn parse_generated_research_manager(
    content: &str,
) -> anyhow::Result<GeneratedResearchManager> {
    let parsed = parse_object_candidates_value(content, GeneratedResearchManager::from_value)?;
    validate_research_manager(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_portfolio_decision(
    content: &str,
) -> anyhow::Result<GeneratedPortfolioDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedPortfolioDecision::from_value)?;
    validate_portfolio_decision(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_analyst_decision(
    content: &str,
) -> anyhow::Result<GeneratedAnalystDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedAnalystDecision::from_value)?;
    if parsed.action.eq_ignore_ascii_case("finalize") && parsed.final_report.is_none() {
        let report = parse_object_candidates_value(content, GeneratedRoleReport::from_value)?;
        return Ok(GeneratedAnalystDecision {
            action: "finalize".to_string(),
            reasoning: "normalized legacy role-report response into analyst finalize decision"
                .to_string(),
            final_report: Some(report),
            tool_name: None,
            tool_arguments: None,
        });
    }
    validate_analyst_decision(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_debate_turn(content: &str) -> anyhow::Result<GeneratedDebateTurn> {
    match parse_object_candidates_value(content, GeneratedDebateTurn::from_value) {
        Ok(parsed) => {
            validate_debate_turn(&parsed, content);
            Ok(parsed)
        }
        Err(primary_error) => parse_generated_debate_turn_lenient(content).ok_or(primary_error),
    }
}

pub(crate) fn parse_generated_trader_decision(
    content: &str,
) -> anyhow::Result<GeneratedTraderDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedTraderDecision::from_value)?;
    validate_trader_decision(&parsed, content);
    Ok(parsed)
}

pub fn parse_generated_subscription_qa_answer(
    content: &str,
) -> anyhow::Result<GeneratedSubscriptionQaAnswer> {
    parse_object_candidates_value(content, GeneratedSubscriptionQaAnswer::from_value)
}
use super::candidates::parse_generated_debate_turn_lenient;

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_generated_portfolio_decision ---

    #[test]
    fn parse_portfolio_valid() {
        let json = r#"{
            "rating": "Buy",
            "confidence": 75,
            "risk_assessment": "moderate",
            "summary": "strong outlook",
            "rationale": "good fundamentals",
            "executive_summary": "buy recommendation",
            "investment_thesis": "growth story intact"
        }"#;
        let result = parse_generated_portfolio_decision(json);
        assert!(result.is_ok());
        let decision = result.unwrap();
        assert_eq!(decision.rating, "Buy");
    }

    #[test]
    fn parse_portfolio_invalid_json() {
        let result = parse_generated_portfolio_decision("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_portfolio_empty_object() {
        let result = parse_generated_portfolio_decision("{}");
        assert!(result.is_err());
    }

    // --- parse_generated_trader_decision ---

    #[test]
    fn parse_trader_valid() {
        let json = r#"{
            "action": "Buy",
            "reasoning": "good setup",
            "trader_plan": "enter at 100"
        }"#;
        let result = parse_generated_trader_decision(json);
        assert!(result.is_ok());
        let decision = result.unwrap();
        assert_eq!(decision.action, "Buy");
    }

    #[test]
    fn parse_trader_invalid() {
        let result = parse_generated_trader_decision("{invalid}");
        assert!(result.is_err());
    }

    // --- parse_generated_analyst_decision ---

    #[test]
    fn parse_analyst_with_tool_call() {
        let json = r#"{
            "action": "use_tool",
            "reasoning": "need more data",
            "tool_name": "get_fundamentals",
            "tool_arguments": {"symbol": "AAPL"}
        }"#;
        let result = parse_generated_analyst_decision(json);
        assert!(result.is_ok());
    }

    // --- parse_generated_debate_turn ---

    #[test]
    fn parse_debate_valid() {
        let json = r#"{
            "stance": "aggressive",
            "argument": "strong momentum",
            "response_to_others": "agree with bull case"
        }"#;
        let result = parse_generated_debate_turn(json);
        assert!(result.is_ok());
    }

    // --- parse_generated_subscription_qa_answer ---

    #[test]
    fn parse_qa_valid() {
        let json = r#"{"answer": "yes"}"#;
        let result = parse_generated_subscription_qa_answer(json);
        assert!(result.is_ok());
    }
}
