#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    #[serde(default)]
    pub model: Option<String>,
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionUsage {
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
}

pub fn estimate_chat_completion_usage(
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

pub fn approximate_tokens_from_chars(chars: i64) -> i64 {
    ((chars.max(1) + 3) / 4).max(1)
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessageResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessageResponse {
    #[serde(default)]
    pub content: Option<Value>,
    #[serde(default)]
    pub refusal: Option<Value>,
    #[serde(default)]
    pub tool_calls: Option<Vec<Value>>,
}

impl ChatMessageResponse {
    pub fn content_text(&self) -> String {
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
pub struct AnthropicResponse {
    pub content: Vec<AnthropicContentBlock>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicContentBlock {
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default)]
    pub input_tokens: Option<i64>,
    #[serde(default)]
    pub output_tokens: Option<i64>,
}

/// Shared exponential backoff configuration for LLM retries.
/// Initial: 750ms, multiplier: 2x, jitter: 15%, max interval: 8s, max elapsed: 120s.
pub fn llm_retry_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(std::time::Duration::from_millis(750))
        .with_multiplier(2.0)
        .with_randomization_factor(0.15)
        .with_max_interval(std::time::Duration::from_secs(8))
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(120)))
        .build()
}

