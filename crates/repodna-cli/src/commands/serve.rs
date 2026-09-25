//! `repodna serve`: the web interface and local API on 127.0.0.1.

use repodna_app::AppError;
use repodna_server::{Server, ServerOptions};

use super::{Ctx, print};
use crate::cli::ServeCmd;

/// Worker threads answering requests.
const WORKERS: usize = 4;

/// Runs `repodna serve` until Ctrl+C.
pub fn run(ctx: &Ctx, cmd: &ServeCmd) -> Result<(), AppError> {
    let server = Server::bind(
        ctx.paths.clone(),
        ServerOptions {
            port: cmd.port,
            web_dir: cmd.web_dir.clone(),
            allow_scans: !cmd.no_scan,
            template: ctx.analyze_options("", &cmd.analysis),
            workers: WORKERS,
        },
    )?;
    let mut out = format!(
        "RepoDNA is listening on http://{} (this machine only).\n\nOpen this link to sign in. It contains your session token, so do not share it:\n  {}\n",
        server.address(),
        server.login_url()
    );
    if !server.has_web_interface() {
        out.push_str(
            "\nThis build does not include the interactive web interface; the page lists stored reports.\n",
        );
    }
    if cmd.no_scan {
        out.push_str("Starting analyses from the browser is turned off.\n");
    }
    out.push_str("\nPress Ctrl+C to stop.");
    print(&out)?;
    server.run(&ctx.cancel);
    print("Stopped.")
}
