//! The diagnostics bundle written by `repodna doctor --export`: the results of the checks,
//! the environment, the effective configuration, and summaries of local storage and
//! plugins, as JSON files in a ZIP file that can be attached to a bug report.
//!
//! The bundle contains no source code, file contents, analysis results, or names of stored
//! repositories, and the user's home directory is replaced with `~` wherever it appears.

use std::io::{Cursor, IsTerminal, Write};
use std::path::Path;

use repodna_app::{AppError, DIAGNOSTICS_FOLDER, load_config, write_file};
use repodna_core::model::metadata::SCHEMA_VERSION;
use repodna_core::time::Timestamp;
use repodna_git::GitRunner;
use repodna_parser::builtin_languages;
use repodna_plugin::discover;
use serde_json::{Value, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::Ctx;
use super::manage::build_commit;
use crate::cli::AnalysisArgs;

/// Environment variables that change what RepoDNA does.
const VARIABLES: [&str; 10] = [
    "REPODNA_HOME",
    "REPODNA_GIT",
    "NO_COLOR",
    "TERM",
    "COLUMNS",
    "SOURCE_DATE_EPOCH",
    "XDG_DATA_HOME",
    "XDG_CONFIG_HOME",
    "LOCALAPPDATA",
    "APPDATA",
];

/// Replaces the user's home directory with `~` in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redactor {
    home: Vec<String>,
}

impl Redactor {
    /// Uses the current user's home directory (`HOME`, or `USERPROFILE` on Windows).
    pub fn from_environment() -> Self {
        let home = ["HOME", "USERPROFILE"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty());
        Self::new(home.as_deref())
    }

    /// Replaces `home`, written with either kind of slash. A root directory such as `/` or
    /// `C:\` is never replaced.
    pub fn new(home: Option<&str>) -> Self {
        let Some(home) = home.map(|home| home.trim_end_matches(['/', '\\'])) else {
            return Self { home: Vec::new() };
        };
        let below_root = home
            .rfind(['/', '\\'])
            .is_some_and(|last| last + 1 < home.len());
        if !below_root {
            return Self { home: Vec::new() };
        }
        let mut forms = vec![home.to_owned(), home.replace('\\', "/")];
        forms.dedup();
        Self { home: forms }
    }

    /// `text` with every occurrence of the home directory replaced.
    pub fn text(&self, text: &str) -> String {
        self.home
            .iter()
            .fold(text.to_owned(), |text, home| replace_path(&text, home, "~"))
    }

    /// Replaces the home directory in every string of `value`.
    pub fn value(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.text(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.value(item)),
            Value::Object(fields) => fields.values_mut().for_each(|field| self.value(field)),
            _ => {}
        }
    }
}

