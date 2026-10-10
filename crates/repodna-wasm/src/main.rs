//! RepoDNA's analysis, reports, and Project DNA cards as a WebAssembly program.
//!
//! Built for `wasm32-wasip1`, it runs where there is no Git and no local storage: in the
//! web version, which hands it the files the user chose through a small in-memory WASI
//! file system, and under Node.js through `@sanskarin/repodna-wasm`. It also builds and
//! runs natively, which is how its tests run.
//!
//! ```text
//! repodna-wasm analyze <directory or archive> [--profile <name>] [--anonymize-contributors]
//! repodna-wasm report <artifact> [--format html|markdown|json] [--theme <name>] [--privacy <preset>]
//! repodna-wasm card <artifact> [--dark] [--png]
//! repodna-wasm version
//! ```
//!
//! Results are written to standard output. While analyzing, progress is written to standard
//! error as one JSON object per line, and a failure as `{"event":"error","message":...}`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use repodna_core::CancellationToken;
use repodna_core::config::load::ConfigLoader;
use repodna_core::config::{AnalysisProfile, Parallelism, PrivacyPreset, ReportTheme};
use repodna_core::io::{read_artifact, to_json};
use repodna_core::model::artifact::RepositoryDna;
use repodna_engine::{AnalysisRequest, InputSpec, Progress, ProgressEvent, analyze};
use repodna_report::card::{self, CardOptions};
use repodna_report::png::render_png;
use repodna_report::{ReportOptions, html_report, json_report, markdown_report};
use serde_json::{Value, json};

/// Where temporary files go under WASI.
const FALLBACK_TEMP_DIR: &str = "/tmp";

/// How much larger than its SVG size a PNG card is rendered.
const PNG_SCALE: f32 = 2.0;

/// A failed command, with the message shown to the user.
#[derive(Debug, PartialEq, Eq)]
struct Failure(String);

impl<E: std::fmt::Display> From<E> for Failure {
    fn from(error: E) -> Self {
        Self(error.to_string())
    }
}

/// The command line, split into the command, its operand, and its options.
#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    command: String,
    operand: Option<String>,
    options: Vec<(String, Option<String>)>,
}

/// Options that take a value; every other option is a switch.
const VALUED: [&str; 4] = ["--profile", "--format", "--theme", "--privacy"];

fn parse_arguments(args: impl IntoIterator<Item = String>) -> Result<Arguments, Failure> {
    let mut args = args.into_iter();
    let mut parsed = Arguments {
        command: args.next().unwrap_or_default(),
        ..Arguments::default()
    };
    while let Some(arg) = args.next() {
        if let Some((name, value)) = arg.split_once('=').filter(|_| arg.starts_with("--")) {
            parsed
                .options
                .push((name.to_owned(), Some(value.to_owned())));
        } else if VALUED.contains(&arg.as_str()) {
            let value = args
                .next()
                .ok_or_else(|| Failure(format!("{arg} needs a value")))?;
            parsed.options.push((arg, Some(value)));
        } else if arg.starts_with("--") {
            parsed.options.push((arg, None));
        } else if parsed.operand.is_none() {
            parsed.operand = Some(arg);
        } else {
            return Err(Failure(format!("unexpected argument `{arg}`")));
        }
    }
    Ok(parsed)
}

