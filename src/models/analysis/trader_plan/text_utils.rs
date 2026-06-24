fn split_section_items(
    sections: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Vec<String> {
    sections
        .get(key)
        .map(|value| split_semicolon_items(value))
        .unwrap_or_default()
}

fn humanize_structured_risk_assessment(risk: &StructuredRiskAssessment) -> String {
    let mut summary = Vec::new();
    if !risk.decision_blocking_gaps.is_empty() {
        summary.push(format!("当前主要阻断项是：{}", risk.decision_blocking_gaps.join("；")));
    }
    if !risk.key_risks.is_empty() {
        summary.push(format!("核心风险包括：{}", risk.key_risks.join("；")));
    }
    if !risk.offsetting_supports.is_empty() {
        summary.push(format!("但当前仍有支撑因素：{}", risk.offsetting_supports.join("；")));
    }
    if summary.is_empty() {
        risk.raw_text.trim().to_string()
    } else {
        summary.join(" ")
    }
}

fn render_structured_risk_assessment_sections(risk: &StructuredRiskAssessment) -> String {
    let ordered = [
        ("当前阻断项", &risk.decision_blocking_gaps),
        ("核心风险", &risk.key_risks),
        ("当前支撑", &risk.offsetting_supports),
        ("失效/重审条件", &risk.invalidation_conditions),
        ("可管理缺口", &risk.serious_but_manageable_gaps),
        ("可容忍背景缺口", &risk.tolerable_context_gaps),
    ];

    let mut sections = ordered
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(title, items)| {
            format!(
                "### {title}\n{}\n",
                items
                    .iter()
                    .map(|item| format!("- {item}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<_>>();

    if !risk.overall_risk_framing.trim().is_empty() {
        sections.push(format!(
            "### 整体判断\n{}\n",
            risk.overall_risk_framing.trim()
        ));
    }

    sections.join("\n")
}

impl AnalysisResult {
    pub fn analyst_runtime_state(&self, key: &str) -> Option<&AnalystRuntimeState> {
        self.artifacts
            .analyst_runtime_states
            .iter()
            .find(|item| item.key == key)
    }

    pub fn analyst_runtime_state_mut(&mut self, key: &str) -> &mut AnalystRuntimeState {
        if let Some(index) = self
            .artifacts
            .analyst_runtime_states
            .iter()
            .position(|item| item.key == key)
        {
            return &mut self.artifacts.analyst_runtime_states[index];
        }
        self.artifacts
            .analyst_runtime_states
            .push(AnalystRuntimeState {
                key: key.to_string(),
                ..Default::default()
            });
        self.artifacts
            .analyst_runtime_states
            .last_mut()
            .expect("just pushed analyst runtime state")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- split_section_items ---

    #[test]
    fn split_section_items_found() {
        let mut sections = BTreeMap::new();
        sections.insert("key1".to_string(), "a; b; c".to_string());
        assert_eq!(split_section_items(&sections, "key1"), vec!["a", "b", "c"]);
    }

    #[test]
    fn split_section_items_missing() {
        let sections = BTreeMap::new();
        assert_eq!(split_section_items(&sections, "missing"), Vec::<String>::new());
    }

    // --- humanize_structured_risk_assessment ---

    #[test]
    fn humanize_with_all_fields() {
        let risk = StructuredRiskAssessment {
            decision_blocking_gaps: vec!["gap1".into()],
            key_risks: vec!["risk1".into()],
            offsetting_supports: vec!["support1".into()],
            ..Default::default()
        };
        let result = humanize_structured_risk_assessment(&risk);
        assert!(result.contains("gap1"));
        assert!(result.contains("risk1"));
        assert!(result.contains("support1"));
    }

    #[test]
    fn humanize_empty_falls_back_to_raw() {
        let risk = StructuredRiskAssessment {
            raw_text: "fallback text".into(),
            ..Default::default()
        };
        assert_eq!(humanize_structured_risk_assessment(&risk), "fallback text");
    }

    #[test]
    fn humanize_empty_all() {
        let risk = StructuredRiskAssessment::default();
        assert_eq!(humanize_structured_risk_assessment(&risk), "");
    }

    // --- render_structured_risk_assessment_sections ---

    #[test]
    fn render_sections_with_data() {
        let risk = StructuredRiskAssessment {
            decision_blocking_gaps: vec!["gap1".into()],
            key_risks: vec!["risk1".into(), "risk2".into()],
            overall_risk_framing: "整体判断".into(),
            ..Default::default()
        };
        let result = render_structured_risk_assessment_sections(&risk);
        assert!(result.contains("当前阻断项"));
        assert!(result.contains("核心风险"));
        assert!(result.contains("整体判断"));
        assert!(result.contains("- gap1"));
        assert!(result.contains("- risk1"));
    }

    #[test]
    fn render_sections_empty() {
        let risk = StructuredRiskAssessment::default();
        let result = render_structured_risk_assessment_sections(&risk);
        assert!(result.is_empty());
    }

    // --- analyst_runtime_state ---

    #[test]
    fn runtime_state_found() {
        let mut result = AnalysisResult::default();
        result.artifacts.analyst_runtime_states.push(AnalystRuntimeState {
            key: "market".into(),
            ..Default::default()
        });
        assert!(result.analyst_runtime_state("market").is_some());
    }

    #[test]
    fn runtime_state_not_found() {
        let result = AnalysisResult::default();
        assert!(result.analyst_runtime_state("nonexistent").is_none());
    }

    // --- analyst_runtime_state_mut ---

    #[test]
    fn runtime_state_mut_existing() {
        let mut result = AnalysisResult::default();
        result.artifacts.analyst_runtime_states.push(AnalystRuntimeState {
            key: "market".into(),
            cleared: false,
            ..Default::default()
        });
        result.analyst_runtime_state_mut("market").cleared = true;
        assert!(result.artifacts.analyst_runtime_states[0].cleared);
    }

    #[test]
    fn runtime_state_mut_creates_new() {
        let mut result = AnalysisResult::default();
        result.analyst_runtime_state_mut("news").cleared = true;
        assert_eq!(result.artifacts.analyst_runtime_states.len(), 1);
        assert_eq!(result.artifacts.analyst_runtime_states[0].key, "news");
        assert!(result.artifacts.analyst_runtime_states[0].cleared);
    }
}

