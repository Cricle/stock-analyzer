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
        summary.push(format!("risk_section.blocking_prefix{}", risk.decision_blocking_gaps.join("；")));
    }
    if !risk.key_risks.is_empty() {
        summary.push(format!("risk_section.risk_prefix{}", risk.key_risks.join("；")));
    }
    if !risk.offsetting_supports.is_empty() {
        summary.push(format!("risk_section.support_prefix{}", risk.offsetting_supports.join("；")));
    }
    if summary.is_empty() {
        risk.raw_text.trim().to_string()
    } else {
        summary.join(" ")
    }
}

fn render_structured_risk_assessment_sections(risk: &StructuredRiskAssessment) -> String {
    let ordered = [
        ("risk_section.blocking_gaps", &risk.decision_blocking_gaps),
        ("risk_section.key_risks", &risk.key_risks),
        ("risk_section.offsetting_supports", &risk.offsetting_supports),
        ("risk_section.invalidation_conditions", &risk.invalidation_conditions),
        ("risk_section.manageable_gaps", &risk.serious_but_manageable_gaps),
        ("risk_section.tolerable_gaps", &risk.tolerable_context_gaps),
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
            "### risk_section.overall_framing\n{}\n",
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

