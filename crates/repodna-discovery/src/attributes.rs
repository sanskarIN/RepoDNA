//! Linguist attributes from `.gitattributes` files.
//!
//! Repositories mark generated, vendored, and documentation files for GitHub's language
//! statistics with the `linguist-generated`, `linguist-vendored`, and
//! `linguist-documentation` attributes. RepoDNA honors the same markers, so a repository
//! that is already configured for GitHub needs no RepoDNA configuration.
//!
//! Patterns are translated to repository-relative globs with `.gitignore` semantics (see
//! [`repodna_core::glob`]): a pattern without a slash matches at any depth below the
//! `.gitattributes` file, and one with a slash is relative to it.

use std::collections::HashSet;

/// Paths marked by linguist attributes, as repository-relative glob patterns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinguistAttributes {
    /// `linguist-generated`.
    pub generated: Vec<String>,
    /// `linguist-vendored`.
    pub vendored: Vec<String>,
    /// `linguist-documentation`.
    pub documentation: Vec<String>,
    /// Paths where one of the attributes is explicitly turned off
    /// (`-linguist-generated`, `linguist-vendored=false`, …): first-party files.
    pub first_party: Vec<String>,
}

impl LinguistAttributes {
    /// `true` when no linguist attribute was found.
    pub fn is_empty(&self) -> bool {
        self.generated.is_empty()
            && self.vendored.is_empty()
            && self.documentation.is_empty()
            && self.first_party.is_empty()
    }

    /// Adds the linguist attributes of one `.gitattributes` file located in `directory`
    /// (repository-relative, empty for the root).
    pub fn add_file(&mut self, directory: &str, text: &str) {
        visit_patterns(directory, text, |kind, glob| {
            let list = kind.list(self);
            if !list.iter().any(|existing| existing == glob) {
                list.push(glob.to_owned());
            }
        });
    }
}

/// Collects rules across many attribute files without repeatedly scanning prior rules.
///
/// Insertion order is preserved within each category. Membership indexes are
/// temporary and are dropped when collection finishes.
#[derive(Debug, Default)]
pub struct LinguistAttributeCollector {
    attributes: LinguistAttributes,
    seen: HashSet<(AttributeKind, String)>,
}

impl LinguistAttributeCollector {
    /// Adds one file, returning whether it contributed a previously unseen rule.
    pub fn add_file(&mut self, directory: &str, text: &str) -> bool {
        let mut changed = false;
        visit_patterns(directory, text, |kind, glob| {
            if self.seen.insert((kind, glob.to_owned())) {
                kind.list(&mut self.attributes).push(glob.to_owned());
                changed = true;
            }
        });
        changed
    }

    /// Returns the ordered rules, releasing the temporary membership index.
    pub fn finish(self) -> LinguistAttributes {
        self.attributes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AttributeKind {
    Generated,
    Vendored,
    Documentation,
    FirstParty,
}

impl AttributeKind {
    fn list(self, attributes: &mut LinguistAttributes) -> &mut Vec<String> {
        match self {
            Self::Generated => &mut attributes.generated,
            Self::Vendored => &mut attributes.vendored,
            Self::Documentation => &mut attributes.documentation,
            Self::FirstParty => &mut attributes.first_party,
        }
    }
}

fn visit_patterns(directory: &str, text: &str, mut visit: impl FnMut(AttributeKind, &str)) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("[attr]") {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(pattern) = fields.next() else {
            continue;
        };
        // Quoted patterns (with escapes) and negated patterns are not valid for
        // attributes in the forms RepoDNA supports; skip them rather than guess.
        if pattern.starts_with('"') || pattern.starts_with('!') {
            continue;
        }
        let glob = to_glob(directory, pattern);
        for attribute in fields {
            let (name, value) = match attribute.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (attribute, None),
            };
            let (name, set) = match name.strip_prefix('-') {
                Some(name) => (name, false),
                None if name.starts_with('!') => continue,
                None => (name, value.is_none_or(|v| !matches!(v, "false" | "0"))),
            };
            let kind = match (name, set) {
                (_, false)
                    if matches!(
                        name,
                        "linguist-generated" | "linguist-vendored" | "linguist-documentation"
                    ) =>
                {
                    AttributeKind::FirstParty
                }
                ("linguist-generated", true) => AttributeKind::Generated,
                ("linguist-vendored", true) => AttributeKind::Vendored,
                ("linguist-documentation", true) => AttributeKind::Documentation,
                _ => continue,
            };
            visit(kind, &glob);
        }
    }
}

