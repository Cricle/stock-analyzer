use crate::llm::parse::diagnosis::{DiagnosisIssue, IssueSeverity};

/// Retry an LLM call with progressive diagnosis feedback.
///
/// On each retry, the `diagnose` function is called on the output. If any
/// `Error`-severity issues are found, the `build_retry_hint` function is
/// called to produce a corrective prompt hint, and the attempt is retried.
///
/// After exhausting retries, the last output is returned (it's still usable,
/// just has quality issues).
pub async fn retry_with_diagnosis<F, Fut, T, D>(
    stage_name: &str,
    max_retries: u32,
    mut attempt: F,
    diagnose: D,
    build_retry_hint: impl Fn(&[DiagnosisIssue], u32) -> String,
) -> anyhow::Result<T>
where
    F: FnMut(Option<&str>) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
    D: Fn(&T) -> Vec<DiagnosisIssue>,
{
    let mut all_issues = Vec::new();

    for retry in 0..=max_retries {
        let retry_hint = if retry == 0 {
            None
        } else {
            let hint = build_retry_hint(&all_issues, retry);
            Some(hint)
        };

        let output = attempt(retry_hint.as_deref()).await?;
        let issues = diagnose(&output);

        let has_errors = issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Error));

        if !has_errors {
            if retry > 0 {
                tracing::info!(
                    stage = stage_name,
                    retry = retry,
                    "LLM output fixed after retry"
                );
            }
            return Ok(output);
        }

        all_issues = issues;
        tracing::warn!(
            stage = stage_name,
            retry = retry,
            issues = %all_issues
                .iter()
                .map(|i| i.message.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            "LLM output has issues, retrying"
        );
    }

    // All retries exhausted -- perform one final attempt without hint
    // to get a clean output (the last one had errors).
    tracing::warn!(
        stage = stage_name,
        retries = max_retries,
        "LLM retries exhausted, using best-effort output"
    );
    attempt(None).await
}

/// Default retry hint builder: lists the issues and asks for strict JSON.
pub fn default_retry_hint_builder(issues: &[DiagnosisIssue], retry: u32) -> String {
    let issue_summary = issues
        .iter()
        .map(|i| format!("{}: {}", i.field, i.message))
        .collect::<Vec<_>>()
        .join("; ");

    format!(
        "IMPORTANT (retry {}): Your previous output had quality issues: {}. \
         You MUST provide real, substantive content for all required fields. \
         Do NOT use placeholder text or default values. \
         Return strict JSON only.",
        retry, issue_summary,
    )
}
