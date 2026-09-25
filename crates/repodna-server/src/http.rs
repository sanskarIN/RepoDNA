//! Small HTTP helpers: URL parsing, cookies, and responses with security headers.

use std::io::Cursor;

use tiny_http::{Header, Response, StatusCode};

/// A response body with its content type and extra headers.
pub type Reply = Response<Cursor<Vec<u8>>>;

/// Content Security Policy for the web interface.
pub const APP_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Policy for generated reports, which carry their own stricter policy in the document.
pub const REPORT_CSP: &str = "frame-ancestors 'none'";

fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

/// Builds a response with the security headers every response carries.
pub fn respond(status: u16, content_type: &str, body: Vec<u8>, csp: &str) -> Reply {
    // Bodies are in memory, so send their length instead of chunked encoding.
    let mut response = Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_chunked_threshold(usize::MAX);
    for (name, value) in [
        ("Content-Type", content_type),
        ("Content-Security-Policy", csp),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "no-referrer"),
        ("X-Frame-Options", "DENY"),
        ("Cross-Origin-Opener-Policy", "same-origin"),
        ("Cross-Origin-Resource-Policy", "same-origin"),
        ("Cache-Control", "no-store"),
        ("Server", "RepoDNA"),
    ] {
        if let Some(header) = header(name, value) {
            response.add_header(header);
        }
    }
    response
}

/// A JSON response.
pub fn json(status: u16, value: &serde_json::Value) -> Reply {
    let mut body = serde_json::to_vec_pretty(value).unwrap_or_default();
    body.push(b'\n');
    respond(status, "application/json; charset=utf-8", body, APP_CSP)
}

/// A JSON error: `{"error": message}`.
pub fn error(status: u16, message: &str) -> Reply {
    json(status, &serde_json::json!({ "error": message }))
}

/// Adds a header to a response.
pub fn with_header(mut reply: Reply, name: &str, value: &str) -> Reply {
    if let Some(header) = header(name, value) {
        reply.add_header(header);
    }
    reply
}

/// Decodes `%XX` escapes and `+` in a query component.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(value) => {
                        out.push(value);
                        index += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A parsed request target: the decoded path segments and query parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The raw path.
    pub path: String,
    /// Decoded, non-empty path segments.
    pub segments: Vec<String>,
    /// Decoded query parameters in order.
    pub query: Vec<(String, String)>,
}

impl Target {
    /// Parses a request URL such as `/api/repositories/x?format=html`.
    pub fn parse(url: &str) -> Self {
        let (path, query) = url.split_once('?').unwrap_or((url, ""));
        let path = path.split('#').next().unwrap_or_default().to_owned();
        let segments = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(percent_decode)
            .collect();
        let query = query
            .split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                (percent_decode(key), percent_decode(value))
            })
            .collect();
        Self {
            path,
            segments,
            query,
        }
    }

    /// The first value of a query parameter.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Finds a cookie value in a `Cookie` header.
pub fn cookie<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        (key == name).then_some(value)
    })
}

/// Compares two strings in time that does not depend on where they differ.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets_and_cookies() {
        let target = Target::parse("/api/repositories/My%20Repo/report?format=html&x=a%2Bb+c&flag");
        assert_eq!(target.path, "/api/repositories/My%20Repo/report");
        assert_eq!(
            target.segments,
            vec!["api", "repositories", "My Repo", "report"]
        );
        assert_eq!(target.param("format"), Some("html"));
        assert_eq!(target.param("x"), Some("a+b c"));
        assert_eq!(target.param("flag"), Some(""));
        assert_eq!(target.param("missing"), None);
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(
            cookie("a=1; repodna_token=abc; b=2", "repodna_token"),
            Some("abc")
        );
        assert_eq!(cookie("a=1", "repodna_token"), None);
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }
}
