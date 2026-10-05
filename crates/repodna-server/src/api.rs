//! Request handling: the Host and Origin checks, authentication, the JSON API, reports,
//! and the web interface.

use std::io::Read;

use repodna_app::{AnalyzeOptions, AppPaths};
use repodna_core::config::{AnalysisProfile, PrivacyPreset};
use repodna_core::model::artifact::RepositoryDna;
use repodna_report::card::{CardOptions, render as render_card};
use repodna_report::compare;
use repodna_report::compare::json_comparison;
use repodna_store::Store;
use serde_json::{Value, json};
use tiny_http::{Method, Request};

use crate::assets::Assets;
use crate::http::{
    APP_CSP, REPORT_CSP, Reply, Target, constant_time_eq, cookie, error, json, respond, with_header,
};
use crate::jobs::Jobs;
use crate::library::{self, LookupError};

/// Name of the session cookie.
pub const COOKIE: &str = "repodna_token";
/// Largest request body accepted.
const MAX_BODY: u64 = 64 * 1024;

/// Everything request handlers share.
pub struct State {
    /// Storage and configuration locations.
    pub paths: AppPaths,
    /// Local storage.
    pub store: Store,
    /// The session token.
    pub token: String,
    /// The port the server listens on.
    pub port: u16,
    /// The web interface.
    pub assets: Assets,
    /// Background analyses.
    pub jobs: Jobs,
    /// Allow starting analyses through the API.
    pub allow_scans: bool,
    /// Configuration for analyses started through the API.
    pub template: AnalyzeOptions,
}

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

impl State {
    /// Accepts only this machine's names for the server, which defeats DNS rebinding.
    fn host_allowed(&self, host: Option<&str>) -> bool {
        host.is_some_and(|host| {
            let host = host.to_ascii_lowercase();
            host == format!("127.0.0.1:{}", self.port) || host == format!("localhost:{}", self.port)
        })
    }

    /// Requests from pages served elsewhere are refused.
    fn origin_allowed(&self, origin: Option<&str>) -> bool {
        origin.is_none_or(|origin| {
            let origin = origin.to_ascii_lowercase();
            origin == format!("http://127.0.0.1:{}", self.port)
                || origin == format!("http://localhost:{}", self.port)
        })
    }

    fn authenticated(&self, request: &Request) -> bool {
        let presented = header(request, "X-RepoDNA-Token")
            .or_else(|| header(request, "Authorization").and_then(|v| v.strip_prefix("Bearer ")))
            .or_else(|| header(request, "Cookie").and_then(|v| cookie(v, COOKIE)));
        presented.is_some_and(|token| constant_time_eq(token.trim(), &self.token))
    }
}

fn lookup_error(failure: &LookupError) -> Reply {
    match failure {
        LookupError::NoRepository(_) | LookupError::NoScan => error(404, &failure.to_string()),
        LookupError::Storage(_) => error(500, &failure.to_string()),
    }
}

fn privacy(target: &Target) -> Result<PrivacyPreset, Reply> {
    library::parse_privacy(target.param("privacy").unwrap_or("local"))
        .map_err(|message| error(400, &message))
}

fn load(
    state: &State,
    query: &str,
    target: &Target,
) -> Result<(RepositoryDna, PrivacyPreset), Reply> {
    let preset = privacy(target)?;
    let dna = library::load(&state.store, query, target.param("scan"), preset)
        .map_err(|failure| lookup_error(&failure))?;
    Ok((dna, preset))
}

fn view(dna: &RepositoryDna, name: &str) -> Option<Value> {
    Some(match name {
        "architecture" => json!(dna.architecture),
        "dependencies" => json!(dna.dependencies),
        "history" => json!(dna.git),
        "hotspots" => json!({
            "hotspots": dna.git.hot_spots,
            "complexity": dna.code_quality.complexity,
        }),
        "timeline" => json!(dna.evolution),
        "findings" => json!(dna.findings),
        "insights" => json!(dna.insights),
        "fingerprint" => json!(dna.fingerprint),
        "metrics" => json!(dna.metrics),
        "structure" => json!(dna.structure),
        "languages" => json!(dna.languages),
        _ => return None,
    })
}