/// Translates a `.gitattributes` pattern in `directory` to a repository-relative glob.
fn to_glob(directory: &str, pattern: &str) -> String {
    let anchored = pattern.starts_with('/') || pattern.trim_end_matches('/').contains('/');
    let pattern = pattern.trim_start_matches('/');
    match (directory.is_empty(), anchored) {
        (true, true) => format!("/{pattern}"),
        (true, false) => pattern.to_owned(),
        (false, true) => format!("/{directory}/{pattern}"),
        (false, false) => format!("/{directory}/**/{pattern}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::glob::GlobSet;

    #[test]
    fn reads_linguist_attributes() {
        let mut attributes = LinguistAttributes::default();
        attributes.add_file(
            "",
            "# Generated files\n*.pb.go linguist-generated\nschemas/*.json linguist-generated=true -diff\nthird_party/** linguist-vendored\ndocs/** linguist-documentation\nvendor/ours/** -linguist-vendored\n*.txt text eol=lf\n[attr]binary -diff -merge -text\n",
        );
        attributes.add_file(
            "web",
            "dist/** linguist-generated\nlegacy.js linguist-vendored=false\n",
        );
        assert_eq!(
            attributes.generated,
            vec!["*.pb.go", "/schemas/*.json", "/web/dist/**"]
        );
        assert_eq!(attributes.vendored, vec!["/third_party/**"]);
        assert_eq!(attributes.documentation, vec!["/docs/**"]);
        assert_eq!(
            attributes.first_party,
            vec!["/vendor/ours/**", "/web/**/legacy.js"]
        );
        assert!(!attributes.is_empty());
    }

    #[test]
    fn collector_preserves_category_order_and_reports_only_new_rules() {
        let files = [
            (
                "",
                "*.js linguist-generated linguist-vendored\n*.js linguist-generated\n",
            ),
            ("", "*.js linguist-vendored\n*.js linguist-generated\n"),
            ("", "*.js -linguist-generated -linguist-vendored\n"),
            (
                "nested",
                "*.js linguist-generated\n*.md linguist-documentation\n",
            ),
            ("nested", "*.md linguist-documentation\n"),
            ("", "*.txt text eol=lf\n"),
        ];
        let mut direct = LinguistAttributes::default();
        let mut collector = LinguistAttributeCollector::default();
        for ((directory, text), expected) in files
            .into_iter()
            .zip([true, false, true, true, false, false])
        {
            let before = direct.clone();
            direct.add_file(directory, text);
            assert_eq!(direct != before, expected);
            assert_eq!(collector.add_file(directory, text), expected);
        }
        assert_eq!(collector.finish(), direct);
    }

    #[test]
    fn collector_matches_direct_insertion_for_many_nested_files() {
        let mut direct = LinguistAttributes::default();
        let mut collector = LinguistAttributeCollector::default();
        for index in 0..512 {
            let directory = format!("packages/p{}", index % 256);
            let text = "gen/** linguist-generated\n*.js linguist-vendored\nREADME.md linguist-documentation\n/src/main.js -linguist-generated\n";
            direct.add_file(&directory, text);
            assert_eq!(collector.add_file(&directory, text), index < 256);
        }
        let collected = collector.finish();
        assert_eq!(collected.generated.len(), 256);
        assert_eq!(collected.vendored.len(), 256);
        assert_eq!(collected.documentation.len(), 256);
        assert_eq!(collected.first_party.len(), 256);
        assert_eq!(collected, direct);
    }

    #[test]
    fn globs_match_like_git() {
        let mut attributes = LinguistAttributes::default();
        attributes.add_file(
            "",
            "*.pb.go linguist-generated\n/root.json linguist-generated\n",
        );
        attributes.add_file("app", "gen/*.ts linguist-generated\n");
        let generated = GlobSet::new(&attributes.generated).unwrap();
        assert!(generated.matches("api/v1/service.pb.go"));
        assert!(generated.matches("root.json"));
        assert!(!generated.matches("nested/root.json"));
        assert!(generated.matches("app/gen/types.ts"));
        assert!(!generated.matches("gen/types.ts"));
        assert!(LinguistAttributes::default().is_empty());
    }
}
