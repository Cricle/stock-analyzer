pub fn extract_stream_delta_text(value: &Value) -> String {
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
            tools: None,
            tool_choice: None,
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
