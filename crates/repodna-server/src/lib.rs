//! The RepoDNA local server behind `repodna serve`.
//!
//! It serves the web interface and a JSON API for stored analyses, and it can start
//! analyses in the background. It listens on 127.0.0.1 only. Every API request must carry
//! the session token, either in a header or in a `SameSite=Strict` cookie set when the
//! user opens the link printed at startup; requests with any other `Host` header are
//! refused (which defeats DNS rebinding), and requests that change state are refused when
//! their `Origin` is another site. Responses carry a restrictive Content Security Policy.
//! Nothing is uploaded anywhere: the server only reads local storage and the paths the
//! user asks it to analyze.

pub mod api;
pub mod assets;
pub mod http;
pub mod jobs;
pub mod library;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use repodna_app::{AnalyzeOptions, AppError, AppPaths};
use repodna_core::CancellationToken;
use repodna_core::hash::to_hex;

pub use api::{COOKIE, State};
pub use assets::Assets;

/// How long a worker waits for a request before checking for shutdown.
const POLL: Duration = Duration::from_millis(200);

/// Server settings.
#[derive(Debug, Clone, Default)]
pub struct ServerOptions {
    /// Port on 127.0.0.1; 0 picks a free one.
    pub port: u16,
    /// Serve the web interface from this directory instead of the embedded files.
    pub web_dir: Option<PathBuf>,
    /// Allow starting analyses through the API.
    pub allow_scans: bool,
    /// Configuration for analyses started through the API (`input` is ignored).
    pub template: AnalyzeOptions,
    /// Worker threads (at least one).
    pub workers: usize,
}

/// A bound server.
pub struct Server {
    http: Arc<tiny_http::Server>,
    state: Arc<State>,
    address: SocketAddr,
    workers: usize,
}

fn new_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| AppError::internal(format!("no secure random source: {error}")))?;
    Ok(to_hex(&bytes))
}

impl Server {
    /// Binds to 127.0.0.1 and opens local storage.
    pub fn bind(paths: AppPaths, options: ServerOptions) -> Result<Self, AppError> {
        let store = paths.open_store()?;
        let http = tiny_http::Server::http(("127.0.0.1", options.port)).map_err(|error| {
            AppError::input(format!(
                "cannot listen on 127.0.0.1:{}: {error}",
                options.port
            ))
            .with_hint("Choose another port with --port.")
        })?;
        let address = http
            .server_addr()
            .to_ip()
            .ok_or_else(|| AppError::internal("the server is not listening on an IP address"))?;
        if let Some(dir) = &options.web_dir
            && !dir.join("index.html").is_file()
        {
            return Err(AppError::input(format!(
                "{} has no index.html; build the web interface first",
                dir.display()
            )));
        }
        let state = State {
            paths,
            store,
            token: new_token()?,
            port: address.port(),
            assets: Assets::choose(options.web_dir),
            jobs: jobs::Jobs::default(),
            allow_scans: options.allow_scans,
            template: options.template,
        };
        Ok(Self {
            http: Arc::new(http),
            state: Arc::new(state),
            address,
            workers: options.workers.max(1),
        })
    }

    /// The listening address.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The session token.
    pub fn token(&self) -> &str {
        &self.state.token
    }

    /// The link that signs the browser in.
    pub fn login_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/?token={}",
            self.address.port(),
            self.state.token
        )
    }

    /// `true` when this server has the interactive web interface.
    pub fn has_web_interface(&self) -> bool {
        self.state.assets.available()
    }

    /// Serves requests until `cancel` is set, then stops running analyses.
    pub fn run(&self, cancel: &CancellationToken) {
        let handles: Vec<_> = (0..self.workers)
            .map(|_| {
                let http = Arc::clone(&self.http);
                let state = Arc::clone(&self.state);
                let cancel = cancel.clone();
                std::thread::spawn(move || {
                    while !cancel.is_cancelled() {
                        match http.recv_timeout(POLL) {
                            Ok(Some(mut request)) => {
                                let reply = api::handle(&state, &mut request);
                                // A client that disconnected early is not an error.
                                let _ = request.respond(reply);
                            }
                            Ok(None) => {}
                            Err(_) => std::thread::sleep(POLL),
                        }
                    }
                })
            })
            .collect();
        for handle in handles {
            let _ = handle.join();
        }
        self.state.jobs.cancel_all();
    }
}