fn report(state: &State, query: &str, target: &Target) -> Reply {
    let (dna, preset) = match load(state, query, target) {
        Ok(loaded) => loaded,
        Err(reply) => return reply,
    };
    let theme = match library::parse_theme(target.param("theme").unwrap_or("professional")) {
        Ok(theme) => theme,
        Err(message) => return error(400, &message),
    };
    let format = target.param("format").unwrap_or("html");
    match library::report(&dna, format, theme, preset) {
        Ok(document) => respond(
            200,
            document.content_type,
            document.text.into_bytes(),
            if format == "html" {
                REPORT_CSP
            } else {
                APP_CSP
            },
        ),
        Err(message) if message.starts_with("unknown report format") => error(400, &message),
        Err(message) => error(500, &message),
    }
}

fn compare_route(state: &State, target: &Target) -> Reply {
    let queries: Vec<&str> = target
        .param("repositories")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .collect();
    if queries.len() < 2 {
        return error(400, "name at least two repositories: ?repositories=a,b");
    }
    let mut artifacts = Vec::new();
    for query in queries {
        match load(state, query, target) {
            Ok((dna, _)) => artifacts.push(dna),
            Err(reply) => return reply,
        }
    }
    json(200, &json_comparison(&compare(&artifacts)))
}

fn start_scan(state: &State, request: &mut Request) -> Reply {
    if !state.allow_scans {
        return error(
            403,
            "starting analyses is turned off for this server (--no-scan)",
        );
    }
    let mut body = String::new();
    if request
        .as_reader()
        .take(MAX_BODY + 1)
        .read_to_string(&mut body)
        .is_err()
        || body.len() as u64 > MAX_BODY
    {
        return error(413, "the request body is too large or not UTF-8");
    }
    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return error(
            400,
            "send a JSON object such as {\"input\": \"/path/to/repository\"}",
        );
    };
    let Some(input) = value
        .get("input")
        .and_then(Value::as_str)
        .filter(|i| !i.trim().is_empty())
    else {
        return error(400, "`input` must name a directory, archive, or Git URL");
    };
    let mut options = state.template.clone();
    options.input = input.trim().to_owned();
    if let Some(profile) = value.get("profile").and_then(Value::as_str) {
        match profile.parse::<AnalysisProfile>() {
            Ok(profile) => options.config.overrides.profile = Some(profile),
            Err(message) => return error(400, &message),
        }
    }
    match state.jobs.start(state.paths.clone(), options) {
        Ok(job) => json(202, &json!(job)),
        Err(message) => error(429, &message),
    }
}

fn escape(text: &str) -> String {
    repodna_report::text::escape_html(text)
}

/// The built-in page shown when this build has no web interface.
fn builtin_page(state: &State, authenticated: bool) -> Reply {
    let mut body = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>RepoDNA</title><style>body{font:16px/1.5 system-ui,sans-serif;max-width:760px;margin:40px auto;padding:0 16px;color:#1f2328;background:#fff}a{color:#0b5cad}li{margin:6px 0}small{color:#59636e}@media (prefers-color-scheme:dark){body{background:#0d1117;color:#e6edf3}a{color:#6cb6ff}small{color:#9198a1}}</style></head><body><h1>RepoDNA</h1>",
    );
    if !authenticated {
        body.push_str("<p>Open the link that <code>repodna serve</code> printed in your terminal. It contains a token that proves the request comes from you.</p></body></html>");
        return respond(401, "text/html; charset=utf-8", body.into_bytes(), APP_CSP);
    }
    body.push_str("<p>This build does not include the interactive web interface. Stored analyses and their reports are listed below.</p>");
    match state.store.repositories() {
        Ok(repositories) if repositories.is_empty() => body.push_str(
            "<p>No stored analyses yet. Run <code>repodna analyze &lt;path&gt;</code>, then reload this page.</p>",
        ),
        Ok(repositories) => {
            body.push_str("<ul>");
            for r in repositories {
                let id = escape(&r.id);
                body.push_str(&format!(
                    "<li><a href=\"/api/repositories/{id}/report\">{}</a> <small>{} · {} analyses · latest {} · <a href=\"/api/repositories/{id}/artifact\">JSON</a></small></li>",
                    escape(&r.name),
                    escape(&r.location),
                    r.scans,
                    r.last_scanned_at.date_string()
                ));
            }
            body.push_str("</ul>");
        }
        Err(e) => body.push_str(&format!("<p>Local storage failed: {}</p>", escape(&e.to_string()))),
    }
    body.push_str("<p><small>RepoDNA · Made by the Sanskar · <a href=\"https://github.com/sanskarIN/RepoDNA\">github.com/sanskarIN/RepoDNA</a></small></p></body></html>");
    respond(200, "text/html; charset=utf-8", body.into_bytes(), APP_CSP)
}

