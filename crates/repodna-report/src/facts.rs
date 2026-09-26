//! Headline facts shared by the card, badges, summaries, and comparisons.
//!
//! Values are `None` when the analysis that would produce them did not run, so renderers
//! can show "not analyzed" instead of a misleading zero.

use repodna_core::model::artifact::RepositoryDna;
use repodna_core::time::Timestamp;

/// Languages shown by name before the rest fold into "Other".
pub const NAMED_LANGUAGES: usize = 7;

/// A language's share of first-party code.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageShare {
    /// Identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Share (0–1).
    pub share: f64,
    /// Categorical slot, or `None` for languages folded into "Other".
    pub slot: Option<usize>,
}

/// Headline facts about a repository snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    /// Repository name.
    pub name: String,
    /// Owner or namespace.
    pub owner: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Languages by share of first-party code, largest first.
    pub languages: Vec<LanguageShare>,
    /// Files discovered.
    pub files: u64,
    /// Code lines.
    pub code_lines: u64,
    /// Commits, when history was analyzed.
    pub commits: Option<u64>,
    /// Contributors, when history was analyzed.
    pub contributors: Option<u64>,
    /// Days between the first and latest commit.
    pub age_days: Option<i64>,
    /// Direct dependencies, when dependencies were analyzed.
    pub dependencies: Option<u64>,
    /// Architecture style, when architecture was inferred.
    pub architecture: Option<String>,
    /// Activity level, when history was analyzed.
    pub activity: Option<String>,
    /// Files with tests (test files and source files with inline tests), when tests were
    /// analyzed.
    pub test_files: Option<u64>,
    /// DNA hash.
    pub dna_hash: String,
    /// When the analysis was generated.
    pub generated: Timestamp,
    /// Short revision.
    pub revision: Option<String>,
    /// Web location of the repository (`host/owner/name`), when a remote is recorded.
    pub location: Option<String>,
}

/// Turns a remote URL into `host/path` without scheme, user, or `.git` suffix.
pub fn web_location(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = if let Some((_, rest)) = url.split_once("://") {
        rest.to_owned()
    } else if let Some((user_host, path)) = url.split_once(':') {
        // scp-like syntax: git@host:owner/name.git
        let host = user_host.rsplit('@').next()?;
        if host.contains('/') || path.starts_with('/') {
            return None;
        }
        format!("{host}/{path}")
    } else {
        return None;
    };
    let rest = rest
        .rsplit_once('@')
        .map_or(rest.as_str(), |(_, host)| host);
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    (!rest.is_empty() && rest.contains('/')).then(|| rest.to_owned())
}

/// Languages with a share of first-party code, largest first, with categorical slots.
pub fn language_shares(dna: &RepositoryDna) -> Vec<LanguageShare> {
    let mut languages: Vec<&repodna_core::model::languages::LanguageStat> = dna
        .languages
        .languages
        .iter()
        .filter(|language| language.share > 0.0)
        .collect();
    languages.sort_by(|a, b| b.share.total_cmp(&a.share).then_with(|| a.id.cmp(&b.id)));
    languages
        .into_iter()
        .enumerate()
        .map(|(index, language)| LanguageShare {
            id: language.id.clone(),
            name: language.name.clone(),
            share: language.share,
            slot: (index < NAMED_LANGUAGES).then_some(index),
        })
        .collect()
}

/// Languages for charts: the named ones plus a single folded "Other" entry.
pub fn folded_languages(shares: &[LanguageShare]) -> Vec<LanguageShare> {
    let mut folded: Vec<LanguageShare> = shares
        .iter()
        .filter(|language| language.slot.is_some())
        .cloned()
        .collect();
    let other: f64 = shares
        .iter()
        .filter(|language| language.slot.is_none())
        .map(|language| language.share)
        .sum();
    if other > 0.0 {
        folded.push(LanguageShare {
            id: "other".to_owned(),
            name: "Other".to_owned(),
            share: other,
            slot: None,
        });
    }
    folded
}

/// Extracts the headline facts.
pub fn facts(dna: &RepositoryDna) -> Facts {
    let git = dna.git.status.has_results().then_some(&dna.git);
    Facts {
        name: dna.identity.name.clone(),
        owner: dna.identity.owner.clone(),
        description: dna
            .identity
            .description
            .clone()
            .or_else(|| dna.docs.description.clone()),
        languages: language_shares(dna),
        files: dna.structure.total_files,
        code_lines: dna.structure.code_lines,
        commits: git.map(|git| git.commit_count),
        contributors: git.map(|git| git.contributors.len() as u64),
        age_days: git.and_then(|git| {
            let (first, last) = (git.first_commit.as_ref()?, git.last_commit.as_ref()?);
            Some(first.timestamp.days_until(last.timestamp).max(0))
        }),
        dependencies: dna
            .dependencies
            .status
            .has_results()
            .then_some(dna.dependencies.direct_count),
        architecture: (dna.architecture.status.has_results()
            && dna.architecture.style != "Unknown")
            .then(|| dna.architecture.style.clone()),
        activity: git.map(|git| git.activity.level.label().to_owned()),
        test_files: dna
            .tests
            .status
            .has_results()
            .then_some(dna.tests.test_files + dna.tests.inline_test_files),
        dna_hash: dna.fingerprint.dna_hash.clone(),
        generated: dna.analysis_metadata.generated_at,
        revision: dna
            .analysis_metadata
            .revision
            .as_ref()
            .map(|revision| revision.chars().take(7).collect()),
        location: dna
            .identity
            .remotes
            .iter()
            .find(|remote| remote.name == "origin")
            .or_else(|| dna.identity.remotes.first())
            .and_then(|remote| web_location(&remote.url)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::languages::{LanguageKind, LanguageStat, ParserCapability};
    use repodna_core::model::metadata::AnalysisMetadata;

    #[test]
    fn derives_web_locations() {
        assert_eq!(
            web_location("https://github.com/acme/widget.git").as_deref(),
            Some("github.com/acme/widget")
        );
        assert_eq!(
            web_location("git@gitlab.com:group/sub/tool.git").as_deref(),
            Some("gitlab.com/group/sub/tool")
        );
        assert_eq!(
            web_location("ssh://git@host.example:2222/a/b").as_deref(),
            Some("host.example:2222/a/b")
        );
        assert_eq!(web_location("/srv/git/repo.git"), None);
        assert_eq!(web_location("C:/work/repo"), None);
    }

    #[test]
    fn marks_unanalyzed_facts_as_missing() {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        let language = |id: &str, share: f64| LanguageStat {
            id: id.into(),
            name: id.to_uppercase(),
            kind: LanguageKind::Programming,
            files: 1,
            bytes: 1,
            code_lines: 1,
            comment_lines: 0,
            blank_lines: 0,
            share,
            capability: ParserCapability::Lexical,
        };
        dna.languages.languages = (0..9)
            .map(|i| language(&format!("l{i}"), 0.1 - f64::from(i) * 0.005))
            .collect();
        dna.tests.status = SectionStatus::Analyzed;
        dna.tests.test_files = 4;
        let facts = facts(&dna);
        assert_eq!(facts.commits, None);
        assert_eq!(facts.architecture, None);
        assert_eq!(facts.test_files, Some(4));
        assert_eq!(facts.languages.len(), 9);
        assert_eq!(facts.languages[6].slot, Some(6));
        assert_eq!(facts.languages[7].slot, None);
        let folded = folded_languages(&facts.languages);
        assert_eq!(folded.len(), NAMED_LANGUAGES + 1);
        assert_eq!(folded.last().unwrap().name, "Other");
    }
}
