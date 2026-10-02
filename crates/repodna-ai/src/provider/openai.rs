//! OpenAI-compatible chat completion servers: local runtimes such as Ollama, llama.cpp,
//! LM Studio, and vLLM, or hosted services that implement the same API.

use serde_json::{Value, json};

use crate::error::AiError;
use crate::provider::http::{Client, Endpoint};
use crate::provider::{AiProvider, Completion, CompletionRequest};

/// Talks to `<endpoint>/chat/completions`.
pub struct OpenAiCompatibleProvider {
    endpoint: Endpoint,
    url: String,
    model: String,
    api_key: Option<String>,
    client: Client,
}

impl std::fmt::Debug for OpenAiCompatibleProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatibleProvider")
            .field("url", &self.url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "(set)"))
            .finish()
    }
}

impl OpenAiCompatibleProvider {
    /// Creates a provider. `api_key` is sent as a bearer token when present.
    pub fn new(
        endpoint: Endpoint,
        model: String,
        api_key: Option<String>,
        timeout_seconds: u64,
    ) -> Self {
        let client = Client::new(&endpoint, timeout_seconds);
        Self {
            url: endpoint.url("/chat/completions"),
            endpoint,
            model,
            api_key,
            client,
        }
    }
}

/// Reads the text of a message's `content`, which is a string or a list of parts.
fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

impl AiProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &'static str {
        "openai-compatible"
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
        let authorization = self.api_key.as_ref().map(|key| format!("Bearer {key}"));
        let mut headers: Vec<(&str, &str)> = Vec::new();
        if let Some(value) = &authorization {
            headers.push(("authorization", value));
        }
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
            "max_tokens": request.max_output_tokens,
            "stream": false,
        });
        let mut reply = self.client.post(&self.url, &headers, &body)?;
        // Some hosted models accept only `max_completion_tokens`.
        if reply.status == 400 && reply.body.contains("max_completion_tokens") {
            if let Some(object) = body.as_object_mut() {
                object.remove("max_tokens");
                object.insert(
                    "max_completion_tokens".to_owned(),
                    json!(request.max_output_tokens),
                );
            }
            reply = self.client.post(&self.url, &headers, &body)?;
        }
        let value = reply.check()?.json()?;
        let choice = value
            .pointer("/choices/0")
            .ok_or_else(|| AiError::InvalidResponse("the response has no choices".to_owned()))?;
        let text = content_text(choice.pointer("/message/content").unwrap_or(&Value::Null));
        if text.trim().is_empty() {
            if choice
                .pointer("/message/refusal")
                .and_then(Value::as_str)
                .is_some_and(|refusal| !refusal.is_empty())
            {
                return Err(AiError::Refused);
            }
            if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
                return Err(AiError::Truncated {
                    limit: request.max_output_tokens,
                });
            }
            return Err(AiError::InvalidResponse(
                "the response contains no text".to_owned(),
            ));
        }
        if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
            return Err(AiError::Truncated {
                limit: request.max_output_tokens,
            });
        }
        Ok(Completion {
            text,
            input_tokens: value
                .pointer("/usage/prompt_tokens")
                .and_then(Value::as_u64),
            output_tokens: value
                .pointer("/usage/completion_tokens")
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
            max_output_tokens: 700,
            cancel: None,
        }
    }

    #[test]
    fn sends_chat_completions_and_reads_the_answer() {
        let (base, seen) = fake::serve(vec![(
            200,
            "",
            r#"{"choices":[{"message":{"role":"assistant","content":"{\"summary\":\"ok\"}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":120,"completion_tokens":9}}"#.to_owned(),
        )]);
        let provider = OpenAiCompatibleProvider::new(
            Endpoint::parse(&format!("{base}/v1")).unwrap(),
            "llama3.2".into(),
            Some("secret".into()),
            10,
        );
        assert!(!provider.remote());
        assert!(!format!("{provider:?}").contains("secret"));
        let completion = provider.complete(&request()).unwrap();
        assert_eq!(completion.text, r#"{"summary":"ok"}"#);
        assert_eq!(completion.input_tokens, Some(120));
        assert_eq!(completion.output_tokens, Some(9));
        let sent = seen.recv().unwrap();
        assert_eq!(sent.path, "/v1/chat/completions");
        assert_eq!(sent.header("authorization"), Some("Bearer secret"));
        let body: Value = serde_json::from_str(&sent.body).unwrap();
        assert_eq!(body["model"], "llama3.2");
        assert_eq!(body["max_tokens"], 700);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "task");
    }

    #[test]
    fn falls_back_to_max_completion_tokens_and_detects_truncation() {
        let (base, seen) = fake::serve(vec![
            (
                400,
                "",
                r#"{"error":{"message":"Unsupported parameter: 'max_tokens'. Use 'max_completion_tokens' instead."}}"#.to_owned(),
            ),
            (
                200,
                "",
                r#"{"choices":[{"message":{"content":[{"type":"text","text":"{\"summ"}]},"finish_reason":"length"}]}"#.to_owned(),
            ),
        ]);
        let provider =
            OpenAiCompatibleProvider::new(Endpoint::parse(&base).unwrap(), "m".into(), None, 10);
        let error = provider.complete(&request()).unwrap_err();
        assert!(matches!(error, AiError::Truncated { limit: 700 }));
        let first = seen.recv().unwrap();
        assert_eq!(first.header("authorization"), None);
        let second: Value = serde_json::from_str(&seen.recv().unwrap().body).unwrap();
        assert_eq!(second["max_completion_tokens"], 700);
        assert!(second.get("max_tokens").is_none());
    }

    #[test]
    fn reports_refusals() {
        let (base, _seen) = fake::serve(vec![(
            200,
            "",
            r#"{"choices":[{"message":{"content":null,"refusal":"I can't help with that."},"finish_reason":"stop"}]}"#.to_owned(),
        )]);
        let provider =
            OpenAiCompatibleProvider::new(Endpoint::parse(&base).unwrap(), "m".into(), None, 10);
        assert!(matches!(
            provider.complete(&request()),
            Err(AiError::Refused)
        ));
    }
}