/// Handles one request.
pub fn handle(state: &State, request: &mut Request) -> Reply {
    if !state.host_allowed(header(request, "Host")) {
        return error(
            403,
            "unexpected Host header; use the address printed by `repodna serve`",
        );
    }
    let target = Target::parse(request.url());
    let method = request.method().clone();
    let segments: Vec<&str> = target.segments.iter().map(String::as_str).collect();

    // Signing in: the printed link carries the token once; afterwards a cookie does.
    if method == Method::Get && segments.is_empty() && target.param("token").is_some() {
        let valid = target
            .param("token")
            .is_some_and(|token| constant_time_eq(token, &state.token));
        if !valid {
            return error(403, "the token in this link is not valid for this server");
        }
        let reply = respond(303, "text/plain; charset=utf-8", Vec::new(), APP_CSP);
        let reply = with_header(reply, "Location", "/");
        return with_header(
            reply,
            "Set-Cookie",
            &format!(
                "{COOKIE}={}; HttpOnly; SameSite=Strict; Path=/",
                state.token
            ),
        );
    }

    if segments.first() == Some(&"api") {
        if segments == ["api", "health"] {
            return json(
                200,
                &json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }),
            );
        }
        if !state.authenticated(request) {
            return error(
                401,
                "missing or invalid token; open the link printed by `repodna serve`",
            );
        }
        if method != Method::Get && !state.origin_allowed(header(request, "Origin")) {
            return error(403, "requests from other origins are refused");
        }
        return match (&method, &segments[1..]) {
            (Method::Get, ["session"]) => json(
                200,
                &json!({
                    "authenticated": true,
                    "version": env!("CARGO_PKG_VERSION"),
                    "allowScans": state.allow_scans,
                    "webInterface": state.assets.available(),
                }),
            ),
            (Method::Get, ["repositories"]) => match library::repositories(&state.store) {
                Ok(list) => json(200, &list),
                Err(failure) => lookup_error(&failure),
            },
            (Method::Get, ["repositories", query]) => {
                match library::repository_detail(&state.store, query) {
                    Ok(detail) => json(200, &detail),
                    Err(failure) => lookup_error(&failure),
                }
            }
            (Method::Get, ["repositories", query, "artifact"]) => match load(state, query, &target)
            {
                Ok((dna, _)) => json(200, &json!(dna)),
                Err(reply) => reply,
            },
            (Method::Get, ["repositories", query, "report"]) => report(state, query, &target),
            (Method::Get, ["repositories", query, "card.svg"]) => match load(state, query, &target)
            {
                Ok((dna, _)) => respond(
                    200,
                    "image/svg+xml",
                    render_card(
                        &dna,
                        CardOptions {
                            dark: target
                                .param("dark")
                                .is_some_and(|v| v == "1" || v == "true"),
                            branding: true,
                        },
                    )
                    .into_bytes(),
                    APP_CSP,
                ),
                Err(reply) => reply,
            },
            (Method::Get, ["repositories", query, name]) => match load(state, query, &target) {
                Ok((dna, _)) => match view(&dna, name) {
                    Some(value) => json(200, &value),
                    None => error(404, &format!("unknown view `{name}`")),
                },
                Err(reply) => reply,
            },
            (Method::Get, ["compare"]) => compare_route(state, &target),
            (Method::Get, ["scans"]) => json(200, &json!(state.jobs.list())),
            (Method::Post, ["scans"]) => start_scan(state, request),
            (Method::Get, ["scans", id]) => match state.jobs.get(id) {
                Some(job) => json(200, &json!(job)),
                None => error(404, "no such analysis job"),
            },
            (Method::Delete, ["scans", id]) => {
                if state.jobs.cancel(id) {
                    json(202, &json!({ "cancelling": id }))
                } else {
                    error(404, "no such analysis job")
                }
            }
            _ => error(404, "no such API endpoint"),
        };
    }

    if method != Method::Get && method != Method::Head {
        return error(405, "method not allowed");
    }
    if !state.assets.available() {
        return if segments.is_empty() {
            builtin_page(state, state.authenticated(request))
        } else {
            error(404, "not found")
        };
    }
    match state.assets.get(&target.path) {
        Some((bytes, content_type)) => respond(200, content_type, bytes, APP_CSP),
        None => error(404, "not found"),
    }
}
