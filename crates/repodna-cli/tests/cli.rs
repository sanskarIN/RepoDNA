//! End-to-end tests of the `repodna` binary. Each test uses its own storage directory
//! (`REPODNA_HOME`), so nothing touches the user's real configuration or data.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repodna_testkit::{GitRepo, git_available, write_tree};
use tempfile::TempDir;

struct Env {
    home: TempDir,
    work: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            home: TempDir::new().unwrap(),
            work: TempDir::new().unwrap(),
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_repodna"))
            .args(args)
            .current_dir(self.work.path())
            .env("REPODNA_HOME", self.home.path())
            .env("NO_COLOR", "1")
            .env("SOURCE_DATE_EPOCH", "1790000000")
            .env_remove("COLUMNS")
            .output()
            .unwrap()
    }

    fn work(&self, name: &str) -> PathBuf {
        self.work.path().join(name)
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

/// A small repository without Git metadata.
fn sample_repo(root: &Path, with_readme: bool) {
    write_tree(
        root,
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nserde = \"1\"\n",
            ),
            (
                "src/lib.rs",
                "//! Widgets.\n\nmod parse;\n\npub use parse::parse;\n\n/// Adds.\npub fn add(a: u32, b: u32) -> u32 {\n    if a > b { a + b } else { b + a }\n}\n",
            ),
            (
                "src/parse.rs",
                "/// Parses.\npub fn parse(text: &str) -> usize {\n    // TODO: handle errors\n    text.len()\n}\n",
            ),
            (
                "tests/add.rs",
                "#[test]\nfn adds() {\n    assert_eq!(widget::add(1, 2), 3);\n}\n",
            ),
        ],
    );
    if with_readme {
        write_tree(
            root,
            &[(
                "README.md",
                "# Widget\n\nA small widget library.\n\n## Usage\n\n```sh\ncargo test\n```\n",
            )],
        );
    }
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn prints_version_help_and_usage_errors() {
    let env = Env::new();
    let version = env.run(&["version"]);
    assert_eq!(code(&version), 0);
    assert!(stdout(&version).starts_with(&format!("RepoDNA {}", env!("CARGO_PKG_VERSION"))));
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&env.run(&["version", "--json"]))).unwrap();
    assert_eq!(json["schemaVersion"], "1.0");

    let help = env.run(&["--help"]);
    assert_eq!(code(&help), 0);
    assert!(stdout(&help).contains("Exit codes:"));
    assert_eq!(code(&env.run(&["no-such-command"])), 2);
    assert_eq!(code(&env.run(&["compare", "only-one"])), 2);
    let completions = env.run(&["completions", "bash"]);
    assert_eq!(code(&completions), 0);
    assert!(stdout(&completions).contains("repodna"));
}

