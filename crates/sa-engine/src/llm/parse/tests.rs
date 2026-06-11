#[cfg(test)]
mod tests {
    use super::{parse_generated_debate_turn, parse_generated_research_manager};

    #[test]
    fn repairs_research_manager_missing_object_wrapper() {
        let content = r#"{"recommendation":"Overweight","rating":"Overweight","confidence":0.66,"risk_assessment":{"overall_risk_frame":"frame","key_risks":["risk"],"offsetting_supports":["support"],"missing_evidence":"tolerable_context_gaps":["gap-a"],"serious_but_manageable_gaps":["gap-b"],"decision_blocking_gaps":["gap-c"]},"rationale":"because","strategic_actions":{"position_expression":"small","what_to_monitor_next":["223.75"]}}"#;

        let parsed = parse_generated_research_manager(content).expect("should parse repaired JSON");

        assert_eq!(parsed.recommendation, "Overweight");
        assert!(parsed.risk_assessment.contains("overall_risk_frame: frame"));
        assert!(
            parsed
                .risk_assessment
                .contains("missing_evidence: tolerable_context_gaps: gap-a")
        );
        assert!(
            parsed
                .risk_assessment
                .contains("serious_but_manageable_gaps: gap-b")
        );
        assert_eq!(
            parsed.strategic_actions,
            "position_expression: small\nwhat_to_monitor_next: 223.75"
        );
    }

    #[test]
    fn leniently_parses_debate_turn_with_unescaped_quotes_in_response() {
        let content = r#"{"speaker":"Bear Researcher","stance":"bear","response":"我不同意多头把NVDA视为“只会越来越强”的前提。问题不在于公司差，而在于市场把“AI平台”叙事过度外推。","confidence":0.81,"evidence_points":["估值对持续超预期依赖很强","趋势拥挤提高回撤脆弱性"],"risks":["若后续财报继续大超预期，空头判断会被证伪"]}"#;

        let parsed =
            parse_generated_debate_turn(content).expect("should parse lenient debate JSON");

        assert_eq!(parsed.speaker, "Bear Researcher");
        assert_eq!(parsed.stance, "bear");
        assert!(parsed.response.contains("只会越来越强"));
        assert_eq!(parsed.evidence_points.len(), 2);
        assert_eq!(parsed.risks.len(), 1);
    }
}