/// Replaces `path` in `text` where it is a whole path: not part of a longer directory name
/// such as `/home/alice` for `/home/al`, and not in the middle of another path.
fn replace_path(text: &str, path: &str, with: &str) -> String {
    let name_char = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | '.');
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (start, _) in text.match_indices(path) {
        let end = start + path.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let whole = before.is_none_or(|c| !name_char(c) && c != '/' && c != '\\')
            && after.is_none_or(|c| !name_char(c));
        if start >= copied && whole {
            out.push_str(&text[copied..start]);
            out.push_str(with);
            copied = end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// The explanation at the top of the bundle.
fn readme(created: Timestamp) -> String {
    format!(
        "RepoDNA diagnostics
===================

Created by `repodna doctor --export` with RepoDNA {version} on {date} (UTC).

The files describe your RepoDNA installation, to help find the cause of a problem:

  doctor.json         the results of `repodna doctor`
  environment.json    RepoDNA's version, the operating system and processor, the Git
                      version, whether output goes to a terminal, and the environment
                      variables that change what RepoDNA does
  configuration.json  the effective configuration in the directory where the command ran
                      (defaults, your user configuration, a file passed with --config, and
                      that directory's repodna.toml), and the files it came from
  storage.json        where RepoDNA stores data, and how many analyses and cache entries it
                      holds, with their sizes
  plugins.json        the plugins found, whether they are enabled, and their problems

The bundle contains no source code, file contents, analysis results, or names of stored
repositories. Your home directory is shown as ~. The configuration can contain paths and
patterns you wrote yourself; `--no-project-config` leaves out the repository's
repodna.toml.

Please read the files before you share them. To report a problem, open an issue at
https://github.com/sanskarIN/RepoDNA/issues/new/choose and attach this file.
",
        version = env!("CARGO_PKG_VERSION"),
        date = created.to_rfc3339().replace('T', " ").trim_end_matches('Z'),
    )
}

fn environment(ctx: &Ctx, created: Timestamp) -> Value {
    let git = match GitRunner::detect() {
        Ok(git) => json!({ "version": git.version() }),
        Err(error) => json!({ "error": error.to_string() }),
    };
    let variables: serde_json::Map<String, Value> = VARIABLES
        .iter()
        .filter_map(|name| {
            let value = std::env::var_os(name)?;
            Some((
                (*name).to_owned(),
                Value::String(value.to_string_lossy().into_owned()),
            ))
        })
        .collect();
    json!({
        "createdAt": created,
        "repodna": {
            "version": env!("CARGO_PKG_VERSION"),
            "commit": build_commit(),
            "artifactSchema": SCHEMA_VERSION,
            "pluginApi": repodna_plugin::PLUGIN_API,
            "storageSchema": repodna_store::schema::SCHEMA_VERSION,
            "builtInLanguages": builtin_languages().len(),
        },
        "platform": {
            "os": std::env::consts::OS,
            "family": std::env::consts::FAMILY,
            "arch": std::env::consts::ARCH,
            "processors": std::thread::available_parallelism().map_or(1, usize::from),
        },
        "git": git,
        "terminal": {
            "stdout": std::io::stdout().is_terminal(),
            "stderr": std::io::stderr().is_terminal(),
            "color": ctx.out.color,
        },
        "variables": variables,
    })
}

fn configuration(ctx: &Ctx) -> Value {
    let options = ctx.config_options(&AnalysisArgs::default());
    match load_config(&ctx.paths, Some(Path::new(".")), &options) {
        Ok(loaded) => json!({
            "userConfiguration": ctx.paths.config_file,
            "sources": loaded.sources,
            "warnings": loaded.warnings,
            "config": loaded.config,
        }),
        Err(error) => json!({
            "userConfiguration": ctx.paths.config_file,
            "error": error.message,
        }),
    }
}

fn storage(ctx: &Ctx) -> Value {
    let store = match ctx.paths.open_store() {
        Ok(store) => store,
        Err(error) => {
            return json!({ "directory": ctx.paths.data_home, "error": error.message });
        }
    };
    let problems = store
        .check()
        .unwrap_or_else(|error| vec![error.to_string()]);
    match store.stats() {
        Ok(stats) => json!({
            "directory": stats.home,
            "repositories": stats.repositories,
            "analyses": stats.scans,
            "artifacts": stats.artifacts,
            "artifactBytes": stats.artifact_bytes,
            "databaseBytes": stats.database_bytes,
            "cacheEntries": stats.cache.entries,
            "cacheBytes": stats.cache.bytes,
            "problems": problems,
        }),
        Err(error) => json!({
            "directory": ctx.paths.data_home,
            "error": error.to_string(),
            "problems": problems,
        }),
    }
}

fn plugins(ctx: &Ctx) -> Value {
    let options = ctx.config_options(&AnalysisArgs::default());
    let config = match load_config(&ctx.paths, None, &options) {
        Ok(loaded) => loaded.config,
        Err(error) => return json!({ "error": error.message }),
    };
    let directories = ctx.paths.plugin_dirs(&config.plugins.directories, &[]);
    let discovery = discover(&directories);
    let found: Vec<Value> = discovery
        .plugins
        .iter()
        .map(|plugin| {
            let manifest = &plugin.manifest;
            json!({
                "name": manifest.name,
                "version": manifest.version,
                "api": manifest.api,
                "enabled": config.plugins.enabled.contains(&manifest.name),
                "analyzer": !manifest.command.is_empty(),
                "languageDefinitions": manifest.languages.len(),
                "permissions": manifest.permissions,
                "directory": plugin.directory,
                "problems": plugin.problems,
            })
        })
        .collect();
    json!({
        "directories": directories,
        "enabled": config.plugins.enabled,
        "found": found,
        "warnings": discovery.warnings,
    })
}

/// The files of the bundle, in order, with the home directory redacted.
pub fn files(
    ctx: &Ctx,
    checks: Value,
    redactor: &Redactor,
    created: Timestamp,
) -> Result<Vec<(String, Vec<u8>)>, AppError> {
    let mut files = vec![(
        format!("{DIAGNOSTICS_FOLDER}/README.txt"),
        readme(created).into_bytes(),
    )];
    let documents = [
        ("doctor.json", checks),
        ("environment.json", environment(ctx, created)),
        ("configuration.json", configuration(ctx)),
        ("storage.json", storage(ctx)),
        ("plugins.json", plugins(ctx)),
    ];
    for (name, mut document) in documents {
        redactor.value(&mut document);
        let mut text = serde_json::to_string_pretty(&document)
            .map_err(|error| AppError::internal(error.to_string()))?;
        text.push('\n');
        files.push((format!("{DIAGNOSTICS_FOLDER}/{name}"), text.into_bytes()));
    }
    Ok(files)
}

/// Packs `files` into a ZIP archive.
pub fn zip(files: &[(String, Vec<u8>)], created: Timestamp) -> Result<Vec<u8>, AppError> {
    let civil = created.civil();
    let modified = u16::try_from(civil.year)
        .ok()
        .and_then(|year| {
            let part = |value: u32| u8::try_from(value).unwrap_or(0);
            DateTime::from_date_and_time(
                year,
                part(civil.month),
                part(civil.day),
                part(civil.hour),
                part(civil.minute),
                part(civil.second),
            )
            .ok()
        })
        .unwrap_or_default();
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(modified)
        .unix_permissions(0o644);
    let failed = |error: &dyn std::fmt::Display| {
        AppError::internal(format!("cannot create the diagnostics file: {error}"))
    };
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name.as_str(), options)
            .map_err(|error| failed(&error))?;
        archive.write_all(bytes).map_err(|error| failed(&error))?;
    }
    let cursor = archive.finish().map_err(|error| failed(&error))?;
    Ok(cursor.into_inner())
}

/// Writes the bundle to `path`.
pub fn export(ctx: &Ctx, checks: Value, path: &Path, force: bool) -> Result<(), AppError> {
    let created = Timestamp::now();
    let files = files(ctx, checks, &Redactor::from_environment(), created)?;
    write_file(path, &zip(&files, created)?, force)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_home_directory_only_as_a_whole_path() {
        let redactor = Redactor::new(Some("/home/al/"));
        assert_eq!(
            redactor.text("/home/al/.local/share/repodna is readable"),
            "~/.local/share/repodna is readable"
        );
        assert_eq!(redactor.text("/home/al"), "~");
        assert_eq!(redactor.text("\"/home/al\", /home/al:x"), "\"~\", ~:x");
        assert_eq!(redactor.text("/home/alice/x"), "/home/alice/x");
        assert_eq!(redactor.text("/data/home/al/x"), "/data/home/al/x");
        assert_eq!(redactor.text("/home/al.old/x"), "/home/al.old/x");
    }

    #[test]
    fn replaces_windows_homes_with_either_slash() {
        let redactor = Redactor::new(Some(r"C:\Users\Al"));
        assert_eq!(
            redactor.text(r"C:\Users\Al\AppData\Local\RepoDNA"),
            r"~\AppData\Local\RepoDNA"
        );
        assert_eq!(redactor.text("C:/Users/Al/plugins"), "~/plugins");
    }

    #[test]
    fn never_replaces_a_root_directory() {
        for root in ["/", "C:\\", "C:", ""] {
            let redactor = Redactor::new(Some(root));
            assert_eq!(redactor.text("/usr/bin C:\\x"), "/usr/bin C:\\x", "{root}");
        }
        assert_eq!(Redactor::new(None).text("/home/al"), "/home/al");
    }

    #[test]
    fn redacts_every_string_in_a_document() {
        let redactor = Redactor::new(Some("/home/al"));
        let mut document = json!({
            "directory": "/home/al/data",
            "list": ["/home/al/a", 3, null],
            "nested": { "path": "/home/al" },
        });
        redactor.value(&mut document);
        assert_eq!(
            document,
            json!({ "directory": "~/data", "list": ["~/a", 3, null], "nested": { "path": "~" } })
        );
    }

    #[test]
    fn packs_files_into_a_readable_zip() {
        let created = Timestamp::from_ymd(2026, 9, 26).unwrap();
        let files = vec![
            (
                "repodna-diagnostics/README.txt".to_owned(),
                b"hello".to_vec(),
            ),
            ("repodna-diagnostics/a.json".to_owned(), b"{}\n".to_vec()),
        ];
        let bytes = zip(&files, created).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 2);
        let mut readme = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("repodna-diagnostics/README.txt").unwrap(),
            &mut readme,
        )
        .unwrap();
        assert_eq!(readme, "hello");
        let modified = archive.by_index(1).unwrap().last_modified().unwrap();
        assert_eq!(
            (modified.year(), modified.month(), modified.day()),
            (2026, 9, 26)
        );
    }

    #[test]
    fn the_readme_names_every_file() {
        let text = readme(Timestamp::from_ymd(2026, 9, 26).unwrap());
        for name in [
            "doctor.json",
            "environment.json",
            "configuration.json",
            "storage.json",
            "plugins.json",
        ] {
            assert!(text.contains(name), "{name}");
        }
        assert!(text.contains("2026-09-26 00:00:00 (UTC)"));
        assert!(text.contains("no source code"));
    }
}