#[test]
fn analyzes_views_reports_and_exports() {
    let env = Env::new();
    // Not directly in the working directory, so that `widget` names the stored repository.
    let repo = env.work("repos/widget");
    sample_repo(&repo, true);
    let repo_arg = path(&repo);

    let analyzed = env.run(&["analyze", &repo_arg]);
    assert_eq!(code(&analyzed), 0, "{}", stderr(&analyzed));
    let text = stdout(&analyzed);
    assert!(text.contains("widget"), "{text}");
    assert!(text.contains("Files"), "{text}");
    assert!(stderr(&analyzed).contains("Stored as analysis"));

    let json = env.run(&["analyze", &repo_arg, "--format", "json", "-q"]);
    let artifact: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    assert_eq!(artifact["schemaVersion"], "1.0");
    assert_eq!(artifact["identity"]["name"], "widget");

    let architecture = env.run(&["architecture", &repo_arg, "--format", "json", "-q"]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&architecture)).unwrap();
    assert!(value["architecture"]["modules"].is_array());
    let view = env.run(&["dependencies", &repo_arg, "-q"]);
    assert_eq!(code(&view), 0);
    assert!(stdout(&view).contains("serde"), "{}", stdout(&view));
    let findings = env.run(&["findings", &repo_arg, "--format", "json", "-q"]);
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout(&findings))
            .unwrap()
            .is_array()
    );
    let show = env.run(&["show", &repo_arg, "--section", "summary,tests", "-q"]);
    assert!(stdout(&show).contains("Tests"), "{}", stdout(&show));

    // Stored analyses are listed and can be read by name.
    let list = env.run(&["list"]);
    assert!(stdout(&list).contains("widget"));
    let stored = env.run(&["hotspots", "widget"]);
    assert_eq!(code(&stored), 0, "{}", stderr(&stored));
    assert!(stderr(&stored).contains("Using the stored analysis"));

    // Report bundles never overwrite user files.
    let out = env.work("report");
    let report = env.run(&["report", "widget", "--output", &path(&out), "-q"]);
    assert_eq!(code(&report), 0, "{}", stderr(&report));
    assert!(out.join("index.html").is_file());
    assert!(out.join(".repodna-output").is_file());
    assert!(out.join("data/findings.csv").is_file());
    assert_eq!(
        code(&env.run(&["report", "widget", "--output", &path(&out), "-q"])),
        0
    );
    let mine = env.work("mine");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("notes.txt"), "keep").unwrap();
    let refused = env.run(&["report", "widget", "--output", &path(&mine), "-q"]);
    assert_eq!(code(&refused), 3);
    assert!(stderr(&refused).contains("hint:"));
    assert_eq!(
        std::fs::read_to_string(mine.join("notes.txt")).unwrap(),
        "keep"
    );

    let md = env.run(&["report", "widget", "--format", "markdown", "-q"]);
    assert!(stdout(&md).contains("repodna-signature"));
    let card = env.run(&[
        "card",
        "widget",
        "--output",
        &path(&env.work("card.svg")),
        "-q",
    ]);
    assert_eq!(code(&card), 0, "{}", stderr(&card));
    assert!(
        std::fs::read_to_string(env.work("card.svg"))
            .unwrap()
            .contains("<svg")
    );

    // Export, then import into a second, empty storage directory.
    let exported = env.work("widget.repodna");
    assert_eq!(
        code(&env.run(&["export", "widget", "--output", &path(&exported), "-q"])),
        0
    );
    let other = Env::new();
    let imported = other.run(&["import", &path(&exported)]);
    assert_eq!(code(&imported), 0, "{}", stderr(&imported));
    assert!(stdout(&imported).contains("snapshot"));
    let from_file = other.run(&["show", &path(&exported), "--section", "summary", "-q"]);
    assert_eq!(code(&from_file), 0);

    let compare = env.run(&[
        "compare",
        "widget",
        &path(&exported),
        "--format",
        "json",
        "-q",
    ]);
    assert_eq!(code(&compare), 0, "{}", stderr(&compare));
    assert!(serde_json::from_str::<serde_json::Value>(&stdout(&compare)).is_ok());
}

#[test]
fn ci_fails_only_when_asked() {
    let env = Env::new();
    let repo = env.work("widget");
    sample_repo(&repo, false);
    let repo_arg = path(&repo);
    let report_only = env.run(&["ci", &repo_arg, "-q"]);
    assert_eq!(code(&report_only), 0, "{}", stderr(&report_only));
    assert!(stdout(&report_only).contains("not formal guarantees"));
    let failing = env.run(&["ci", &repo_arg, "--fail-on", "attention", "-q"]);
    assert_eq!(code(&failing), 5, "{}", stdout(&failing));
    let summary = env.work("summary.md");
    let github = env.run(&[
        "ci",
        &repo_arg,
        "--format",
        "github",
        "--summary-file",
        &path(&summary),
        "-q",
    ]);
    assert_eq!(code(&github), 0);
    assert!(
        std::fs::read_to_string(&summary)
            .unwrap()
            .contains("RepoDNA")
    );
}

#[test]
fn errors_explain_what_to_do() {
    let env = Env::new();
    let missing = env.run(&["analyze", &path(&env.work("missing"))]);
    assert_eq!(code(&missing), 3);
    assert!(stderr(&missing).contains("error:"));
    assert!(stderr(&missing).contains("hint:"));

    let repo = env.work("widget");
    sample_repo(&repo, true);
    let explain = env.run(&["explain", &path(&repo), "-q"]);
    assert_eq!(code(&explain), 4);
    assert!(stderr(&explain).contains("AI features are off"));

    assert_eq!(code(&env.run(&["cache", "reset"])), 2);
    assert_eq!(code(&env.run(&["init", &path(&repo)])), 0);
    assert_eq!(code(&env.run(&["init", &path(&repo)])), 3);
    assert_eq!(code(&env.run(&["config", "validate", &path(&repo)])), 0);

    std::fs::write(
        repo.join("repodna.toml"),
        "[analysis]\nprofile = \"fastest\"\n",
    )
    .unwrap();
    let invalid = env.run(&["analyze", &path(&repo), "-q"]);
    assert_eq!(code(&invalid), 4, "{}", stderr(&invalid));
}

