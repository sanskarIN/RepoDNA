//! Findings, the onboarding guide, the evidence and metrics appendices, and metadata.

use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::metadata::{AnalyzerStatus, DataSourceKind};
use repodna_core::severity::Severity;

use super::{confidence, heading};
use crate::doc::{Block, Blocks, Inline, Table, code, plain, truncate};
use crate::sections::Section;
use crate::text::{list, number, thousands};

/// Findings shown in full before the rest are summarized.
const MAX_FULL_FINDINGS: usize = 60;

pub(super) fn findings(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Findings);
    let counts = dna.finding_counts();
    blocks.stats(vec![
        ("Critical".to_owned(), counts.critical.to_string()),
        ("Warnings".to_owned(), counts.warning.to_string()),
        ("Attention".to_owned(), counts.attention.to_string()),
        ("Informational".to_owned(), counts.info.to_string()),
        ("Suppressed".to_owned(), counts.suppressed.to_string()),
    ]);
    let active: Vec<_> = dna.findings.iter().filter(|f| !f.is_suppressed()).collect();
    if active.is_empty() {
        blocks.text("No active findings.");
    }
    let (major, info): (Vec<_>, Vec<_>) = active
        .into_iter()
        .partition(|f| f.severity >= Severity::Attention);
    for finding in major.iter().take(MAX_FULL_FINDINGS) {
        blocks.0.push(Block::Finding(Box::new((*finding).clone())));
    }
    if major.len() > MAX_FULL_FINDINGS {
        blocks.text(format!(
            "{} more findings are listed in the evidence appendix and the JSON artifact.",
            major.len() - MAX_FULL_FINDINGS
        ));
    }
    if !info.is_empty() {
        blocks.heading(3, "Informational findings", None);
        let mut table = Table::new(&["Finding", "Rule", "Confidence", "Summary"]);
        let mut rows = info;
        table.omitted = truncate(&mut rows, 80);
        for finding in rows {
            table.row(vec![
                plain(finding.title.clone()),
                code(finding.rule.clone()),
                plain(confidence(finding.confidence)),
                plain(finding.summary.clone()),
            ]);
        }
        blocks.table(table);
    }
    let suppressed: Vec<_> = dna.findings.iter().filter(|f| f.is_suppressed()).collect();
    if !suppressed.is_empty() {
        blocks.heading(3, "Suppressed findings", None);
        let mut table = Table::new(&["Finding", "Rule", "Reason", "Suppressed by"]);
        for finding in suppressed {
            let suppression = finding.suppressed.as_ref();
            table.row(vec![
                plain(finding.title.clone()),
                code(finding.rule.clone()),
                plain(suppression.map(|s| s.reason.clone()).unwrap_or_default()),
                plain(suppression.map(|s| s.source.clone()).unwrap_or_default()),
            ]);
        }
        blocks.table(table);
    }
    blocks.heading(3, "How severity is assigned", None);
    blocks.list(
        Severity::ALL
            .iter()
            .map(|severity| {
                vec![
                    Inline::Strong(severity.label().to_owned()),
                    Inline::Text(format!(": {}", severity.criteria())),
                ]
            })
            .collect(),
    );
}

