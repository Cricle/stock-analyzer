impl LlmClient {
    pub(crate) async fn generate_with_anthropic(&self, prompt: &str) -> anyhow::Result<String> {
        const MAX_ATTEMPTS: usize = 6;
        let mut attempt = 0usize;
        let backoff = llm_retry_backoff();

        let base_url = self.openai_base_url.trim_end_matches('/');
        let url = if base_url.ends_with("/v1") {
            format!("{}/messages", base_url)
        } else {
            format!("{}/v1/messages", base_url)
        };

        // System prompt for Anthropic API — sent as content blocks to enable prompt caching.
        let system_blocks = serde_json::json!([{
            "type": "text",
            "text": concat!(
                "You are a disciplined quantitative analyst in a multi-agent stock analysis system. ",
                "You must output valid JSON with no markdown fences, no code blocks, and no commentary outside the JSON object. ",
                "All numeric fields must be actual numbers, not strings. ",
                "All price levels must be realistic relative to the instrument's current trading range. ",
                "When evidence is insufficient, state what is missing explicitly rather than inventing data. ",
                "Missing-evidence classification: 'blocking_gaps' = data without which the thesis cannot be tested at all (e.g. no price data, no financials). ",
                "'tolerable_gaps' = data that would strengthen conviction but is not strictly required for action (e.g. insider transactions, earnings guidance, analyst revisions). ",
                "'manageable_gaps' = data that creates uncertainty but can be addressed with position sizing or stop discipline. ",
                "For A-shares (A股): insider transaction data and earnings guidance are often unavailable -- classify them as tolerable, not blocking."
            ),
            "cache_control": {"type": "ephemeral"}
        }]);

        retry(backoff, || {
            attempt += 1;
            let url = url.clone();
            let prompt = prompt.to_string();
            let model = self.model.clone();
            let api_key = self.openai_api_key.clone();
            let http = self.http.clone();
            let timeout = self.timeout;
            let tracker = self.usage_tracker.clone();
            let system_blocks = system_blocks.clone();
            async move {
                let request = serde_json::json!({
                    "model": model,
                    "max_tokens": 16384,
                    "system": system_blocks,
                    "messages": [
                        {
                            "role": "user",
                            "content": prompt
                        }
                    ],
                    "temperature": 0.0
                });

                let response = tokio::time::timeout(
                    timeout,
                    http.post(&url)
                        .header("x-api-key", &api_key)
                        .header("anthropic-version", "2023-06-01")
                        .header("anthropic-beta", "prompt-caching-2024-07-31")
                        .header(CONTENT_TYPE, "application/json")
                        .json(&request)
                        .send(),
                )
                .await
                .context("LLM request timed out")?
                .context("failed to call Anthropic API")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    let err = anyhow::anyhow!("anthropic request failed with {status}: {body}");
                    if is_retryable_llm_error(&err) && attempt < MAX_ATTEMPTS {
                        return Err(BackoffError::transient(err));
                    }
                    return Err(BackoffError::permanent(err));
                }

                let payload: AnthropicResponse = tokio::time::timeout(timeout, response.json())
                    .await
                    .context("Anthropic response decode timed out")?
                    .context("failed to decode Anthropic response")?;

                let content = payload.content_text();
                let input_tokens = payload.usage.input_tokens.unwrap_or(0);
                let output_tokens = payload.usage.output_tokens.unwrap_or(0);
                let cache_read = payload.usage.cache_read_input_tokens.unwrap_or(0);
                let cache_creation = payload.usage.cache_creation_input_tokens.unwrap_or(0);
                {
                    let mut t = tracker.lock().expect("usage tracker mutex poisoned");
                    t.total_requests += 1;
                    t.prompt_tokens += input_tokens;
                    t.completion_tokens += output_tokens;
                    t.total_tokens += input_tokens + output_tokens;
                    let entry = t.by_model.entry(model).or_default();
                    entry.requests += 1;
                    entry.prompt_tokens += input_tokens;
                    entry.completion_tokens += output_tokens;
                    entry.total_tokens += input_tokens + output_tokens;
                    if cache_read > 0 || cache_creation > 0 {
                        tracing::debug!(
                            cache_read, cache_creation, input_tokens, output_tokens,
                            "anthropic prompt cache hit"
                        );
                    }
                }

                if !content.trim().is_empty() {
                    return Ok(content);
                }
                Err(BackoffError::permanent(anyhow::anyhow!(
                    "Anthropic response contained no content"
                )))
            }
        })
        .await
    }

    pub async fn healthcheck_anthropic(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let base = base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        };
        let request = serde_json::json!({
            "model": model,
            "max_tokens": 32,
            "messages": [
                {
                    "role": "user",
                    "content": "Reply with JSON: {\"ok\":true}"
                }
            ]
        });
        let response = self
            .http
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to call Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("anthropic healthcheck failed with {status}: {body}");
        }
        Ok(())
    }
}

pub fn is_retryable_llm_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("LLM request timed out")
            || text.contains("failed to call OpenAI-compatible LLM endpoint")
            || text.contains("429 Too Many Requests")
            || text.contains("500 Internal Server Error")
            || text.contains("502 Bad Gateway")
            || text.contains("503 Service Unavailable")
            || text.contains("504 Gateway Timeout")
            || text.contains("520")
            || text.contains("521")
            || text.contains("522")
            || text.contains("523")
            || text.contains("524")
            || text.contains("525")
            || text.contains("526")
            || text.contains("Upstream stream ended without a terminal response event")
            || text.contains("connection reset")
            || text.contains("timed out")
    })
}

impl AnthropicResponse {
    fn content_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| block.text.as_deref())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
