//! Privacy presets applied before a report is rendered.
//!
//! Artifacts never contain secret values or absolute local paths. The presets remove more
//! for reports that leave the machine:
//!
//! * `local` keeps everything in the artifact.
//! * `share` removes commit messages, marker comment text, and command output.
//! * `public` additionally replaces contributor names and identifiers with numbered
//!   pseudonyms, removes remote URLs (the repository name and owner are kept), and removes
//!   symbol and function names.

use std::collections::HashMap;

use repodna_core::config::PrivacyPreset;
use repodna_core::model::artifact::RepositoryDna;

fn redact(dna: &mut RepositoryDna, description: &str) {
    let redactions = &mut dna.analysis_metadata.privacy.redactions;
    if !redactions.iter().any(|existing| existing == description) {
        redactions.push(description.to_owned());
    }
}

fn share(dna: &mut RepositoryDna) {
    for commit in &mut dna.git.commits {
        commit.subject.clear();
    }
    redact(dna, "Commit messages are omitted.");
    for item in &mut dna.code_quality.markers.items {
        item.text.clear();
    }
    redact(dna, "Marker comment text is omitted.");
    for execution in dna
        .builds
        .executions
        .iter_mut()
        .chain(dna.tests.executions.iter_mut())
    {
        execution.output_tail.clear();
    }
    redact(dna, "Command output is omitted.");
}

fn public(dna: &mut RepositoryDna) {
    let mut pseudonyms: HashMap<String, String> = HashMap::new();
    for (index, contributor) in dna.git.contributors.iter_mut().enumerate() {
        let pseudonym = format!("contributor-{}", index + 1);
        pseudonyms.insert(contributor.id.clone(), pseudonym.clone());
        contributor.id = pseudonym;
        contributor.name = format!("Contributor {}", index + 1);
    }
    for commit in &mut dna.git.commits {
        commit.author = pseudonyms
            .get(&commit.author)
            .cloned()
            .unwrap_or_else(|| "contributor".to_owned());
    }
    redact(
        dna,
        "Contributor names and identifiers are replaced with pseudonyms.",
    );
    dna.identity.remotes.clear();
    redact(dna, "Remote URLs are removed.");
    dna.structure.symbols.clear();
    let quality = &mut dna.code_quality;
    for function in quality
        .complexity
        .top_functions
        .iter_mut()
        .chain(quality.large_functions.iter_mut())
        .chain(quality.deep_nesting.iter_mut())
    {
        function.name.clear();
    }
    redact(dna, "Symbol and function names are removed.");
}

/// Returns a copy of the artifact with the preset applied and recorded in its privacy
/// metadata.
pub fn apply_privacy(dna: &RepositoryDna, preset: PrivacyPreset) -> RepositoryDna {
    let mut copy = dna.clone();
    match preset {
        PrivacyPreset::Local => {}
        PrivacyPreset::Share => share(&mut copy),
        PrivacyPreset::Public => {
            share(&mut copy);
            public(&mut copy);
        }
    }
    copy.analysis_metadata.privacy.preset = preset;
    copy
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::git::{CommitRecord, ContributorRecord};
    use repodna_core::model::identity::{RemoteInfo, RepositoryIdentity};
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::quality::{FunctionSignal, MarkerItem, MarkerKind};
    use repodna_core::time::Timestamp;

    fn artifact() -> RepositoryDna {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                owner: Some("acme".into()),
                remotes: vec![RemoteInfo {
                    name: "origin".into(),
                    url: "https://git.example.test/acme/widget.git".into(),
                    host: Some("git.example.test".into()),
                    provider: "other".into(),
                }],
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.git.contributors = vec![ContributorRecord {
            id: "c-abc".into(),
            name: "Ana Real".into(),
            commits: 3,
            insertions: 0,
            deletions: 0,
            first_commit: Timestamp::UNIX_EPOCH,
            last_commit: Timestamp::UNIX_EPOCH,
            active_days: 1,
            areas: Vec::new(),
        }];
        dna.git.commits = vec![CommitRecord {
            hash: "1".repeat(40),
            short: "1111111".into(),
            author: "c-abc".into(),
            timestamp: Timestamp::UNIX_EPOCH,
            subject: "Fix the internal billing export".into(),
            files_changed: 1,
            insertions: 1,
            deletions: 0,
            merge: false,
        }];
        dna.code_quality.markers.items = vec![MarkerItem {
            path: "src/a.rs".into(),
            line: 3,
            kind: MarkerKind::Todo,
            text: "TODO: ask Ana about the customer list".into(),
        }];
        dna.code_quality.large_functions = vec![FunctionSignal {
            path: "src/a.rs".into(),
            name: "process_payroll".into(),
            line: 10,
            end_line: 200,
            lines: 190,
            cyclomatic: 20,
            nesting: 3,
            language: "rust".into(),
        }];
        dna
    }

    #[test]
    fn local_keeps_everything() {
        let dna = artifact();
        let local = apply_privacy(&dna, PrivacyPreset::Local);
        assert_eq!(local.git.commits[0].subject, dna.git.commits[0].subject);
        assert!(local.analysis_metadata.privacy.redactions.is_empty());
    }

    #[test]
    fn share_removes_free_text() {
        let shared = apply_privacy(&artifact(), PrivacyPreset::Share);
        assert!(shared.git.commits[0].subject.is_empty());
        assert!(shared.code_quality.markers.items[0].text.is_empty());
        assert_eq!(shared.git.contributors[0].name, "Ana Real");
        assert_eq!(
            shared.analysis_metadata.privacy.preset,
            PrivacyPreset::Share
        );
        assert_eq!(shared.analysis_metadata.privacy.redactions.len(), 3);
    }

    #[test]
    fn public_anonymizes_people_and_names() {
        let public = apply_privacy(&artifact(), PrivacyPreset::Public);
        assert_eq!(public.git.contributors[0].name, "Contributor 1");
        assert_eq!(public.git.contributors[0].id, "contributor-1");
        assert_eq!(public.git.commits[0].author, "contributor-1");
        assert!(public.identity.remotes.is_empty());
        assert_eq!(public.identity.owner.as_deref(), Some("acme"));
        assert!(public.code_quality.large_functions[0].name.is_empty());
        let json = serde_json::to_string(&public).unwrap();
        for private in [
            "Ana Real",
            "c-abc",
            "billing",
            "payroll",
            "git.example.test",
            "customer",
        ] {
            assert!(!json.contains(private), "{private} leaked");
        }
    }
}
