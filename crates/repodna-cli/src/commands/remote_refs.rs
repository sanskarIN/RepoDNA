//! List advertised remote branches and tags without cloning or creating analyses.

use repodna_app::{AppError, SourceReferences, source_references_matching};
use repodna_git::url::UrlPolicy;

use super::{Ctx, print, to_json};
use crate::cli::{RemoteRefFilter, RemoteRefsCmd};

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
    let kind = match cmd.kind {
        RemoteRefFilter::All => None,
        RemoteRefFilter::Branch => Some("branch"),
        RemoteRefFilter::Tag => Some("tag"),
    };
    let listing =
        source_references_matching(&cmd.input, policy, kind, cmd.search.as_deref(), &ctx.cancel)?;
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
    fn renders_filtered_list_without_hiding_truncation() {
        let mut result = listing();
        result
            .references
            .retain(|reference| reference.kind == "tag");
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
        result.references.clear();
        assert!(result.references.is_empty());
        let output = render(&result);
        assert!(output.contains("No matching references"));
        assert!(output.contains("Listing limited"));
        result.truncated = false;
        assert!(!render(&result).contains("Listing limited"));
    }
}
