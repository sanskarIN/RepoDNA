//! Report sections and which of them to include.

use std::fmt;
use std::str::FromStr;

/// A section of the full report, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Section {
    /// Title page with the DNA card.
    Cover,
    /// Executive summary and first-look answers.
    Summary,
    /// Repository identity.
    Identity,
    /// Language composition.
    Languages,
    /// Project structure.
    Structure,
    /// Inferred architecture.
    Architecture,
    /// Dependencies.
    Dependencies,
    /// Change hotspots.
    Hotspots,
    /// Complexity signals.
    Complexity,
    /// Duplication and similarity.
    Duplication,
    /// Tests.
    Tests,
    /// Build, environment, and CI.
    Build,
    /// Documentation.
    Documentation,
    /// Security signals.
    Security,
    /// Git history.
    History,
    /// Contributor analytics.
    Contributors,
    /// Codebase Time Machine.
    TimeMachine,
    /// Architectural evolution and the codebase story.
    Evolution,
    /// Major findings.
    Findings,
    /// Onboarding guide.
    Onboarding,
    /// Evidence appendix.
    Evidence,
    /// Raw metrics appendix.
    Metrics,
    /// Tool and version metadata.
    Metadata,
}

impl Section {
    /// Every section in report order.
    pub const ALL: [Section; 23] = [
        Section::Cover,
        Section::Summary,
        Section::Identity,
        Section::Languages,
        Section::Structure,
        Section::Architecture,
        Section::Dependencies,
        Section::Hotspots,
        Section::Complexity,
        Section::Duplication,
        Section::Tests,
        Section::Build,
        Section::Documentation,
        Section::Security,
        Section::History,
        Section::Contributors,
        Section::TimeMachine,
        Section::Evolution,
        Section::Findings,
        Section::Onboarding,
        Section::Evidence,
        Section::Metrics,
        Section::Metadata,
    ];

    /// Identifier used in configuration and anchors.
    pub const fn id(self) -> &'static str {
        match self {
            Section::Cover => "cover",
            Section::Summary => "summary",
            Section::Identity => "identity",
            Section::Languages => "languages",
            Section::Structure => "structure",
            Section::Architecture => "architecture",
            Section::Dependencies => "dependencies",
            Section::Hotspots => "hotspots",
            Section::Complexity => "complexity",
            Section::Duplication => "duplication",
            Section::Tests => "tests",
            Section::Build => "build",
            Section::Documentation => "documentation",
            Section::Security => "security",
            Section::History => "history",
            Section::Contributors => "contributors",
            Section::TimeMachine => "time-machine",
            Section::Evolution => "evolution",
            Section::Findings => "findings",
            Section::Onboarding => "onboarding",
            Section::Evidence => "evidence",
            Section::Metrics => "metrics",
            Section::Metadata => "metadata",
        }
    }

    /// Heading shown in reports.
    pub const fn title(self) -> &'static str {
        match self {
            Section::Cover => "Project DNA",
            Section::Summary => "Executive summary",
            Section::Identity => "Repository identity",
            Section::Languages => "Language map",
            Section::Structure => "Project structure",
            Section::Architecture => "Architecture",
            Section::Dependencies => "Dependencies",
            Section::Hotspots => "Code hotspots",
            Section::Complexity => "Complexity signals",
            Section::Duplication => "Duplication signals",
            Section::Tests => "Tests",
            Section::Build => "Build information",
            Section::Documentation => "Documentation",
            Section::Security => "Security signals",
            Section::History => "Git history",
            Section::Contributors => "Contributor analytics",
            Section::TimeMachine => "Codebase Time Machine",
            Section::Evolution => "Architectural evolution",
            Section::Findings => "Major findings",
            Section::Onboarding => "Onboarding guide",
            Section::Evidence => "Evidence appendix",
            Section::Metrics => "Raw metrics appendix",
            Section::Metadata => "Tool and version metadata",
        }
    }

    const fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for Section {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        Section::ALL
            .into_iter()
            .find(|section| section.id() == normalized)
            .ok_or_else(|| {
                let valid: Vec<&str> = Section::ALL.iter().map(|s| s.id()).collect();
                format!(
                    "unknown report section {value:?}; expected one of {}",
                    valid.join(", ")
                )
            })
    }
}

/// A set of report sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionSet(u32);

impl Default for SectionSet {
    fn default() -> Self {
        Self::all()
    }
}

impl SectionSet {
    /// Every section.
    pub fn all() -> Self {
        Self(Section::ALL.iter().fold(0, |bits, s| bits | s.bit()))
    }

    /// Parses section identifiers; an empty list means every section.
    pub fn parse<S: AsRef<str>>(ids: &[S]) -> Result<Self, String> {
        if ids.is_empty() {
            return Ok(Self::all());
        }
        let mut bits = 0;
        for id in ids {
            bits |= id.as_ref().parse::<Section>()?.bit();
        }
        Ok(Self(bits))
    }

    /// Returns `true` if the section is included.
    pub const fn contains(self, section: Section) -> bool {
        self.0 & section.bit() != 0
    }

    /// Included sections in report order.
    pub fn iter(self) -> impl Iterator<Item = Section> {
        Section::ALL.into_iter().filter(move |s| self.contains(*s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_section_lists() {
        let all = SectionSet::parse::<&str>(&[]).unwrap();
        assert_eq!(all.iter().count(), Section::ALL.len());
        let some = SectionSet::parse(&["summary", "Time_Machine"]).unwrap();
        assert_eq!(
            some.iter().collect::<Vec<_>>(),
            vec![Section::Summary, Section::TimeMachine]
        );
        let error = SectionSet::parse(&["nope"]).unwrap_err();
        assert!(error.contains("expected one of cover, summary"));
    }

    #[test]
    fn identifiers_round_trip() {
        for section in Section::ALL {
            assert_eq!(section.id().parse::<Section>().unwrap(), section);
            assert!(!section.title().is_empty());
        }
    }
}
