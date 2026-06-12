fn extract_stream_delta_text(value: &Value) -> String {
    let mut parts = Vec::new();

    if let Some(delta_content) = value["choices"]
        .get(0)
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
    {
        match delta_content {
            Value::String(text) if !text.is_empty() => parts.push(text.to_string()),
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.get("text").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        parts.push(text.to_string());
                    } else if let Some(text) = item.get("content").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        parts.push(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    if parts.is_empty()
        && let Some(message_content) = value["choices"]
            .get(0)
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
    {
        match message_content {
            Value::String(text) if !text.is_empty() => parts.push(text.to_string()),
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.get("text").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        parts.push(text.to_string());
                    } else if let Some(text) = item.get("content").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        parts.push(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    parts.join("")
}

impl LlmClient {
    pub async fn stream_with_openai_compatible<F>(
        &self,
        prompt: &str,
        on_delta: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str) -> anyhow::Result<()>,
    {
        const MAX_ATTEMPTS: usize = 6;
        let mut attempt = 0usize;
        let backoff = llm_retry_backoff();

        let on_delta = std::sync::Mutex::new(on_delta);

        retry(backoff, || {
            attempt += 1;
            let on_delta = &on_delta;
            async move {
                let request = serde_json::json!({
                    "model": self.model,
                    "messages": [
                        {
                            "role": "system",
                            "content": "You must output valid JSON with no markdown fences."
                        },
                        {
                            "role": "user",
                            "content": prompt
                        }
                    ],
                    "temperature": 0.2,
                    "response_format": { "type": "json_object" },
                    "stream": true
                });
                let response = tokio::time::timeout(
                    self.timeout,
                    self.http
                        .post(format!("{}/chat/completions", self.openai_base_url))
                        .header(AUTHORIZATION, format!("Bearer {}", self.openai_api_key))
                        .header(CONTENT_TYPE, "application/json")
                        .json(&request)
                        .send(),
                )
                .await
                .context("LLM request timed out")?
                .context("failed to call OpenAI-compatible LLM endpoint")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    let err = anyhow::anyhow!("llm stream request failed with {status}: {body}");
                    if is_retryable_llm_error(&err) && attempt < MAX_ATTEMPTS {
                        return Err(BackoffError::transient(err));
                    }
                    return Err(BackoffError::permanent(err));
                }

                let mut stream = response.bytes_stream();
                let mut buffer = String::new();
                let mut content = String::new();

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| {
                        let err = anyhow::anyhow!("failed to read LLM stream chunk: {e}");
                        if is_retryable_llm_error(&err) && attempt < MAX_ATTEMPTS {
                            BackoffError::transient(err)
                        } else {
                            BackoffError::permanent(err)
                        }
                    })?;
                    buffer.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(index) = buffer.find("\n\n") {
                        let frame: String = buffer.drain(..index + 2).collect();
                        let frame = &frame[..frame.len() - 2];
                        for line in frame.lines() {
                            let line = line.trim();
                            if !line.starts_with("data:") {
                                continue;
                            }
                            let payload = line.trim_start_matches("data:").trim();
                            if payload == "[DONE]" {
                                let request = ChatCompletionRequest {
                                    model: self.model.clone(),
                                    messages: vec![
                                        ChatMessage {
                                            role: "system".to_string(),
                                            content: "You must output valid JSON with no markdown fences."
                                                .to_string(),
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
                                self.record_usage(self.model.as_str(), None, &request, &content)
                                    .await;
                                return Ok(content);
                            }
                            let value: Value = serde_json::from_str(payload)
                                .with_context(|| format!("invalid llm stream payload: {payload}"))
                                .map_err(BackoffError::permanent)?;
                            let delta = extract_stream_delta_text(&value);
                            if !delta.is_empty() {
                                content.push_str(&delta);
                                on_delta
                                    .lock()
                                    .expect("stream callback mutex poisoned")(&delta)
                                    .map_err(BackoffError::permanent)?;
                            }
                        }
                    }
                }

                Err(BackoffError::permanent(anyhow::anyhow!(
                    "LLM stream ended without [DONE]"
                )))
            }
        })
        .await
    }

    pub async fn list_models_openai_compatible(
        &self,
        base_url: &str,
        api_key: &str,
    ) -> anyhow::Result<Vec<String>> {
        let response = self
            .http
            .get(format!("{}/models", base_url.trim_end_matches('/')))
            .header(AUTHORIZATION, format!("Bearer {}", api_key))
            .send()
            .await
            .context("failed to call OpenAI-compatible models endpoint")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("openai-compatible models request failed with {status}: {body}");
        }

        let payload: ModelsResponse = response
            .json()
            .await
            .context("failed to decode OpenAI-compatible models response")?;
        let models = payload
            .data
            .into_iter()
            .map(|item| item.id.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        Ok(models)
    }

    pub async fn healthcheck_openai_compatible(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> anyhow::Result<()> {
        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Reply with JSON: {\"ok\":true}".to_string(),
            }],
            temperature: 0.0,
            response_format: Some(ResponseFormat {
                kind: "json_object".to_string(),
            }),
        };

        let response = self
            .http
            .post(format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            ))
            .header(AUTHORIZATION, format!("Bearer {}", api_key))
            .header(CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to call OpenAI-compatible endpoint")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("openai-compatible healthcheck failed with {status}: {body}");
        }

        Ok(())
    }

}
