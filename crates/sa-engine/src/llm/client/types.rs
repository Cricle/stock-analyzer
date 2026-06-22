#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    model: Option<String>,
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionUsage {
    #[serde(default)]
    prompt_tokens: i64,
    #[serde(default)]
    completion_tokens: i64,
    #[serde(default)]
    total_tokens: i64,
}

fn estimate_chat_completion_usage(
    request: &ChatCompletionRequest,
    content: &str,
) -> ChatCompletionUsage {
    let prompt_chars = request
        .messages
        .iter()
        .map(|message| message.role.len() + message.content.len())
        .sum::<usize>() as i64;
    let completion_chars = content.len() as i64;
    let prompt_tokens = approximate_tokens_from_chars(prompt_chars);
    let completion_tokens = approximate_tokens_from_chars(completion_chars);
    ChatCompletionUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}

fn approximate_tokens_from_chars(chars: i64) -> i64 {
    ((chars.max(1) + 3) / 4).max(1)
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessageResponse {
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    refusal: Option<Value>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
}

impl ChatMessageResponse {
    fn content_text(&self) -> String {
        if let Some(content) = &self.content {
            match content {
                Value::String(text) => return text.clone(),
                Value::Array(parts) => {
                    let text = parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::Object(map) => map
                                .get("text")
                                .and_then(Value::as_str)
                                .or_else(|| map.get("content").and_then(Value::as_str)),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
                Value::Null => {}
                other => return other.to_string(),
            }
        }

        if let Some(refusal) = &self.refusal {
            match refusal {
                Value::String(text) if !text.trim().is_empty() => return text.clone(),
                Value::Array(parts) => {
                    let text = parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::Object(map) => map.get("text").and_then(Value::as_str),
                            Value::String(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
                other if !other.is_null() => return other.to_string(),
                _ => {}
            }
        }

        if let Some(tool_calls) = &self.tool_calls
            && !tool_calls.is_empty()
        {
            return serde_json::to_string(tool_calls).unwrap_or_default();
        }

        String::new()
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<i64>,
    #[serde(default)]
    output_tokens: Option<i64>,
}

/// Shared exponential backoff configuration for LLM retries.
/// Initial: 750ms, multiplier: 2x, jitter: 15%, max interval: 8s, max elapsed: 120s.
pub(super) fn llm_retry_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(std::time::Duration::from_millis(750))
        .with_multiplier(2.0)
        .with_randomization_factor(0.15)
        .with_max_interval(std::time::Duration::from_secs(8))
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(120)))
        .build()
}
