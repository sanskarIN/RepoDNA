//! The `repodna` command-line interface.
//!
//! The binary is a thin wrapper around [`run`]. Commands live in [`commands`]; they parse
//! nothing themselves and call the shared application services in `repodna-app`, so the
//! CLI, the local server, and the desktop app behave the same way.

pub mod cli;
pub mod commands;
pub mod progress;
pub mod render;
pub mod term;

use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use repodna_app::{AppError, AppPaths, ErrorKind};
use repodna_core::CancellationToken;

use crate::cli::{Cli, Command};
use crate::commands::views::View;
use crate::commands::{
    Ctx, analyze, ci, compare, doctor, explain, manage, plugins, report, serve, views,
};
use crate::progress::Reporter;
use crate::term::Style;

fn dispatch(ctx: &Ctx, command: &Command) -> Result<(), AppError> {
    match command {
        Command::Analyze(cmd) => analyze::run(ctx, cmd),
        Command::Report(cmd) => report::run_report(ctx, cmd),
        Command::Architecture(cmd) => views::run_view(ctx, cmd, View::Architecture),
        Command::Dependencies(cmd) => views::run_view(ctx, cmd, View::Dependencies),
        Command::History(cmd) => views::run_view(ctx, cmd, View::History),
        Command::Hotspots(cmd) => views::run_view(ctx, cmd, View::Hotspots),
        Command::Timeline(cmd) => views::run_view(ctx, cmd, View::Timeline),
        Command::Findings(cmd) => views::run_findings(ctx, cmd),
        Command::Show(cmd) => views::run_show(ctx, cmd),
        Command::Compare(cmd) => compare::run(ctx, cmd),
        Command::Card(cmd) => report::run_card(ctx, cmd),
        Command::Badge(cmd) => report::run_badge(ctx, cmd),
        Command::Onboarding(cmd) => report::run_onboarding(ctx, cmd),
        Command::Explain(cmd) => explain::run(ctx, cmd),
        Command::Ci(cmd) => ci::run(ctx, cmd),
        Command::Export(cmd) => report::run_export(ctx, cmd),
        Command::Import(cmd) => report::run_import(ctx, cmd),
        Command::List(cmd) => report::run_list(ctx, cmd),
        Command::Init(cmd) => manage::run_init(cmd),
        Command::Config(cmd) => manage::run_config(ctx, cmd),
        Command::Plugins(cmd) => plugins::run(ctx, cmd),
        Command::Cache(cmd) => manage::run_cache(ctx, cmd),
        Command::Clean(cmd) => manage::run_clean(ctx, cmd),
        Command::Doctor(cmd) => doctor::run(ctx, cmd),
        Command::Version(cmd) => manage::run_version(cmd),
        Command::Serve(cmd) => serve::run(ctx, cmd),
        Command::Completions(cmd) => manage::run_completions(cmd),
    }
}

/// Prints an error and its hint on standard error.
fn report_error(style: Style, error: &AppError) {
    let label = if error.kind == ErrorKind::Cancelled {
        style.yellow("cancelled:")
    } else {
        style.red("error:")
    };
    let message = if error.kind == ErrorKind::Cancelled {
        "the operation was stopped; completed cache entries were kept".to_owned()
    } else {
        error.message.clone()
    };
    let mut text = format!("{label} {message}\n");
    if let Some(hint) = &error.hint {
        text.push_str(&format!("{} {hint}\n", style.dim("hint:")));
    }
    to_stderr(&text);
}

/// Writes to standard error, ignoring failures (there is nowhere left to report them).
fn to_stderr(text: &str) {
    let _ = std::io::stderr().lock().write_all(text.as_bytes());
}

fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Parses the command line, runs the command, and returns the process exit code.
pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return exit_code(if error.use_stderr() {
                ErrorKind::Usage.exit_code()
            } else {
                0
            });
        }
    };
    let err_style = Style::detect(cli.global.color, true);
    let cancel = CancellationToken::new();
    {
        let cancel = cancel.clone();
        let style = err_style;
        // A second Ctrl+C exits immediately.
        let _ = ctrlc::set_handler(move || {
            if cancel.is_cancelled() {
                std::process::exit(ErrorKind::Cancelled.exit_code());
            }
            cancel.cancel();
            to_stderr(&format!(
                "\n{} Stopping… completed cache entries are kept. Press Ctrl+C again to quit now.\n",
                style.yellow("■")
            ));
        });
    }
    let paths = match AppPaths::resolve() {
        Ok(paths) => paths,
        Err(error) => {
            report_error(err_style, &error);
            return exit_code(error.exit_code());
        }
    };
    let reporter =
        (!cli.global.quiet).then(|| Reporter::new(err_style, std::io::stderr().is_terminal()));
    let ctx = Ctx {
        out: Style::detect(cli.global.color, false),
        err: err_style,
        global: cli.global.clone(),
        paths,
        cancel,
        reporter,
    };
    match dispatch(&ctx, &cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(reporter) = &ctx.reporter {
                reporter.finish();
            }
            report_error(err_style, &error);
            exit_code(error.exit_code())
        }
    }
}
