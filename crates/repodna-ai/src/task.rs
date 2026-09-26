//! What an explanation is about.

use crate::text::sanitize;

/// Longest question accepted, in characters.
pub const MAX_QUESTION_CHARS: usize = 2_000;

/// An explanation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiTask {
    /// Explain the whole repository to a newcomer.
    Repository,
    /// Explain the inferred architecture.
    Architecture,
    /// Summarize how the repository evolved.
    History,
    /// Explain the dependency relationships.
    Dependencies,
    /// Draft an onboarding guide.
    Onboarding,
    /// Explain one module or directory.
    Module(String),
    /// Explain why a file is (or is not) a hotspot.
    Hotspot(String),
    /// Answer a question about the repository.
    Question(String),
}

/// Task identifiers accepted by [`AiTask::parse`].
pub const TASK_IDS: [&str; 8] = [
    "repository",
    "architecture",
    "history",
    "dependencies",
    "onboarding",
    "module",
    "hotspot",
    "question",
];

impl AiTask {
    /// Builds a task from its identifier and, for `module`, `hotspot`, and `question`, its
    /// subject.
    pub fn parse(id: &str, subject: Option<&str>) -> Result<Self, String> {
        let subject = subject.map(str::trim).filter(|s| !s.is_empty());
        let needs = |what: &str| format!("the {id} task needs {what}");
        match (id, subject) {
            ("repository", None) => Ok(Self::Repository),
            ("architecture", None) => Ok(Self::Architecture),
            ("history", None) => Ok(Self::History),
            ("dependencies", None) => Ok(Self::Dependencies),
            ("onboarding", None) => Ok(Self::Onboarding),
            ("module", Some(path)) => Ok(Self::Module(path.trim_matches('/').to_owned())),
            ("hotspot", Some(path)) => Ok(Self::Hotspot(path.trim_matches('/').to_owned())),
            ("question", Some(text)) => {
                if text.chars().count() > MAX_QUESTION_CHARS {
                    Err(format!(
                        "questions are limited to {MAX_QUESTION_CHARS} characters"
                    ))
                } else {
                    Ok(Self::Question(text.to_owned()))
                }
            }
            ("module" | "hotspot", None) => Err(needs("a path")),
            ("question", None) => Err(needs("a question")),
            (
                "repository" | "architecture" | "history" | "dependencies" | "onboarding",
                Some(_),
            ) => Err(format!("the {id} task takes no subject")),
            _ => Err(format!(
                "unknown explanation `{id}`; expected one of: {}",
                TASK_IDS.join(", ")
            )),
        }
    }

    /// Stable identifier.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Architecture => "architecture",
            Self::History => "history",
            Self::Dependencies => "dependencies",
            Self::Onboarding => "onboarding",
            Self::Module(_) => "module",
            Self::Hotspot(_) => "hotspot",
            Self::Question(_) => "question",
        }
    }

    /// The path or question the task is about, if any.
    pub fn subject(&self) -> Option<&str> {
        match self {
            Self::Module(subject) | Self::Hotspot(subject) | Self::Question(subject) => {
                Some(subject)
            }
            _ => None,
        }
    }

    /// Human-readable title.
    pub fn title(&self) -> String {
        match self {
            Self::Repository => "Repository explanation".to_owned(),
            Self::Architecture => "Architecture explanation".to_owned(),
            Self::History => "Evolution summary".to_owned(),
            Self::Dependencies => "Dependency explanation".to_owned(),
            Self::Onboarding => "Onboarding guide".to_owned(),
            Self::Module(path) => format!("Module explanation: {path}"),
            Self::Hotspot(path) => format!("Hotspot explanation: {path}"),
            Self::Question(_) => "Answer".to_owned(),
        }
    }

    /// What the model is asked to do.
    pub fn instructions(&self) -> String {
        match self {
            Self::Repository => "Explain this repository to a developer who is seeing it for the first time: its likely purpose, major components, entry points, main dependencies, how it is built and tested, development workflow signals, the files to read first, and how it changed recently.".to_owned(),
            Self::Architecture => "Explain how the code is organized: the inferred architecture style, the main modules and which depend on which, layering, dependency cycles, and which components are central and why they matter.".to_owned(),
            Self::History => "Summarize how the repository evolved: how it started, periods of growth and inactivity, releases, the areas that changed most, and what changed recently.".to_owned(),
            Self::Dependencies => "Explain the dependency relationships: which external packages matter most and where they are used, how internal modules depend on each other, and any concentration, duplicate-version, or cycle risks the evidence shows.".to_owned(),
            Self::Onboarding => "Write an onboarding guide as ordered points: what to read first, how to set up, build, and test, where the main code lives, and the terms a newcomer needs.".to_owned(),
            Self::Module(path) => format!("Explain the module or directory `{}`: what it appears to contain, what depends on it and what it depends on, how it has changed, and any risks the analysis found in it.", sanitize(path, 300)),
            Self::Hotspot(path) => format!("Explain whether and why `{}` is a hotspot: what its change frequency, size, complexity, and dependents say, and what a maintainer might check first.", sanitize(path, 300)),
            Self::Question(question) => format!("Answer the user's question using only the evidence. The question: {}", sanitize(question, MAX_QUESTION_CHARS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tasks_and_subjects() {
        assert_eq!(AiTask::parse("repository", None), Ok(AiTask::Repository));
        assert_eq!(
            AiTask::parse("module", Some("src/net/")),
            Ok(AiTask::Module("src/net".into()))
        );
        assert!(AiTask::parse("module", None).is_err());
        assert!(AiTask::parse("history", Some("x")).is_err());
        assert!(
            AiTask::parse("poem", None)
                .unwrap_err()
                .contains("expected one of")
        );
        let long = "?".repeat(MAX_QUESTION_CHARS + 1);
        assert!(AiTask::parse("question", Some(&long)).is_err());
        let task = AiTask::parse("hotspot", Some("src/a.rs")).unwrap();
        assert_eq!(task.id(), "hotspot");
        assert_eq!(task.subject(), Some("src/a.rs"));
        assert!(task.instructions().contains("`src/a.rs`"));
    }
}
