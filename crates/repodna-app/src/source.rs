//! User-requested, bounded remote reference discovery for native clients.

use repodna_core::CancellationToken;
use repodna_git::GitError;
use repodna_git::remote_refs::{RemoteRefKind, RemoteRefQuery, discover_remote_refs_matching};
use repodna_git::url::{UrlPolicy, looks_like_url};
use serde::Serialize;

use crate::AppError;

/// A selectable remote branch or tag, without the submitted remote URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReference {
    /// Branch or tag namespace.
    pub kind: &'static str,
    /// Full reference name.
    pub name: String,
    /// Advertised object ID; may be an annotated tag object.
    pub object: String,
}

/// Discovery results for one explicit request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReferences {
    /// Bounded, deterministically ordered choices.
    pub references: Vec<SourceReference>,
    /// The server omitted additional references.
    pub truncated: bool,
}

/// Discover remote references with the caller's existing clone policy.
/// This does not upload analysis data or modify repositories.
pub fn source_references(
    input: &str,
    policy: UrlPolicy,
    cancel: &CancellationToken,
) -> Result<SourceReferences, AppError> {
    source_references_matching(input, policy, None, None, cancel)
}

/// Search remote references by namespace and substring before the result cap.
pub fn source_references_matching(
    input: &str,
    policy: UrlPolicy,
    kind: Option<&str>,
    search: Option<&str>,
    cancel: &CancellationToken,
) -> Result<SourceReferences, AppError> {
    let kind = match kind {
        None | Some("all") => None,
        Some("branch") => Some(RemoteRefKind::Branch),
        Some("tag") => Some(RemoteRefKind::Tag),
        Some(_) => return Err(AppError::input("reference kind must be all, branch or tag")),
    };
    let query = RemoteRefQuery::new(kind, search)
        .map_err(|error| AppError::input(error.to_string()))?;
    if !looks_like_url(input) {
        return Err(AppError::input(
            "a remote Git URL is required to list references",
        ));
    }
    let listing = discover_remote_refs_matching(input, policy, &query, cancel).map_err(|error| {
        let message = error.to_string();
        match error {
            GitError::InvalidUrl(_) => AppError::input(message),
            GitError::Cancelled => AppError::cancelled(),
            _ => AppError::external(message),
        }
    })?;
    Ok(SourceReferences {
        references: listing
            .refs
            .into_iter()
            .map(|reference| SourceReference {
                kind: match reference.kind {
                    RemoteRefKind::Branch => "branch",
                    RemoteRefKind::Tag => "tag",
                },
                name: reference.name,
                object: reference.object,
            })
            .collect(),
        truncated: listing.truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn rejects_local_input_and_invalid_remotes_without_git() {
        for input in [
            "C:\\projects\\repo",
            "/tmp/project",
            "file:///tmp/project",
            "https://127.0.0.1/org/repo",
        ] {
            let error = source_references(input, UrlPolicy::default(), &CancellationToken::new())
                .unwrap_err();
            assert_eq!(error.kind, ErrorKind::Input);
        }
    }

    #[test]
    fn rejects_invalid_query_before_contacting_git() {
        let cancel = CancellationToken::new();
        for (kind, search) in [
            (Some("commit"), None),
            (Some("tag"), Some("a\ncontrol")),
        ] {
            let error = source_references_matching(
                "https://github.com/org/repo",
                UrlPolicy::default(),
                kind,
                search,
                &cancel,
            )
            .unwrap_err();
            assert_eq!(error.kind, ErrorKind::Input);
        }
    }

    #[test]
    fn cancellation_is_reported_without_network() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = source_references("https://github.com/org/repo", UrlPolicy::default(), &cancel)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Cancelled);
    }
}
