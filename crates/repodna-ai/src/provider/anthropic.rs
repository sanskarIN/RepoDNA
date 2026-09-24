//! The Anthropic Messages API, called over HTTPS without an SDK.
//!
//! Requests contain the model, the output limit, the instructions as the system prompt,
//! and one user message. Thinking is left at the model's default, so the output limit
//! (16000 unless configured) also leaves room for reasoning tokens. Only `text` blocks of
//! the reply are read.

use serde_json::{Value, json};

use crate::error::AiError;
use crate::provider::http::{Client, Endpoint};
use crate::provider::{AiProvider, Completion, CompletionRequest};

/// Default API base URL.
pub const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com";
/// Model used when `ai.model` is not set.
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5";
/// API version header value.
const API_VERSION: &str = "2023-06-01";

/// Talks to `<endpoint>/v1/messages`.
pub struct AnthropicProvider {
    endpoint: Endpoint,
    url: String,
    model: String,
    api_key: Option<String>,
    client: Client,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("url", &self.url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "(set)"))
            .finish()
    }
}

impl AnthropicProvider {
    /// Creates a provider. `api_key` is sent in the `x-api-key` header when present.
    pub fn new(
        endpoint: Endpoint,
        model: String,
        api_key: Option<String>,
        timeout_seconds: u64,
    ) -> Self {
        let client = Client::new(&endpoint, timeout_seconds);
        let url = if endpoint.base.ends_with("/v1") {
            endpoint.url("/messages")
        } else {
            endpoint.url("/v1/messages")
        };
        Self {
            endpoint,
            url,
            model,
            api_key,
            client,
        }
    }
}

impl AiProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn remote(&self) -> bool {
        !self.endpoint.loopback
    }

    fn destination(&self) -> String {
        self.url.clone()
    }

    fn complete(&self, request: &CompletionRequest<'_>) -> Result<Completion, AiError> {
        let mut headers: Vec<(&str, &str)> = vec![("anthropic-version", API_VERSION)];
        if let Some(key) = &self.api_key {
            headers.push(("x-api-key", key));
        }
        let body = json!({
            "model": self.model,
            "max_tokens": request.max_output_tokens,
            "system": request.system,
            "messages": [{"role": "user", "content": request.user}],
        });
        let value = self
            .client
            .post(&self.url, &headers, &body)?
            .check()?
            .json()?;
        match value.get("stop_reason").and_then(Value::as_str) {
            Some("refusal") => return Err(AiError::Refused),
            Some("max_tokens") => {
                return Err(AiError::Truncated {
                    limit: request.max_output_tokens,
                });
            }
            _ => {}
        }
        let text: String = value
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(AiError::InvalidResponse(
                "the response contains no text".to_owned(),
            ));
        }
        Ok(Completion {
            text,
            input_tokens: value.pointer("/usage/input_tokens").and_then(Value::as_u64),
            output_tokens: value
                .pointer("/usage/output_tokens")
                .and_then(Value::as_u64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::http::fake;

    fn request() -> CompletionRequest<'static> {
        CompletionRequest {
            system: "rules",
            user: "task",
            max_output_tokens: 16_000,
            cancel: None,
        }
    }

    #[test]
    fn sends_a_messages_request_and_reads_text_blocks() {
        let (base, seen) = fake::serve(vec![(
            200,
            "",
            r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"thinking","thinking":"...","signature":"s"},{"type":"text","text":"{\"summary\":\"ok\"}"}],"stop_reason":"end_turn","usage":{"input_tokens":2100,"output_tokens":310}}"#.to_owned(),
        )]);
        let provider = AnthropicProvider::new(
            Endpoint::parse(&base).unwrap(),
            DEFAULT_ANTHROPIC_MODEL.into(),
            Some("key-value".into()),
            10,
        );
        assert!(!format!("{provider:?}").contains("key-value"));
        let completion = provider.complete(&request()).unwrap();
        assert_eq!(completion.text, r#"{"summary":"ok"}"#);
        assert_eq!(completion.input_tokens, Some(2100));
        assert_eq!(completion.output_tokens, Some(310));
        let sent = seen.recv().unwrap();
        assert_eq!(sent.path, "/v1/messages");
        assert_eq!(sent.header("x-api-key"), Some("key-value"));
        assert_eq!(sent.header("anthropic-version"), Some(API_VERSION));
        let body: Value = serde_json::from_str(&sent.body).unwrap();
        assert_eq!(body["model"], DEFAULT_ANTHROPIC_MODEL);
        assert_eq!(body["max_tokens"], 16_000);
        assert_eq!(body["system"], "rules");
        assert_eq!(body["messages"][0]["role"], "user");
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn reports_refusals_truncation_and_errors() {
        let (base, _seen) = fake::serve(vec![
            (
                200,
                "",
                r#"{"content":[],"stop_reason":"refusal","usage":{"input_tokens":1,"output_tokens":0}}"#.to_owned(),
            ),
            (
                200,
                "",
                r#"{"content":[{"type":"text","text":"{\"sum"}],"stop_reason":"max_tokens"}"#.to_owned(),
            ),
            (
                404,
                "",
                r#"{"type":"error","error":{"type":"not_found_error","message":"model: nope"}}"#.to_owned(),
            ),
        ]);
        let provider = AnthropicProvider::new(
            Endpoint::parse(&format!("{base}/v1")).unwrap(),
            "nope".into(),
            None,
            10,
        );
        assert!(matches!(
            provider.complete(&request()),
            Err(AiError::Refused)
        ));
        assert!(matches!(
            provider.complete(&request()),
            Err(AiError::Truncated { limit: 16_000 })
        ));
        assert!(matches!(
            provider.complete(&request()),
            Err(AiError::Http { status: 404, ref message }) if message == "model: nope"
        ));
    }
}
