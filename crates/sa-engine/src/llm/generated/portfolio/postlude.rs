
impl GeneratedReflection {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        Self {
            strongest_part: parse::first_non_empty(
                &[field("strongest_part").as_ref(), field("strength").as_ref()],
                "",
            ),
            weakest_uncertainty_or_missing_evidence: parse::first_non_empty(
                &[
                    field("weakest_uncertainty_or_missing_evidence").as_ref(),
                    field("weakest_uncertainty").as_ref(),
                    field("main_uncertainty").as_ref(),
                ],
                "",
            ),
            next_lesson_for_next_run: parse::first_non_empty(
                &[
                    field("next_lesson_for_next_run").as_ref(),
                    field("next_lesson").as_ref(),
                    field("next_lessons").as_ref(),
                ],
                "",
            ),
        }
    }

    fn rendered(&self) -> String {
        serde_json::json!({
            "strongest_part": self.strongest_part,
            "weakest_uncertainty_or_missing_evidence": self.weakest_uncertainty_or_missing_evidence,
            "next_lesson_for_next_run": self.next_lesson_for_next_run,
        })
        .to_string()
    }
}
