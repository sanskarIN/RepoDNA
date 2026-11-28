//! Application services shared by the RepoDNA command-line interface, the local server,
//! and the desktop app.
//!
//! The analysis engine turns a repository into an artifact. This crate adds everything
//! around it that front ends need in the same way: locating user configuration, plugins,
//! and storage; assembling the effective configuration; running analyses with the cache,
//! plugins, and local history; loading earlier analyses from files or storage; writing
//! reports without overwriting files RepoDNA did not create; and optional AI
//! explanations. Front ends only parse input and present results.

pub mod analysis;
pub mod config;
pub mod error;
pub mod explain;
pub mod export;
pub mod paths;
pub mod source;

pub use analysis::{
    AnalysisOutcome, AnalyzeOptions, Loaded, Source, Target, import_artifact, load_stored,
    load_target, resolve_target, run_analysis,
};
pub use config::{ConfigOptions, Overrides, load_config};
pub use error::{AppError, ErrorKind};
pub use explain::{ExplainOutcome, ExplainRequest, explain, plan_explanation};
pub use export::{
    DIAGNOSTICS_FOLDER, OUTPUT_MARKER, OutputFile, artifact_file_name, report_bundle, write_bundle,
    write_file,
};
pub use paths::AppPaths;
pub use source::{SourceReference, SourceReferences, source_references, source_references_matching};

/// An explicit source revision selected by the user.
pub use repodna_git::url::RequestedRevision;

/// Parse one explicit source revision for the CLI, server, and desktop app.
/// Exactly one selector may be supplied; invalid refs never reach Git.
pub fn requested_revision(
    branch: Option<&str>,
    tag: Option<&str>,
    commit: Option<&str>,
) -> Result<Option<RequestedRevision>, AppError> {
    let mut selections = [
        branch.map(|value| RequestedRevision::Branch(value.to_owned())),
        tag.map(|value| RequestedRevision::Tag(value.to_owned())),
        commit.map(|value| RequestedRevision::Commit(value.to_owned())),
    ]
    .into_iter()
    .flatten();
    let selection = selections.next();
    if selections.next().is_some() {
        return Err(AppError::input("select only one of branch, tag, or commit"));
    }
    if let Some(revision) = &selection {
        revision
            .validate()
            .map_err(|error| AppError::input(error.to_string()))?;
    }
    Ok(selection)
}

#[cfg(test)]
mod source_revision_tests {
    use super::*;

    #[test]
    fn rejects_conflicting_and_invalid_revision_selections() {
        assert!(requested_revision(Some("main"), Some("v1"), None).is_err());
        assert!(requested_revision(Some("--exec"), None, None).is_err());
        assert!(requested_revision(None, None, Some("short-sha")).is_err());
        assert_eq!(
            requested_revision(Some("feat/initial-audio-platform"), None, None).unwrap(),
            Some(RequestedRevision::Branch(
                "feat/initial-audio-platform".into()
            )),
        );
        assert_eq!(requested_revision(None, None, None).unwrap(), None);
    }
}
