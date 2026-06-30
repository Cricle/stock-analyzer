impl LlmClient {
    pub async fn generate_with_openai_compatible(
        &self,
        prompt: &str,
    ) -> anyhow::Result<String> {
        // DeepSeek prompt cache requires the first ~64 tokens to match across requests.
        // A shared system message ensures all analyst/manager/trader calls hit the cache.
        let system_message = concat!(
            "You are a disciplined quantitative analyst in a multi-agent stock analysis system. ",
            "You must output valid JSON with no markdown fences, no code blocks, and no commentary outside the JSON object. ",
            "All numeric fields must be actual numbers, not strings. ",
            "All price levels must be realistic relative to the instrument's current trading range. ",
            "When evidence is insufficient, state what is missing explicitly rather than inventing data. ",
            "Missing-evidence classification: 'blocking_gaps' = data without which the thesis cannot be tested at all (e.g. no price data, no financials). ",
            "'tolerable_gaps' = data that would strengthen conviction but is not strictly required for action (e.g. insider transactions, earnings guidance, analyst revisions). ",
            "'manageable_gaps' = data that creates uncertainty but can be addressed with position sizing or stop discipline. ",
            "For A-shares (A股): insider transaction data and earnings guidance are often unavailable -- classify them as tolerable, not blocking."
        );
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_message.to_string(),
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
        let start = std::time::Instant::now();
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
            self.record_otel_metrics(request, 0, 0, start, "error");
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
        let usage = payload
            .usage
            .as_ref()
            .cloned()
            .unwrap_or_else(|| estimate_chat_completion_usage(request, &content));
        self.record_usage(
            payload.model.as_deref().unwrap_or(self.model.as_str()),
            payload.usage.as_ref(),
            request,
            &content,
        )
        .await;
        self.record_otel_metrics(
            request,
            usage.prompt_tokens,
            usage.completion_tokens,
            start,
            "success",
        );
        if !content.trim().is_empty() {
            return Ok(content);
        }
        let diagnostic = serde_json::to_string(&first_choice.message)
            .unwrap_or_else(|_| "<message serialization failed>".to_string());
        bail!("LLM response contained no content: {diagnostic}")
    }

    fn record_otel_metrics(
        &self,
        request: &ChatCompletionRequest,
        prompt_tokens: i64,
        completion_tokens: i64,
        start: std::time::Instant,
        outcome: &'static str,
    ) {
        use opentelemetry::KeyValue;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let meter = opentelemetry::global::meter("tradingagents");
        let llm_requests = meter.u64_counter("llm_requests_total").build();
        let llm_duration = meter.f64_histogram("llm_request_duration_ms").build();
        let llm_tokens_prompt = meter.u64_counter("llm_tokens_prompt_total").build();
        let llm_tokens_completion = meter.u64_counter("llm_tokens_completion_total").build();
        let llm_tokens_total = meter.u64_counter("llm_tokens_total").build();
        let llm_errors = meter.u64_counter("llm_errors_total").build();

        let attrs = [
            KeyValue::new("llm.model", request.model.clone()),
            KeyValue::new("llm.provider", self.provider_type.clone()),
            KeyValue::new("llm.outcome", outcome),
        ];
        llm_requests.add(1, &attrs);
        llm_duration.record(elapsed_ms, &attrs);
        if prompt_tokens > 0 {
            llm_tokens_prompt.add(prompt_tokens as u64, &attrs);
        }
        if completion_tokens > 0 {
            llm_tokens_completion.add(completion_tokens as u64, &attrs);
        }
        let total = prompt_tokens + completion_tokens;
        if total > 0 {
            llm_tokens_total.add(total as u64, &attrs);
        }
        if outcome == "error" {
            llm_errors.add(1, &attrs);
        }
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
