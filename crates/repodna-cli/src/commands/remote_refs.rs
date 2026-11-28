//! List advertised remote branches and tags without cloning or creating analyses.

use repodna_app::{AppError, SourceReferences, source_references};
use repodna_git::url::UrlPolicy;

use super::{Ctx, print, to_json};
use crate::cli::{RemoteRefFilter, RemoteRefsCmd};

fn filter(listing: &mut SourceReferences, kind: RemoteRefFilter, search: Option<&str>) {
    let query = search.unwrap_or_default().trim().to_lowercase();
    listing.references.retain(|reference| {
        let matches_kind = match kind {
            RemoteRefFilter::All => true,
            RemoteRefFilter::Branch => reference.kind == "branch",
            RemoteRefFilter::Tag => reference.kind == "tag",
        };
        matches_kind && reference.name.to_lowercase().contains(&query)
    });
}

fn render(listing: &SourceReferences) -> String {
    let mut out = String::from("Kind\tName\tAdvertised object\n");
    for reference in &listing.references {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            reference.kind, reference.name, reference.object
        ));
    }
    if listing.references.is_empty() {
        out.push_str("No matching references.\n");
    }
    if listing.truncated {
        out.push_str("Listing limited; enter other reference names directly when analyzing.\n");
    }
    out
}

/// Discover remote branches and tags using the shared native source service.
pub fn run(ctx: &Ctx, cmd: &RemoteRefsCmd) -> Result<(), AppError> {
    let policy = UrlPolicy {
        allow_private_hosts: cmd.allow_private_hosts,
        allow_insecure: cmd.allow_insecure_urls,
    };
    let mut listing = source_references(&cmd.input, policy, &ctx.cancel)?;
    filter(&mut listing, cmd.kind, cmd.search.as_deref());
    if cmd.json {
        print(&to_json(&listing)?)
    } else {
        print(&render(&listing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_app::SourceReference;

    fn listing() -> SourceReferences {
        SourceReferences {
            references: vec![
                SourceReference {
                    kind: "branch",
                    name: "feat/Audio".to_owned(),
                    object: "a".repeat(40),
                },
                SourceReference {
                    kind: "tag",
                    name: "feat/Audio".to_owned(),
                    object: "b".repeat(40),
                },
                SourceReference {
                    kind: "branch",
                    name: "main".to_owned(),
                    object: "c".repeat(40),
                },
            ],
            truncated: true,
        }
    }

    #[test]
    fn filters_by_namespace_and_case_insensitive_substring_without_hiding_truncation() {
        let mut result = listing();
        filter(&mut result, RemoteRefFilter::Tag, Some(" AUDIO "));
        assert_eq!(result.references.len(), 1);
        assert_eq!(result.references[0].kind, "tag");
        assert!(result.truncated);
        let output = render(&result);
        assert!(output.contains("tag\tfeat/Audio\t"));
        assert!(output.contains("Listing limited"));
        assert!(!output.contains("branch\t"));
    }

    #[test]
    fn distinguishes_empty_results_from_complete_advertisement() {
        let mut result = listing();
        filter(&mut result, RemoteRefFilter::Branch, Some("not present"));
        assert!(result.references.is_empty());
        let output = render(&result);
        assert!(output.contains("No matching references"));
        assert!(output.contains("Listing limited"));
        result.truncated = false;
        assert!(!render(&result).contains("Listing limited"));
    }
}
