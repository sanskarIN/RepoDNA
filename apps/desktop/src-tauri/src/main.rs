//! The RepoDNA desktop app: the web interface in a native window, answered by the Rust
//! core through Tauri commands instead of HTTP.
//!
//! The commands mirror the local server's API (`repodna serve`) and return the same JSON,
//! so the interface behaves the same in both. Save and folder dialogs and opening links run
//! here, in Rust; the page itself has no access to the file system or the shell. Nothing
//! leaves the machine unless the user analyzes a remote Git URL or opens a web link.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;
use std::path::PathBuf;

use repodna_app::{AnalyzeOptions, AppPaths, report_bundle, write_bundle};
use repodna_core::config::{AnalysisProfile, PrivacyPreset, ReportTheme};
use repodna_report::ReportOptions;
use repodna_report::card::{CardOptions, render as render_card_svg};
use repodna_server::jobs::{JobState, Jobs};
use repodna_server::library;
use repodna_store::Store;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, RunEvent, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;

/// Errors cross into the page as plain messages.
type Reply<T> = Result<T, String>;

/// Local storage and the analyses running in the background.
struct Library {
    paths: AppPaths,
    store: Store,
    jobs: Jobs,
}

/// The library, or why it could not be opened (shown by the page instead of crashing).
struct Desktop(Result<Library, String>);

impl Desktop {
    fn open() -> Self {
        let opened = AppPaths::resolve().and_then(|paths| {
            let store = paths.open_store()?;
            Ok(Library {
                paths,
                store,
                jobs: Jobs::default(),
            })
        });
        Self(opened.map_err(|error| format!("Local storage could not be opened: {error}")))
    }

    fn library(&self) -> Reply<&Library> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
fn session(desktop: State<'_, Desktop>) -> Reply<Value> {
    desktop.library()?;
    Ok(json!({ "version": env!("CARGO_PKG_VERSION"), "allowScans": true }))
}

#[tauri::command]
fn list_repositories(desktop: State<'_, Desktop>) -> Reply<Value> {
    library::repositories(&desktop.library()?.store).map_err(message)
}

#[tauri::command]
fn get_repository(desktop: State<'_, Desktop>, id: String) -> Reply<Value> {
    library::repository_detail(&desktop.library()?.store, &id).map_err(message)
}

#[tauri::command]
fn load_artifact(desktop: State<'_, Desktop>, id: String, scan: Option<String>) -> Reply<Value> {
    let store = &desktop.library()?.store;
    let dna = library::load(store, &id, scan.as_deref(), PrivacyPreset::Local).map_err(message)?;
    serde_json::to_value(dna).map_err(message)
}

#[tauri::command]
fn start_scan(
    desktop: State<'_, Desktop>,
    input: String,
    profile: Option<String>,
) -> Reply<JobState> {
    let library = desktop.library()?;
    let input = input.trim();
    if input.is_empty() {
        return Err("Name a directory, archive, or Git URL to analyze.".to_owned());
    }
    let mut options = AnalyzeOptions {
        input: input.to_owned(),
        ..AnalyzeOptions::default()
    };
    if let Some(profile) = profile {
        options.config.overrides.profile = Some(profile.parse::<AnalysisProfile>()?);
    }
    library.jobs.start(library.paths.clone(), options)
}

#[tauri::command]
fn get_job(desktop: State<'_, Desktop>, id: String) -> Reply<JobState> {
    desktop
        .library()?
        .jobs
        .get(&id)
        .ok_or_else(|| "no such analysis job".to_owned())
}

#[tauri::command]
fn cancel_job(desktop: State<'_, Desktop>, id: String) -> Reply<()> {
    if desktop.library()?.jobs.cancel(&id) {
        Ok(())
    } else {
        Err("no such analysis job".to_owned())
    }
}

fn into_path(path: FilePath) -> Reply<PathBuf> {
    path.into_path().map_err(message)
}

#[tauri::command]
async fn pick_directory(app: AppHandle) -> Reply<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a repository to analyze")
        .blocking_pick_folder();
    Ok(match picked {
        Some(path) => Some(into_path(path)?.display().to_string()),
        None => None,
    })
}

