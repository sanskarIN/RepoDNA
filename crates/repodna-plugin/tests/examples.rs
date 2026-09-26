//! The example plugins in `plugins/` load, and the Python analyzer produces valid output
//! when `python3` is available.

use std::path::{Path, PathBuf};
use std::process::Command;

use repodna_core::CancellationToken;
use repodna_core::config::PluginConfig;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::identity::RepositoryIdentity;
use repodna_core::model::metadata::{AnalysisMetadata, AnalyzerStatus};
use repodna_core::model::structure::{FileCategory, FileRecord};
use repodna_plugin::load_enabled;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn record(path: &str) -> FileRecord {
    FileRecord {
        path: path.into(),
        category: FileCategory::Source,
        language: Some("rust".into()),
        bytes: 10,
        lines: None,
        binary: false,
        generated: false,
        vendored: false,
        hash: None,
        module: None,
        analysis: None,
        skipped: None,
    }
}

#[test]
fn example_plugins_load() {
    let config = PluginConfig {
        enabled: vec!["zig-language".into(), "license-headers".into()],
        ..PluginConfig::default()
    };
    let loaded = load_enabled(&config, &[examples()], true);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.active, vec!["zig-language", "license-headers"]);
    assert_eq!(loaded.languages.len(), 1);
    assert_eq!(loaded.languages[0].id, "zig");
    assert_eq!(loaded.extensions.len(), 1);
}

#[test]
fn license_headers_plugin_reports_missing_identifiers() {
    let available = Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !available {
        eprintln!("python3 is not available; skipping the analyzer run");
        return;
    }
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src/a.rs"),
        "// SPDX-License-Identifier: MIT\nfn a() {}\n",
    )
    .unwrap();
    std::fs::write(repo.path().join("src/b.rs"), "fn b() {}\n").unwrap();
    let mut dna = RepositoryDna::new(
        RepositoryIdentity {
            name: "sample".into(),
            ..RepositoryIdentity::default()
        },
        AnalysisMetadata::default(),
    );
    dna.structure.files = vec![record("src/a.rs"), record("src/b.rs")];

    let config = PluginConfig {
        enabled: vec!["license-headers".into()],
        ..PluginConfig::default()
    };
    let loaded = load_enabled(&config, &[examples()], true);
    loaded.extensions[0]
        .run(repo.path(), &mut dna, &CancellationToken::new())
        .unwrap();
    assert_eq!(dna.plugins[0].status, AnalyzerStatus::Completed);
    let metric = dna
        .metrics
        .raw
        .iter()
        .find(|m| m.id == "plugin.license-headers.spdx-coverage")
        .unwrap();
    assert_eq!(metric.value, 0.5);
    let finding = &dna.findings[0];
    assert_eq!(
        finding.rule,
        "plugin.license-headers.missing-spdx-identifier"
    );
    assert_eq!(finding.paths, vec!["src/b.rs"]);
    assert_eq!(
        finding.title,
        "1 of 2 source files has no SPDX license identifier"
    );
}
