use serde::Deserialize;

use crate::scoring::score_types::{DimensionScore, ScoreReliability};

/// Sentiment scoring via LLM. Takes news headlines and returns a score.
pub async fn score_sentiment(
    llm: &crate::llm::LlmClient,
    symbol: &str,
    headlines: &[String],
    news_limit: usize,
) -> DimensionScore {
    if headlines.is_empty() {
        return DimensionScore {
            score: 50,
            reason: "无新闻数据，情绪中性".into(),
            reliability: ScoreReliability::Missing,
        };
    }

    let limited: Vec<&str> = headlines
        .iter()
        .take(news_limit)
        .map(String::as_str)
        .collect();
    let news_text = limited.join("\n- ");
    let prompt = format!(
        "给股票 {symbol} 的近期新闻情绪评分。评分范围 0-100，50=中性，>70 积极，<30 消极。\n\
         只输出 JSON，不要其他内容：\n\
         {{\"score\": <0-100>, \"reason\": \"一句话理由\"}}\n\n\
         新闻标题：\n- {news_text}"
    );

    let content = match llm.generate(&prompt).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(symbol = %symbol, error = %e, "sentiment LLM call failed");
            return DimensionScore {
                score: 50,
                reason: format!("情绪分析LLM调用失败: {e}"),
                reliability: ScoreReliability::Missing,
            };
        }
    };

    parse_sentiment_response(&content)
}

#[derive(Deserialize)]
struct SentimentResponse {
    score: u8,
    reason: String,
}

/// Parse an LLM sentiment response JSON into a dimension score.
pub fn parse_sentiment_response(content: &str) -> DimensionScore {
    let json_str = content
        .trim()
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
        .or_else(|| {
            content
                .strip_prefix("```")
                .and_then(|s| s.strip_suffix("```"))
        })
        .unwrap_or(content.trim());

    match serde_json::from_str::<SentimentResponse>(json_str) {
        Ok(resp) => DimensionScore {
            score: resp.score.clamp(0, 100),
            reason: resp.reason,
            reliability: ScoreReliability::High,
        },
        Err(e) => {
            tracing::warn!(error = %e, raw = %content, "failed to parse sentiment JSON");
            DimensionScore {
                score: 50,
                reason: "情绪分析解析失败，使用中性评分".into(),
                reliability: ScoreReliability::Missing,
            }
        }
    }
}