pub(super) fn onboarding(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Onboarding);
    let insights = &dna.insights;
    if insights.onboarding.is_empty() && insights.important_files.is_empty() {
        blocks.caution(
            "Onboarding guidance was not generated because the insights stage did not run.",
        );
        return;
    }
    for (index, step) in insights.onboarding.iter().enumerate() {
        blocks.heading(3, format!("{}. {}", index + 1, step.title), None);
        if !step.description.is_empty() {
            blocks.text(step.description.clone());
        }
        let mut items = Vec::new();
        for command in &step.commands {
            items.push(vec![
                Inline::Text("Run ".to_owned()),
                Inline::Code(command.clone()),
            ]);
        }
        for path in &step.paths {
            items.push(vec![
                Inline::Text("Open ".to_owned()),
                Inline::Code(path.clone()),
            ]);
        }
        blocks.list(items);
    }
    if !insights.important_files.is_empty() {
        blocks.heading(3, "Important files", None);
        let mut table = Table::new(&["File", "Score#", "Why"]);
        for file in &insights.important_files {
            table.row(vec![
                code(file.path.clone()),
                plain(number(file.score)),
                plain(file.reasons.join("; ")),
            ]);
        }
        blocks.table(table);
    }
    if !insights.glossary.is_empty() {
        blocks.heading(3, "Glossary", None);
        let mut table = Table::new(&["Term", "Meaning"]);
        for term in &insights.glossary {
            table.row(vec![
                plain(term.term.clone()),
                plain(term.definition.clone()),
            ]);
        }
        blocks.table(table);
    }
}

pub(super) fn evidence(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Evidence);
    if dna.findings.is_empty() {
        blocks.text("There are no findings, so there is no evidence to list.");
        return;
    }
    blocks.text("Every finding with the facts it rests on. Paths are relative to the repository root; line numbers refer to the analyzed revision.");
    let mut table = Table::new(&["Finding", "Severity", "Evidence"]);
    let mut findings: Vec<_> = dna.findings.iter().collect();
    table.omitted = truncate(&mut findings, 300);
    for finding in findings {
        let evidence: Vec<String> = finding.evidence.iter().map(|e| e.describe()).collect();
        let mut severity = finding.severity.label().to_owned();
        if finding.is_suppressed() {
            severity.push_str(" (suppressed)");
        }
        table.row(vec![
            vec![
                Inline::Text(finding.title.clone()),
                Inline::Text(" ".to_owned()),
                Inline::Code(finding.id.clone()),
            ],
            plain(severity),
            plain(if evidence.is_empty() {
                "–".to_owned()
            } else {
                list(&evidence)
            }),
        ]);
    }
    blocks.table(table);
}