/// Asks where to save `text` and writes it there; `None` when the user cancels.
fn save_text(
    app: &AppHandle,
    file_name: &str,
    filter: (&str, &[&str]),
    text: &str,
) -> Reply<Option<String>> {
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(file_name)
        .add_filter(filter.0, filter.1)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = into_path(path)?;
    std::fs::write(&path, text)
        .map_err(|error| format!("{} could not be written: {error}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
async fn save_report(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    id: String,
    format: String,
    scan: Option<String>,
    theme: Option<String>,
    privacy: Option<String>,
) -> Reply<Option<String>> {
    let library = desktop.library()?;
    let preset = library::parse_privacy(privacy.as_deref().unwrap_or("local"))?;
    let theme = library::parse_theme(theme.as_deref().unwrap_or("professional"))?;
    let dna = library::load(&library.store, &id, scan.as_deref(), preset).map_err(message)?;
    match format.as_str() {
        "bundle" => {
            let Some(parent) = app
                .dialog()
                .file()
                .set_title("Choose where to create the report folder")
                .blocking_pick_folder()
            else {
                return Ok(None);
            };
            let dir = into_path(parent)?.join("repodna-report");
            let options = ReportOptions {
                theme,
                privacy: preset,
                ..ReportOptions::default()
            };
            let files = report_bundle(&dna, &options).map_err(message)?;
            write_bundle(&dir, &files, false).map_err(message)?;
            Ok(Some(dir.display().to_string()))
        }
        "card" => {
            let svg = render_card_svg(
                &dna,
                CardOptions {
                    dark: theme == ReportTheme::Dark,
                    branding: true,
                },
            );
            save_text(&app, "dna-card.svg", ("SVG image", &["svg"]), &svg)
        }
        format => {
            let document = library::report(&dna, format, theme, preset)?;
            let (label, file_name) = match document.extension {
                "html" => ("HTML report", "repodna-report.html"),
                "md" => ("Markdown report", "repodna-report.md"),
                _ => ("JSON artifact", "repodna.json"),
            };
            save_text(
                &app,
                file_name,
                (label, &[document.extension]),
                &document.text,
            )
        }
    }
}

#[tauri::command]
fn render_card(
    desktop: State<'_, Desktop>,
    id: String,
    dark: bool,
    scan: Option<String>,
) -> Reply<String> {
    let store = &desktop.library()?.store;
    let dna = library::load(store, &id, scan.as_deref(), PrivacyPreset::Local).map_err(message)?;
    Ok(render_card_svg(
        &dna,
        CardOptions {
            dark,
            branding: true,
        },
    ))
}

/// Opens a web page in the system browser. Only https links are accepted, so the page
/// cannot use this to start programs or open local files.
#[tauri::command]
fn open_link(app: AppHandle, url: String) -> Reply<()> {
    if !is_web_link(&url) {
        return Err("Only https links can be opened.".to_owned());
    }
    app.opener().open_url(url, None::<&str>).map_err(message)
}

fn is_web_link(url: &str) -> bool {
    url.strip_prefix("https://").is_some_and(|rest| {
        !rest.is_empty() && !rest.starts_with('/') && !rest.contains(char::is_whitespace)
    })
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Desktop::open())
        .invoke_handler(tauri::generate_handler![
            session,
            list_repositories,
            get_repository,
            load_artifact,
            start_scan,
            get_job,
            cancel_job,
            pick_directory,
            save_report,
            render_card,
            open_link,
        ])
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|handle, event| {
            if let RunEvent::Exit = event
                && let Some(desktop) = handle.try_state::<Desktop>()
                && let Ok(library) = desktop.library()
            {
                library.jobs.cancel_all();
            }
        }),
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "RepoDNA could not start: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_links_open() {
        assert!(is_web_link("https://github.com/sanskarIN/RepoDNA"));
        assert!(!is_web_link("http://example.com"));
        assert!(!is_web_link("file:///etc/passwd"));
        assert!(!is_web_link("https://"));
        assert!(!is_web_link("https:///local"));
        assert!(!is_web_link("https://example.com/a b"));
        assert!(!is_web_link("javascript:alert(1)"));
    }
}