#[test]
fn manages_configuration_cache_and_plugins() {
    let env = Env::new();
    let init = env.run(&["config", "init"]);
    assert_eq!(code(&init), 0, "{}", stderr(&init));
    let config_path = env.home.path().join("config.toml");
    assert!(config_path.is_file());
    let enable = env.run(&["plugins", "enable", "zig-language"]);
    assert_eq!(code(&enable), 0, "{}", stderr(&enable));
    let text = std::fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("zig-language"));
    assert!(text.contains("# RepoDNA user configuration."));
    assert_eq!(code(&env.run(&["plugins", "disable", "zig-language"])), 0);

    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let check = env.run(&["plugins", "check", &path(&examples.join("zig-language"))]);
    assert_eq!(code(&check), 0, "{}", stderr(&check));
    assert!(stdout(&check).contains("The plugin is valid"));

    let repo = env.work("zigrepo");
    write_tree(
        &repo,
        &[(
            "src/main.zig",
            "const std = @import(\"std\");\npub fn main() void {}\n",
        )],
    );
    let analyzed = env.run(&[
        "analyze",
        &path(&repo),
        "--plugin-dir",
        &path(&examples),
        "--plugin",
        "zig-language",
        "--format",
        "json",
        "-q",
    ]);
    assert_eq!(code(&analyzed), 0, "{}", stderr(&analyzed));
    let artifact: serde_json::Value = serde_json::from_str(&stdout(&analyzed)).unwrap();
    assert_eq!(artifact["languages"]["languages"][0]["id"], "zig");

    let stats = env.run(&["cache", "stats", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&stats)).unwrap();
    assert_eq!(value["repositories"], 1);
    assert_eq!(code(&env.run(&["cache", "clear"])), 0);
    let doctor = env.run(&["doctor"]);
    assert_eq!(code(&doctor), 0, "{}\n{}", stdout(&doctor), stderr(&doctor));
    assert!(stdout(&doctor).contains("storage"));

    let dry = env.run(&["clean", "--all"]);
    assert!(stdout(&dry).contains("Run again with --yes"));
    assert_eq!(code(&env.run(&["clean", "--all", "--yes"])), 0);
    assert!(stdout(&env.run(&["list"])).contains("No stored analyses"));
}

#[cfg(unix)]
#[test]
fn explains_with_a_local_command_provider() {
    let env = Env::new();
    std::fs::write(
        env.home.path().join("config.toml"),
        r#"[ai]
provider = "command"
command = ["sh", "-c", "cat >/dev/null; printf '%s' '{\"summary\":\"A widget library.\",\"points\":[{\"text\":\"It is named widget.\",\"evidence\":[\"E1\"]}],\"confidence\":\"medium\",\"limitations\":[]}'"]
"#,
    )
    .unwrap();
    let repo = env.work("widget");
    sample_repo(&repo, true);
    let dry = env.run(&["explain", &path(&repo), "--dry-run", "-q"]);
    assert_eq!(code(&dry), 0, "{}", stderr(&dry));
    assert!(stdout(&dry).contains("Dry run: nothing was sent."));
    assert!(stdout(&dry).contains("<evidence>"));
    let explained = env.run(&["explain", &path(&repo), "-q"]);
    assert_eq!(code(&explained), 0, "{}", stderr(&explained));
    let text = stdout(&explained);
    assert!(text.contains("(AI-generated)"), "{text}");
    assert!(text.contains("A widget library."), "{text}");
    let json = env.run(&["explain", &path(&repo), "--format", "json", "-q"]);
    let value: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    assert_eq!(value["format"], "repodna-explanation/1");
}

#[test]
fn analyzes_git_history_when_available() {
    if !git_available() {
        return;
    }
    let env = Env::new();
    let repo = GitRepo::new();
    repo.write("src/main.py", "print('one')\n");
    repo.commit(
        "Start",
        "Ada",
        "ada@example.invalid",
        "2024-01-01T10:00:00Z",
    );
    repo.write("src/main.py", "print('two')\n");
    repo.commit(
        "Change",
        "Grace",
        "grace@example.invalid",
        "2024-02-01T10:00:00Z",
    );
    let history = env.run(&["history", &path(repo.path()), "--format", "json", "-q"]);
    assert_eq!(code(&history), 0, "{}", stderr(&history));
    let value: serde_json::Value = serde_json::from_str(&stdout(&history)).unwrap();
    assert_eq!(value["git"]["commitCount"], 2);
    let anonymized = env.run(&[
        "history",
        &path(repo.path()),
        "--format",
        "json",
        "--privacy",
        "public",
        "-q",
    ]);
    assert!(!stdout(&anonymized).contains("Grace"));
}