pub(super) fn metrics(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Metrics);
    let fingerprint = &dna.fingerprint;
    if !fingerprint.dimensions.is_empty() {
        blocks.heading(3, "DNA fingerprint", None);
        if !fingerprint.method.is_empty() {
            blocks.note(fingerprint.method.clone());
        }
        let mut table = Table::new(&[
            "Dimension",
            "Value#",
            "Measured#",
            "Confidence",
            "How it is measured",
        ]);
        for dimension in &fingerprint.dimensions {
            table.row(vec![
                plain(dimension.label.clone()),
                plain(number(dimension.value)),
                plain(format!("{} {}", number(dimension.raw), dimension.unit)),
                plain(confidence(dimension.confidence)),
                plain(dimension.description.clone()),
            ]);
        }
        blocks.table(table);
    }
    let report = &dna.metrics;
    if !report.confidence.is_empty() {
        blocks.heading(3, "Confidence by section", None);
        let mut table = Table::new(&["Section", "Confidence", "Why"]);
        for section in &report.confidence {
            table.row(vec![
                plain(section.section.clone()),
                plain(confidence(section.confidence)),
                plain(section.reason.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !report.signals.is_empty() {
        blocks.heading(3, "Normalized signals", None);
        let mut table = Table::new(&["Signal", "Value#", "Interpretation", "Confidence"]);
        for signal in &report.signals {
            table.row(vec![
                plain(signal.label.clone()),
                plain(number(signal.value)),
                plain(signal.interpretation.clone()),
                plain(confidence(signal.confidence)),
            ]);
        }
        blocks.table(table);
    }
    if !report.raw.is_empty() {
        blocks.heading(3, "Raw metrics", None);
        let mut table = Table::new(&["Metric", "Value#", "Confidence", "Definition", "Method"]);
        for metric in &report.raw {
            table.row(vec![
                vec![
                    Inline::Text(metric.label.clone()),
                    Inline::Text(" ".to_owned()),
                    Inline::Code(metric.id.clone()),
                ],
                plain(format!("{} {}", number(metric.value), metric.unit)),
                plain(confidence(metric.confidence)),
                plain(metric.definition.clone()),
                plain(metric.method.clone()),
            ]);
        }
        blocks.table(table);
    }
}

fn status_label(status: AnalyzerStatus) -> &'static str {
    match status {
        AnalyzerStatus::Completed => "Completed",
        AnalyzerStatus::Partial => "Partial",
        AnalyzerStatus::Skipped => "Skipped",
        AnalyzerStatus::Failed => "Failed",
        AnalyzerStatus::Cancelled => "Cancelled",
    }
}

fn source_label(kind: DataSourceKind) -> &'static str {
    match kind {
        DataSourceKind::LocalFiles => "Local files",
        DataSourceKind::GitHistory => "Git history",
        DataSourceKind::RemoteClone => "Remote clone",
        DataSourceKind::Plugin => "Plugin",
        DataSourceKind::Ai => "AI provider",
        DataSourceKind::Advisory => "Advisory service",
    }
}

/// The machine-readable report signature.
pub fn signature(dna: &RepositoryDna) -> String {
    let value = serde_json::json!({
        "generatedBy": "RepoDNA",
        "toolVersion": dna.tool.version,
        "schemaVersion": dna.schema_version,
        "analysisId": dna.analysis_metadata.id,
        "revision": dna.analysis_metadata.revision,
        "dnaHash": dna.fingerprint.dna_hash,
        "generatedAt": dna.analysis_metadata.generated_at.to_rfc3339(),
    });
    format!("{value:#}")
}

pub(super) fn metadata(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Metadata);
    let metadata = &dna.analysis_metadata;
    let mut table = Table::new(&["Property", "Value"]);
    let mut row = |label: &str, value: Vec<Inline>| table.row(vec![plain(label), value]);
    row("RepoDNA version", plain(dna.tool.version.clone()));
    row("Artifact schema", plain(dna.schema_version.clone()));
    row("Analysis ID", code(metadata.id.clone()));
    row("Generated", plain(metadata.generated_at.to_rfc3339()));
    row("Profile", plain(metadata.profile.id()));
    row(
        "Input",
        plain(format!(
            "{} ({:?})",
            metadata.input.display, metadata.input.kind
        )),
    );
    if let Some(revision) = &metadata.revision {
        row("Revision", code(revision.clone()));
    }
    if let Some(dirty) = metadata.dirty {
        row(
            "Uncommitted changes",
            plain(if dirty {
                "Yes: the working tree differs from the revision"
            } else {
                "No"
            }),
        );
    }
    row(
        "Platform",
        plain(format!(
            "{} {} ({})",
            metadata.platform.os, metadata.platform.arch, metadata.platform.family
        )),
    );
    row("Configuration hash", code(metadata.config_hash.clone()));
    row(
        "Configuration sources",
        plain(metadata.config_sources.join(", ")),
    );
    if metadata.duration_ms > 0 {
        row(
            "Duration",
            plain(format!("{:.1} s", metadata.duration_ms as f64 / 1000.0)),
        );
    }
    row(
        "AI",
        plain(metadata.ai.as_ref().map_or_else(
            || "Not used".to_owned(),
            |ai| {
                format!(
                    "{} {} ({} requests{})",
                    ai.provider,
                    ai.model,
                    ai.requests,
                    if ai.remote { ", remote" } else { ", local" }
                )
            },
        )),
    );
    blocks.table(table);
    blocks.heading(3, "Analysis stages", None);
    let mut table = Table::new(&["Stage", "Status", "Duration#", "Details"]);
    for run in &metadata.analyzers {
        table.row(vec![
            plain(run.label.clone()),
            plain(status_label(run.status)),
            plain(format!("{} ms", thousands(run.duration_ms))),
            plain(run.message.clone().unwrap_or_default()),
        ]);
    }
    blocks.table(table);
    if !metadata.data_sources.is_empty() {
        blocks.heading(3, "Data sources", None);
        let mut table = Table::new(&["Source", "Name", "Detail", "Accessed"]);
        for source in &metadata.data_sources {
            table.row(vec![
                plain(source_label(source.kind)),
                plain(source.name.clone()),
                plain(source.detail.clone()),
                plain(source.accessed_at.to_rfc3339()),
            ]);
        }
        blocks.table(table);
    }
    blocks.heading(3, "Privacy", None);
    let privacy = &metadata.privacy;
    let mut items = vec![
        plain(format!("Privacy preset: {}.", privacy.preset.id())),
        plain("Telemetry: none. RepoDNA does not collect telemetry."),
        plain(if privacy.network_used {
            "The network was used during the analysis (to clone the repository)."
        } else {
            "The network was not used during the analysis."
        }),
        plain(if privacy.remote_ai {
            "A remote AI provider received data."
        } else {
            "No data was sent to a remote AI provider."
        }),
    ];
    items.extend(privacy.redactions.iter().map(|r| plain(r.clone())));
    blocks.list(items);
    if let Ok(serde_json::Value::Object(thresholds)) = serde_json::to_value(&metadata.thresholds) {
        blocks.heading(3, "Thresholds", None);
        let mut table = Table::new(&["Threshold", "Value#"]);
        for (name, value) in thresholds {
            table.row(vec![code(name), plain(value.to_string())]);
        }
        blocks.table(table);
    }
    if !metadata.suppressions.is_empty() {
        blocks.heading(3, "Suppression rules", None);
        let mut table = Table::new(&["Rule", "Path", "Reason", "Defined in"]);
        for rule in &metadata.suppressions {
            table.row(vec![
                code(rule.rule.clone()),
                plain(rule.path.clone().unwrap_or_else(|| "–".to_owned())),
                plain(rule.reason.clone()),
                plain(rule.source.clone().unwrap_or_default()),
            ]);
        }
        blocks.table(table);
    }
    if !metadata.warnings.is_empty() {
        blocks.heading(3, "Warnings", None);
        blocks.list(metadata.warnings.iter().map(|w| plain(w.clone())).collect());
    }
    blocks.heading(3, "Report signature", None);
    blocks.0.push(Block::Preformatted(signature(dna)));
}

#[cfg(test)]
mod tests {
    use super::super::{ContentOptions, section};
    use crate::markdown::render;
    use crate::sections::Section;
    use repodna_core::confidence::Confidence;
    use repodna_core::finding::{Finding, FindingCategory};
    use repodna_core::model::artifact::RepositoryDna;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::severity::Severity;

    #[test]
    fn lists_findings_by_severity_with_criteria() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.findings = vec![
            Finding::new(
                "a.rule",
                "x",
                FindingCategory::Architecture,
                Severity::Warning,
                Confidence::High,
                "A warning",
            ),
            Finding::new(
                "b.rule",
                "y",
                FindingCategory::Documentation,
                Severity::Info,
                Confidence::Medium,
                "An observation",
            ),
        ];
        let markdown = render(&section(&dna, Section::Findings, ContentOptions::default()));
        assert!(markdown.contains("#### Warning · A warning"));
        assert!(markdown.contains("| An observation | `b.rule` | Medium |"));
        assert!(markdown.contains("**Critical**: High-confidence evidence"));
    }

    #[test]
    fn signs_the_report_with_metadata() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.analysis_metadata.revision = Some("abc".into());
        let markdown = render(&section(&dna, Section::Metadata, ContentOptions::default()));
        assert!(markdown.contains("\"generatedBy\": \"RepoDNA\""));
        assert!(markdown.contains("\"revision\": \"abc\""));
        assert!(markdown.contains("Telemetry: none."));
        assert!(markdown.contains("`large_file_lines`"));
    }
}
