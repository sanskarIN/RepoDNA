//! Analyzer plugins as analysis extensions.

use std::path::Path;
use std::time::{Duration, Instant};

use repodna_core::CancellationToken;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::metadata::{AnalyzerStatus, PluginRunRecord};
use repodna_engine::Extension;

use crate::discover::Plugin;
use crate::process::{Limits, Outcome, run};
use crate::protocol::{build_request, parse_response};

/// Runs one analyzer plugin after the built-in stages.
#[derive(Debug, Clone)]
pub struct PluginExtension {
    plugin: Plugin,
    limits: Limits,
    reproducible: bool,
}

impl PluginExtension {
    /// Wraps `plugin`. With `reproducible`, recorded durations are zero.
    pub fn new(plugin: Plugin, limits: Limits, reproducible: bool) -> Self {
        Self {
            plugin,
            limits,
            reproducible,
        }
    }
}

fn duration_ms(started: Instant, reproducible: bool) -> u64 {
    if reproducible {
        0
    } else {
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

impl Extension for PluginExtension {
    fn name(&self) -> String {
        format!("plugin {}", self.plugin.manifest.name)
    }

    fn run(
        &self,
        root: &Path,
        dna: &mut RepositoryDna,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        let manifest = &self.plugin.manifest;
        let started = Instant::now();
        let mut record = PluginRunRecord {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            status: AnalyzerStatus::Failed,
            duration_ms: 0,
            findings: 0,
            metrics: 0,
            message: None,
        };
        let request = build_request(manifest, dna, &root.to_string_lossy());
        let input = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        let result = match run(
            &manifest.command,
            &self.plugin.directory,
            &input,
            self.limits,
            cancel,
        ) {
            Outcome::Success(output) => parse_response(&manifest.name, &output),
            Outcome::Failed(message) => Err(message),
            Outcome::TimedOut => Err(format!(
                "stopped after the {} second time limit",
                self.limits.timeout.as_secs()
            )),
            Outcome::Cancelled => {
                record.status = AnalyzerStatus::Cancelled;
                Err("cancelled".to_owned())
            }
        };
        record.duration_ms = duration_ms(started, self.reproducible);
        match result {
            Ok(contribution) => {
                record.findings = u32::try_from(contribution.findings.len()).unwrap_or(u32::MAX);
                record.metrics = u32::try_from(contribution.metrics.len()).unwrap_or(u32::MAX);
                let mut messages = contribution.notes;
                if !contribution.rejected.is_empty() {
                    messages.push(format!(
                        "{} items rejected: {}",
                        contribution.rejected.len(),
                        contribution
                            .rejected
                            .iter()
                            .take(5)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                }
                record.status = if contribution.rejected.is_empty() {
                    AnalyzerStatus::Completed
                } else {
                    AnalyzerStatus::Partial
                };
                record.message = (!messages.is_empty()).then(|| messages.join(" "));
                dna.findings.extend(contribution.findings);
                dna.metrics.raw.extend(contribution.metrics);
                dna.plugins.push(record);
                Ok(())
            }
            Err(message) => {
                record.message = Some(message.clone());
                dna.plugins.push(record);
                Err(message)
            }
        }
    }
}

/// Default limits from the plugin configuration.
pub fn limits(timeout_seconds: u64, max_output_bytes: u64) -> Limits {
    Limits {
        timeout: Duration::from_secs(timeout_seconds.max(1)),
        max_output_bytes: max_output_bytes.max(1024),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::manifest::{PLUGIN_API, PluginManifest};
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    fn plugin(dir: &Path, script: &str) -> Plugin {
        Plugin {
            manifest: PluginManifest {
                name: "demo".into(),
                version: "0.1.0".into(),
                description: String::new(),
                api: PLUGIN_API,
                command: vec!["sh".into(), "-c".into(), script.into()],
                permissions: Vec::new(),
                languages: Vec::new(),
                homepage: None,
                license: None,
            },
            directory: dir.to_path_buf(),
            problems: Vec::new(),
        }
    }

    fn dna() -> RepositoryDna {
        RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        )
    }

    #[test]
    fn adds_findings_metrics_and_a_run_record() {
        let dir = tempfile::tempdir().unwrap();
        let script = r#"cat >/dev/null; printf '%s' '{"api":1,"findings":[{"rule":"hello","title":"Hello"}],"metrics":[{"id":"n","value":2}],"notes":["done"]}'"#;
        let extension = PluginExtension::new(plugin(dir.path(), script), limits(10, 4096), true);
        assert_eq!(extension.name(), "plugin demo");
        let mut dna = dna();
        extension
            .run(dir.path(), &mut dna, &CancellationToken::new())
            .unwrap();
        assert_eq!(dna.findings[0].rule, "plugin.demo.hello");
        assert_eq!(dna.metrics.raw[0].id, "plugin.demo.n");
        let record = &dna.plugins[0];
        assert_eq!(record.status, AnalyzerStatus::Completed);
        assert_eq!(
            (record.findings, record.metrics, record.duration_ms),
            (1, 1, 0)
        );
        assert_eq!(record.message.as_deref(), Some("done"));
    }

    #[test]
    fn records_failures() {
        let dir = tempfile::tempdir().unwrap();
        let extension = PluginExtension::new(
            plugin(dir.path(), "cat >/dev/null; echo not-json"),
            limits(10, 4096),
            true,
        );
        let mut dna = dna();
        let error = extension
            .run(dir.path(), &mut dna, &CancellationToken::new())
            .unwrap_err();
        assert!(error.contains("not a valid response"));
        assert_eq!(dna.plugins[0].status, AnalyzerStatus::Failed);
        assert!(dna.findings.is_empty());
    }
}
