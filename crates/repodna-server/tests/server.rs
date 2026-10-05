//! The server over a real socket: authentication, Host and Origin checks, the API,
//! reports, and background analyses.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use repodna_app::{AnalyzeOptions, AppPaths, ConfigOptions, run_analysis};
use repodna_core::CancellationToken;
use repodna_engine::Progress;
use repodna_server::{Server, ServerOptions};

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("a JSON body")
    }
}

fn send(
    address: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Response {
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        request.push_str(&format!("Host: 127.0.0.1:{}\r\n", address.port()));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Response {
        status,
        headers,
        body: body.to_owned(),
    }
}

struct Running {
    server: Arc<Server>,
    cancel: CancellationToken,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Running {
    fn start(paths: AppPaths, web_dir: Option<std::path::PathBuf>) -> Self {
        let server = Arc::new(
            Server::bind(
                paths,
                ServerOptions {
                    port: 0,
                    web_dir,
                    allow_scans: true,
                    template: AnalyzeOptions {
                        config: ConfigOptions {
                            ignore_user_config: true,
                            ..ConfigOptions::default()
                        },
                        ..AnalyzeOptions::default()
                    },
                    workers: 2,
                },
            )
            .expect("bind"),
        );
        let cancel = CancellationToken::new();
        let thread = {
            let server = Arc::clone(&server);
            let cancel = cancel.clone();
            std::thread::spawn(move || server.run(&cancel))
        };
        Self {
            server,
            cancel,
            thread: Some(thread),
        }
    }

    fn address(&self) -> SocketAddr {
        self.server.address()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn analyzed_home() -> (tempfile::TempDir, tempfile::TempDir, AppPaths) {
    let home = tempfile::tempdir().expect("home");
    let repo = tempfile::tempdir().expect("repo");
    repodna_testkit::write_tree(
        repo.path(),
        &[
            ("README.md", "# Gadget\n\nA gadget.\n"),
            (
                "src/app.py",
                "import os\n\ndef main():\n    return os.sep\n",
            ),
        ],
    );
    let paths = AppPaths::in_directory(home.path());
    run_analysis(
        &paths,
        &AnalyzeOptions {
            input: repo.path().to_string_lossy().into_owned(),
            config: ConfigOptions {
                ignore_user_config: true,
                ..ConfigOptions::default()
            },
            ..AnalyzeOptions::default()
        },
        Progress::default(),
        &CancellationToken::new(),
    )
    .expect("analysis");
    (home, repo, paths)
}

#[test]
fn protects_the_api_and_serves_analyses() {
    let (_home, repo, paths) = analyzed_home();
    let running = Running::start(paths, None);
    let address = running.address();
    let token = running.server.token().to_owned();
    let auth = [("X-RepoDNA-Token", token.as_str())];

    let health = send(address, "GET", "/api/health", &[], "");
    assert_eq!(health.status, 200);
    assert_eq!(health.header("X-Content-Type-Options"), Some("nosniff"));
    assert!(
        health
            .header("Content-Security-Policy")
            .unwrap()
            .contains("frame-ancestors 'none'")
    );

    assert_eq!(
        send(address, "GET", "/api/repositories", &[], "").status,
        401
    );
    let wrong = [("X-RepoDNA-Token", "0000")];
    assert_eq!(
        send(address, "GET", "/api/repositories", &wrong, "").status,
        401
    );
    let rebinding = [
        ("Host", "attacker.example:80"),
        ("X-RepoDNA-Token", token.as_str()),
    ];
    assert_eq!(
        send(address, "GET", "/api/repositories", &rebinding, "").status,
        403
    );

    let list = send(address, "GET", "/api/repositories", &auth, "");
    assert_eq!(list.status, 200, "{}", list.body);
    let repositories = list.json();
    let id = repositories[0]["id"].as_str().unwrap().to_owned();
    let bearer = format!("Bearer {token}");
    let detail = send(
        address,
        "GET",
        &format!("/api/repositories/{id}"),
        &[("Authorization", &bearer)],
        "",
    );
    assert_eq!(detail.status, 200);
    assert_eq!(detail.json()["scans"].as_array().unwrap().len(), 1);

    let artifact = send(
        address,
        "GET",
        &format!("/api/repositories/{id}/artifact?privacy=share"),
        &auth,
        "",
    );
    assert_eq!(artifact.json()["schemaVersion"], "1.0");
    let architecture = send(
        address,
        "GET",
        &format!("/api/repositories/{id}/architecture"),
        &auth,
        "",
    );
    assert!(architecture.json()["modules"].is_array());
    assert_eq!(
        send(
            address,
            "GET",
            &format!("/api/repositories/{id}/nope"),
            &auth,
            ""
        )
        .status,
        404
    );
    let html = send(
        address,
        "GET",
        &format!("/api/repositories/{id}/report"),
        &auth,
        "",
    );
    assert_eq!(html.status, 200);
    assert_eq!(
        html.header("Content-Type"),
        Some("text/html; charset=utf-8")
    );
    assert!(html.body.starts_with("<!doctype html>"));
    let card = send(
        address,
        "GET",
        &format!("/api/repositories/{id}/card.svg"),
        &auth,
        "",
    );
    assert_eq!(card.header("Content-Type"), Some("image/svg+xml"));
    assert_eq!(
        send(address, "GET", "/api/repositories/missing", &auth, "").status,
        404
    );
    let compare = send(
        address,
        "GET",
        &format!("/api/compare?repositories={id},{id}"),
        &auth,
        "",
    );
    assert_eq!(compare.status, 200, "{}", compare.body);

    // Signing in with the printed link sets a strict cookie that works afterwards.
    assert_eq!(send(address, "GET", "/?token=nope", &[], "").status, 403);
    let login = send(address, "GET", &format!("/?token={token}"), &[], "");
    assert_eq!(login.status, 303);
    let cookie = login.header("Set-Cookie").unwrap().to_owned();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let cookie_value = cookie.split(';').next().unwrap().to_owned();
    let page = send(address, "GET", "/", &[("Cookie", &cookie_value)], "");
    assert_eq!(page.status, 200);
    // The build embeds the web interface when it was built first; otherwise the server
    // shows its built-in page.
    let anonymous = send(address, "GET", "/", &[], "");
    if running.server.has_web_interface() {
        // The interface's files hold no data, so they need no token; the API does.
        assert!(page.body.contains(r#"<div id="root">"#), "{}", page.body);
        assert_eq!(anonymous.status, 200);
    } else {
        // The built-in page lists stored analyses, so it needs the token.
        assert!(
            page.body
                .contains("does not include the interactive web interface")
        );
        assert_eq!(anonymous.status, 401);
    }

    // Starting analyses: refused from other origins, accepted with the token.
    let evil = [
        ("X-RepoDNA-Token", token.as_str()),
        ("Origin", "https://attacker.example"),
    ];
    let input = format!(
        "{{\"input\": {:?}, \"profile\": \"quick\"}}",
        repo.path().to_string_lossy()
    );
    assert_eq!(
        send(address, "POST", "/api/scans", &evil, &input).status,
        403
    );
    assert_eq!(send(address, "POST", "/api/scans", &auth, "{}").status, 400);
    let started = send(address, "POST", "/api/scans", &auth, &input);
    assert_eq!(started.status, 202, "{}", started.body);
    let job = started.json()["id"].as_str().unwrap().to_owned();
    let begun = Instant::now();
    let finished = loop {
        let state = send(address, "GET", &format!("/api/scans/{job}"), &auth, "").json();
        if state["status"] != "running" || begun.elapsed() > Duration::from_secs(60) {
            break state;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(finished["status"], "completed", "{finished}");
    assert!(finished["scanId"].is_string());
    assert_eq!(
        send(address, "DELETE", "/api/scans/job-999", &auth, "").status,
        404
    );
    assert_eq!(send(address, "PUT", "/", &[], "").status, 405);
}

#[test]
fn serves_a_web_interface_directory() {
    let (_home, _repo, paths) = analyzed_home();
    let web = tempfile::tempdir().expect("web");
    std::fs::create_dir_all(web.path().join("assets")).expect("assets");
    std::fs::write(
        web.path().join("index.html"),
        "<!doctype html><title>RepoDNA</title>",
    )
    .expect("index");
    std::fs::write(web.path().join("assets/app.js"), "export {}").expect("script");
    let running = Running::start(paths, Some(web.path().to_path_buf()));
    let address = running.address();
    assert!(running.server.has_web_interface());
    let index = send(address, "GET", "/", &[], "");
    assert_eq!(index.status, 200);
    assert!(
        index
            .header("Content-Security-Policy")
            .unwrap()
            .contains("script-src 'self'")
    );
    let script = send(address, "GET", "/assets/app.js", &[], "");
    assert_eq!(
        script.header("Content-Type"),
        Some("text/javascript; charset=utf-8")
    );
    let route = send(address, "GET", "/repositories/abc", &[], "");
    assert!(route.body.contains("<title>RepoDNA</title>"));
    assert_eq!(
        send(address, "GET", "/assets/missing.js", &[], "").status,
        404
    );
}