impl Arguments {
    fn value(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(option, _)| option == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn switch(&self, name: &str) -> bool {
        self.options.iter().any(|(option, _)| option == name)
    }

    fn operand(&self, what: &str) -> Result<&str, Failure> {
        self.operand
            .as_deref()
            .ok_or_else(|| Failure(format!("`{}` needs {what}", self.command)))
    }

    /// Rejects options the command does not know, so that a typo is not silently ignored.
    fn only(&self, known: &[&str]) -> Result<(), Failure> {
        match self
            .options
            .iter()
            .find(|(option, _)| !known.contains(&option.as_str()))
        {
            Some((option, _)) => Err(Failure(format!(
                "`{}` does not take {option}",
                self.command
            ))),
            None => Ok(()),
        }
    }
}

/// Parses a kebab-case name of a configuration value, such as `dark` or `share`.
fn named<T: serde::de::DeserializeOwned>(name: &str, what: &str) -> Result<T, Failure> {
    serde_json::from_value(Value::String(name.to_owned()))
        .map_err(|_| Failure(format!("unknown {what} `{name}`")))
}

/// Writes one progress event to standard error as a line of JSON.
fn report_progress(event: &ProgressEvent) {
    let line = match event {
        ProgressEvent::StageStarted(stage) => {
            json!({ "event": "stage", "stage": stage.id(), "status": "started" })
        }
        ProgressEvent::StageFinished {
            stage,
            status,
            duration,
        } => json!({
            "event": "stage",
            "stage": stage.id(),
            "status": status,
            "milliseconds": duration.as_millis(),
        }),
        ProgressEvent::Files { done, total } => {
            json!({ "event": "files", "done": done, "total": total })
        }
        ProgressEvent::Message(text) => json!({ "event": "message", "text": text }),
    };
    let _ = writeln!(io::stderr().lock(), "{line}");
}

/// Gives temporary files, such as an extracted archive, a place under WASI, where asking
/// the system for its temporary directory stops the program.
fn ensure_temp_dir() {
    if cfg!(target_os = "wasi") {
        let fallback = Path::new(FALLBACK_TEMP_DIR);
        let _ = std::fs::create_dir_all(fallback);
        let _ = tempfile::env::override_temp_dir(fallback);
    }
}

/// Analyzes a directory or an archive with the repository's own configuration, without
/// Git history (there is no Git), the cache, or plugins.
fn analyze_input(args: &Arguments) -> Result<RepositoryDna, Failure> {
    args.only(&["--profile", "--anonymize-contributors"])?;
    let input = args.operand("a directory or an archive to analyze")?;
    ensure_temp_dir();
    let spec = InputSpec::detect(input);
    let mut loader = ConfigLoader::default();
    if let InputSpec::Directory(path) = &spec {
        loader = ConfigLoader::for_project(path);
    }
    let mut loaded = loader.load()?;
    let config = &mut loaded.config;
    if let Some(profile) = args.value("--profile") {
        config.analysis.profile = profile.parse::<AnalysisProfile>().map_err(Failure)?;
    }
    if args.switch("--anonymize-contributors") {
        config.privacy.anonymize_contributors = true;
    }
    config.performance.parallelism = Parallelism::Threads(1);
    config.performance.cache = false;
    config.plugins.enabled.clear();
    let mut request = AnalysisRequest::new(spec, loaded.config);
    request.config_sources = loaded.sources;
    request.config_warnings = loaded.warnings;
    request.progress = Progress::new(report_progress);
    Ok(analyze(&request, &CancellationToken::new())?)
}

fn load(args: &Arguments) -> Result<RepositoryDna, Failure> {
    let path = PathBuf::from(args.operand("an analysis file")?);
    Ok(read_artifact(&path)?.artifact)
}

fn render_report(args: &Arguments) -> Result<Vec<u8>, Failure> {
    args.only(&["--format", "--theme", "--privacy"])?;
    let dna = load(args)?;
    let theme: ReportTheme = named(args.value("--theme").unwrap_or("professional"), "theme")?;
    let privacy: PrivacyPreset =
        named(args.value("--privacy").unwrap_or("local"), "privacy preset")?;
    let options = ReportOptions {
        theme,
        privacy,
        ..ReportOptions::default()
    };
    let text = match args.value("--format").unwrap_or("html") {
        "html" => html_report(&dna, &options),
        "markdown" => markdown_report(&dna, &options),
        "json" => json_report(&dna, privacy, true)?,
        other => return Err(Failure(format!("unknown report format `{other}`"))),
    };
    Ok(text.into_bytes())
}

fn render_card(args: &Arguments) -> Result<Vec<u8>, Failure> {
    args.only(&["--dark", "--png"])?;
    let dna = load(args)?;
    let svg = card::render(
        &dna,
        CardOptions {
            dark: args.switch("--dark"),
            branding: true,
        },
    );
    if args.switch("--png") {
        Ok(render_png(&svg, PNG_SCALE)?)
    } else {
        Ok(svg.into_bytes())
    }
}

/// Runs a command and returns what it writes to standard output.
fn run(args: &Arguments) -> Result<Vec<u8>, Failure> {
    match args.command.as_str() {
        "analyze" => {
            let dna = analyze_input(args)?;
            let mut json = to_json(&dna, false)?;
            json.push('\n');
            Ok(json.into_bytes())
        }
        "report" => render_report(args),
        "card" => render_card(args),
        "version" => {
            args.only(&[])?;
            Ok(format!("repodna-wasm {}\n", env!("CARGO_PKG_VERSION")).into_bytes())
        }
        "" => Err(Failure(
            "a command is needed: analyze, report, card, or version".to_owned(),
        )),
        other => Err(Failure(format!("unknown command `{other}`"))),
    }
}

fn main() -> ExitCode {
    let result = parse_arguments(std::env::args().skip(1)).and_then(|args| run(&args));
    match result {
        Ok(output) => {
            let mut stdout = io::stdout().lock();
            match stdout.write_all(&output).and_then(|()| stdout.flush()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::FAILURE,
            }
        }
        Err(Failure(message)) => {
            let line = json!({ "event": "error", "message": message });
            let _ = writeln!(io::stderr().lock(), "{line}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Arguments {
        parse_arguments(line.split_whitespace().map(str::to_owned)).unwrap()
    }

    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("README.md"), "# Widget\n").unwrap();
        dir
    }

    #[test]
    fn parses_commands_operands_and_options() {
        let parsed = args("analyze repo --profile quick --anonymize-contributors");
        assert_eq!(parsed.command, "analyze");
        assert_eq!(parsed.operand.as_deref(), Some("repo"));
        assert_eq!(parsed.value("--profile"), Some("quick"));
        assert!(parsed.switch("--anonymize-contributors"));
        assert_eq!(args("report a --theme=dark").value("--theme"), Some("dark"));
        assert_eq!(
            parse_arguments(["report".into(), "a".into(), "--theme".into()]),
            Err(Failure("--theme needs a value".into()))
        );
        assert_eq!(
            parse_arguments(["card".into(), "a".into(), "b".into()]),
            Err(Failure("unexpected argument `b`".into()))
        );
    }

    #[test]
    fn rejects_unknown_commands_options_and_names() {
        assert_eq!(
            run(&args("explain x")),
            Err(Failure("unknown command `explain`".into()))
        );
        assert!(run(&args("")).is_err());
        assert_eq!(
            run(&args("card a --theme dark")),
            Err(Failure("`card` does not take --theme".into()))
        );
        assert_eq!(
            named::<ReportTheme>("neon", "theme"),
            Err(Failure("unknown theme `neon`".into()))
        );
        assert_eq!(
            run(&args("analyze")),
            Err(Failure(
                "`analyze` needs a directory or an archive to analyze".into()
            ))
        );
    }

    #[test]
    fn analyzes_a_directory_and_renders_reports_and_cards() {
        let repo = repository();
        let root = repo.path().display().to_string();
        let output = run(&args(&format!("analyze {root} --profile quick"))).unwrap();
        let dna: RepositoryDna = serde_json::from_slice(&output).unwrap();
        assert_eq!(dna.identity.name, repo_name(repo.path()));
        assert_eq!(dna.languages.primary, vec!["rust".to_owned()]);

        let file = repo.path().join("widget.repodna");
        std::fs::write(&file, &output).unwrap();
        let file = file.display().to_string();
        let html = run(&args(&format!("report {file} --theme dark"))).unwrap();
        assert!(String::from_utf8(html).unwrap().contains("<html"));
        let markdown = run(&args(&format!("report {file} --format markdown"))).unwrap();
        assert!(String::from_utf8(markdown).unwrap().starts_with('#'));
        let svg = run(&args(&format!("card {file} --dark"))).unwrap();
        assert!(String::from_utf8(svg).unwrap().contains("<svg"));
        let png = run(&args(&format!("card {file} --png"))).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }

    fn repo_name(path: &Path) -> String {
        path.file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn reports_the_version() {
        let output = run(&args("version")).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("repodna-wasm {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}
