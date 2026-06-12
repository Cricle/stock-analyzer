impl LlmClient {
    pub(crate) async fn generate_with_openai_compatible(
        &self,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You must output valid JSON with no markdown fences.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: 0.2,
            response_format: Some(ResponseFormat {
                kind: "json_object".to_string(),
            }),
        };

        const MAX_ATTEMPTS: usize = 6;
        let mut attempt = 0usize;
        let backoff = llm_retry_backoff();

        retry(backoff, || {
            attempt += 1;
            let request = &request;
            async move {
                match self.send_chat_completion_once(request).await {
                    Ok(content) => Ok(content),
                    Err(error) if is_retryable_llm_error(&error) && attempt < MAX_ATTEMPTS => {
                        tracing::warn!(
                            attempt,
                            max_attempts = MAX_ATTEMPTS,
                            error = %error,
                            "retrying transient LLM upstream failure"
                        );
                        Err(BackoffError::transient(error))
                    }
                    Err(error) => Err(BackoffError::permanent(error.context(format!(
                        "OpenAI-compatible LLM request failed after {attempt} attempt(s)"
                    )))),
                }
            }
        })
        .await
    }

    async fn send_chat_completion_once(
        &self,
        request: &ChatCompletionRequest,
    ) -> anyhow::Result<String> {
        let response = tokio::time::timeout(
            self.timeout,
            self.http
                .post(format!("{}/chat/completions", self.openai_base_url))
                .header(AUTHORIZATION, format!("Bearer {}", self.openai_api_key))
                .header(CONTENT_TYPE, "application/json")
                .json(request)
                .send(),
        )
        .await
        .context("LLM request timed out")?
        .context("failed to call OpenAI-compatible LLM endpoint")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("llm request failed with {status}: {body}");
        }

        let payload: ChatCompletionResponse = tokio::time::timeout(self.timeout, response.json())
            .await
            .context("LLM response decode timed out")?
            .context("failed to decode OpenAI-compatible LLM response")?;
        let first_choice = payload
            .choices
            .first()
            .context("LLM response contained no choices")?;
        let content = first_choice.message.content_text();
        self.record_usage(
            payload.model.as_deref().unwrap_or(self.model.as_str()),
            payload.usage.as_ref(),
            request,
            &content,
        )
        .await;
        if !content.trim().is_empty() {
            return Ok(content);
        }
        let diagnostic = serde_json::to_string(&first_choice.message)
            .unwrap_or_else(|_| "<message serialization failed>".to_string());
        bail!("LLM response contained no content: {diagnostic}")
    }

    async fn record_usage(
        &self,
        resolved_model: &str,
        usage: Option<&ChatCompletionUsage>,
        request: &ChatCompletionRequest,
        content: &str,
    ) {
        let usage = usage
            .cloned()
            .unwrap_or_else(|| estimate_chat_completion_usage(request, content));
        let mut tracker = self.usage_tracker.lock().expect("usage tracker mutex poisoned");
        tracker.total_requests += 1;
        tracker.prompt_tokens += usage.prompt_tokens;
        tracker.completion_tokens += usage.completion_tokens;
        tracker.total_tokens += usage.total_tokens;
        let entry = tracker
            .by_model
            .entry(resolved_model.trim().to_string())
            .or_default();
        entry.requests += 1;
        entry.prompt_tokens += usage.prompt_tokens;
        entry.completion_tokens += usage.completion_tokens;
        entry.total_tokens += usage.total_tokens;
    }
}
