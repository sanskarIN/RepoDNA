//! Shared HTTP plumbing: endpoint checks, requests, retries, and error messages.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::error::AiError;
use crate::text::single_line;

/// Retries after a rate-limit or overload response.
const MAX_RETRIES: u32 = 2;
/// Longest wait honored from a `retry-after` header, in seconds.
const MAX_RETRY_AFTER: u64 = 30;
/// Largest response body read, in bytes.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// A validated provider endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Base URL without a trailing slash.
    pub base: String,
    /// Host name, lowercase.
    pub host: String,
    /// `true` when the host is this machine.
    pub loopback: bool,
}

/// Returns `true` for host names and addresses that refer to this machine.
pub(crate) fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host.parse::<Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
        || host.parse::<Ipv6Addr>().is_ok_and(|ip| ip.is_loopback())
}

impl Endpoint {
    /// Validates an endpoint URL. Plain `http` is accepted only for this machine, and
    /// credentials, queries, and fragments are rejected.
    pub fn parse(raw: &str) -> Result<Self, AiError> {
        let invalid = |why: &str| AiError::Configuration(format!("ai.endpoint `{raw}` {why}"));
        let base = raw.trim().trim_end_matches('/');
        let (scheme, rest) = base
            .split_once("://")
            .ok_or_else(|| invalid("must start with https:// or http://"))?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "https" && scheme != "http" {
            return Err(invalid("must start with https:// or http://"));
        }
        if rest.contains(['?', '#']) {
            return Err(invalid("must not contain a query or fragment"));
        }
        let authority = rest.split('/').next().unwrap_or_default();
        if authority.contains('@') {
            return Err(invalid(
                "must not contain credentials; name an environment variable in ai.api_key_env instead",
            ));
        }
        let host = match authority.strip_prefix('[') {
            Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
            None => authority.split(':').next().unwrap_or_default(),
        }
        .to_ascii_lowercase();
        if host.is_empty() {
            return Err(invalid("has no host"));
        }
        let loopback = is_loopback(&host);
        if scheme == "http" && !loopback {
            return Err(invalid(
                "must use https; plain http is allowed only for localhost",
            ));
        }
        let path = &rest[authority.len()..];
        Ok(Self {
            base: format!("{scheme}://{}{path}", authority.to_ascii_lowercase()),
            host,
            loopback,
        })
    }

    /// The URL for `path` below the base, unless the base already ends with it.
    pub(crate) fn url(&self, path: &str) -> String {
        if self.base.ends_with(path) {
            self.base.clone()
        } else {
            format!("{}{path}", self.base)
        }
    }
}

/// A response: status, body, and the `retry-after` hint.
#[derive(Debug)]
pub(crate) struct Reply {
    pub status: u16,
    pub body: String,
    pub retry_after: Option<u64>,
}

impl Reply {
    /// Parses the body as JSON.
    pub(crate) fn json(&self) -> Result<Value, AiError> {
        serde_json::from_str(&self.body).map_err(|error| {
            AiError::InvalidResponse(format!("the response is not JSON ({error})"))
        })
    }

    /// Converts an error status into an error with the provider's message.
    pub(crate) fn check(self) -> Result<Self, AiError> {
        if (200..300).contains(&self.status) {
            return Ok(self);
        }
        let mut message = error_message(&self.body);
        if matches!(self.status, 401 | 403) {
            message.push_str(" (check the API key and its permissions)");
        }
        Err(AiError::Http {
            status: self.status,
            message,
        })
    }
}

/// Extracts a readable message from an error body.
pub(crate) fn error_message(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed.as_ref().and_then(|value| {
        value
            .pointer("/error/message")
            .or_else(|| value.get("error"))
            .or_else(|| value.get("message"))
            .or_else(|| value.get("detail"))
            .and_then(Value::as_str)
    });
    let text = single_line(message.unwrap_or(body), 300);
    if text.is_empty() {
        "no error message".to_owned()
    } else {
        text
    }
}

/// A blocking JSON client for one endpoint.
pub(crate) struct Client {
    agent: ureq::Agent,
    timeout: u64,
}

impl Client {
    /// Creates a client. Requests to this machine bypass any configured proxy.
    pub(crate) fn new(endpoint: &Endpoint, timeout_seconds: u64) -> Self {
        let mut config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_seconds.max(1))))
            .http_status_as_error(false)
            .user_agent(format!("RepoDNA/{}", env!("CARGO_PKG_VERSION")));
        if endpoint.loopback {
            config = config.proxy(None);
        }
        Self {
            agent: config.build().into(),
            timeout: timeout_seconds,
        }
    }

    fn post_once(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Reply, AiError> {
        let mut request = self
            .agent
            .post(url)
            .header("content-type", "application/json");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let mut response = request.send(body).map_err(|error| match error {
            ureq::Error::Timeout(_) => AiError::Timeout(self.timeout),
            other => AiError::Transport(other.to_string()),
        })?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok());
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .lossy_utf8(true)
            .read_to_string()
            .map_err(|error| match error {
                ureq::Error::Timeout(_) => AiError::Timeout(self.timeout),
                other => AiError::Transport(other.to_string()),
            })?;
        Ok(Reply {
            status,
            body,
            retry_after,
        })
    }

    /// Posts JSON, retrying rate-limit and overload responses with backoff.
    pub(crate) fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &Value,
    ) -> Result<Reply, AiError> {
        let body = body.to_string();
        let mut attempt = 0;
        loop {
            let reply = self.post_once(url, headers, &body)?;
            let retryable = matches!(reply.status, 408 | 429 | 500 | 502 | 503 | 504 | 529);
            if !retryable || attempt == MAX_RETRIES {
                return Ok(reply);
            }
            let wait = reply
                .retry_after
                .map_or(2 << attempt, |seconds| seconds.min(MAX_RETRY_AFTER));
            thread::sleep(Duration::from_secs(wait));
            attempt += 1;
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A one-connection-per-response HTTP server on 127.0.0.1 for provider tests.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// A request as the server saw it.
    pub(crate) struct Seen {
        pub path: String,
        pub headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Seen {
        pub(crate) fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    /// Serves `responses` (status, extra headers, body) in order; returns the base URL
    /// and a receiver of the requests.
    pub(crate) fn serve(
        responses: Vec<(u16, &'static str, String)>,
    ) -> (String, mpsc::Receiver<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for (status, extra, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                let mut headers = Vec::new();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            length = value.trim().parse().unwrap();
                        }
                        headers.push((name.to_owned(), value.trim().to_owned()));
                    }
                }
                let mut buffer = vec![0; length];
                reader.read_exact(&mut buffer).unwrap();
                let _ = sender.send(Seen {
                    path,
                    headers,
                    body: String::from_utf8(buffer).unwrap(),
                });
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        (format!("http://{address}"), receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_endpoints() {
        let local = Endpoint::parse("http://localhost:11434/v1/").unwrap();
        assert_eq!(local.base, "http://localhost:11434/v1");
        assert_eq!(local.host, "localhost");
        assert!(local.loopback);
        assert!(Endpoint::parse("http://[::1]:8080").unwrap().loopback);
        assert!(Endpoint::parse("http://127.0.0.2").unwrap().loopback);
        let remote = Endpoint::parse("HTTPS://API.Example.com/v1").unwrap();
        assert_eq!(remote.host, "api.example.com");
        assert!(!remote.loopback);
        for bad in [
            "api.example.com",
            "ftp://example.com",
            "http://example.com",
            "https://user:pass@example.com",
            "https://example.com/v1?key=1",
            "https://",
        ] {
            assert!(Endpoint::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(
            remote.url("/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
        let full = Endpoint::parse("http://127.0.0.1/v1/chat/completions").unwrap();
        assert_eq!(
            full.url("/chat/completions"),
            "http://127.0.0.1/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_error_messages() {
        assert_eq!(
            error_message(r#"{"type":"error","error":{"type":"x","message":"Overloaded"}}"#),
            "Overloaded"
        );
        assert_eq!(
            error_message(r#"{"error":"model not found"}"#),
            "model not found"
        );
        assert_eq!(error_message("plain failure\n"), "plain failure");
        assert_eq!(error_message(""), "no error message");
    }

    #[test]
    fn retries_overloaded_responses() {
        let (base, seen) = fake::serve(vec![
            (
                529,
                "retry-after: 0\r\n",
                r#"{"error":{"message":"busy"}}"#.to_owned(),
            ),
            (200, "", r#"{"ok":true}"#.to_owned()),
        ]);
        let endpoint = Endpoint::parse(&base).unwrap();
        let client = Client::new(&endpoint, 10);
        let reply = client
            .post(
                &endpoint.url("/x"),
                &[("x-test", "1")],
                &serde_json::json!({"a": 1}),
            )
            .unwrap()
            .check()
            .unwrap();
        assert_eq!(reply.json().unwrap()["ok"], true);
        let first = seen.recv().unwrap();
        assert_eq!(first.path, "/x");
        assert_eq!(first.header("x-test"), Some("1"));
        assert_eq!(first.body, r#"{"a":1}"#);
        assert!(seen.recv().is_ok());
    }

    #[test]
    fn reports_error_statuses() {
        let (base, _seen) = fake::serve(vec![(
            401,
            "",
            r#"{"error":{"message":"invalid x-api-key"}}"#.to_owned(),
        )]);
        let endpoint = Endpoint::parse(&base).unwrap();
        let error = Client::new(&endpoint, 10)
            .post(&endpoint.url("/x"), &[], &serde_json::json!({}))
            .unwrap()
            .check()
            .unwrap_err();
        assert!(
            matches!(error, AiError::Http { status: 401, ref message } if message.contains("invalid x-api-key") && message.contains("check the API key"))
        );
    }
}
