//! Application errors, each with a category that maps to a documented exit code and,
//! where possible, a hint that says what to do next.

use repodna_ai::AiError;
use repodna_core::error::CoreError;
use repodna_engine::EngineError;
use repodna_git::GitError;
use repodna_report::ReportError;
use repodna_store::StoreError;

/// What kind of failure an error is. Each kind has a stable exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// An unexpected internal failure (exit code 1).
    Internal,
    /// Invalid command-line usage (exit code 2).
    Usage,
    /// The input was not found, unreadable, or unsupported (exit code 3).
    Input,
    /// Invalid configuration (exit code 4).
    Config,
    /// A CI policy was violated (exit code 5).
    Policy,
    /// Local storage could not be read or written (exit code 6).
    Storage,
    /// An external tool or service failed: Git, the network, an AI provider (exit code 7).
    External,
    /// The user cancelled the operation (exit code 130).
    Cancelled,
}

impl ErrorKind {
    /// The process exit code for this kind of error.
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Internal => 1,
            Self::Usage => 2,
            Self::Input => 3,
            Self::Config => 4,
            Self::Policy => 5,
            Self::Storage => 6,
            Self::External => 7,
            Self::Cancelled => 130,
        }
    }

    /// Every kind, in exit-code order.
    pub const ALL: [ErrorKind; 8] = [
        Self::Internal,
        Self::Usage,
        Self::Input,
        Self::Config,
        Self::Policy,
        Self::Storage,
        Self::External,
        Self::Cancelled,
    ];

    /// What the exit code means, for documentation and `--help`.
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::Internal => "unexpected internal error",
            Self::Usage => "invalid command-line usage",
            Self::Input => "input not found, unreadable, or unsupported",
            Self::Config => "invalid configuration",
            Self::Policy => "CI policy violated (findings at or above the --fail-on severity)",
            Self::Storage => "local storage could not be read or written",
            Self::External => "an external tool or service failed (Git, network, AI provider)",
            Self::Cancelled => "cancelled by the user",
        }
    }
}

/// An error with a category, a message, and an optional hint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    /// Category.
    pub kind: ErrorKind,
    /// What went wrong.
    pub message: String,
    /// What to do about it.
    pub hint: Option<String>,
}

impl AppError {
    /// Creates an error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Adds a hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// An internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    /// A usage error.
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, message)
    }

    /// An input error.
    pub fn input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Input, message)
    }

    /// A configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, message)
    }

    /// A policy violation.
    pub fn policy(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Policy, message)
    }

    /// A storage error.
    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, message).with_hint(STORAGE_HINT)
    }

    /// A failure of an external tool or service.
    pub fn external(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::External, message)
    }

    /// Cancellation.
    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "cancelled")
    }

    /// The process exit code.
    pub fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }
}

/// Hint attached to storage errors.
const STORAGE_HINT: &str = "Run `repodna cache repair`. If that does not help, `repodna cache reset` recreates local storage; your repositories are never modified.";

impl From<EngineError> for AppError {
    fn from(error: EngineError) -> Self {
        let kind = match &error {
            EngineError::NotFound(_)
            | EngineError::UnsupportedInput(_)
            | EngineError::Discovery(_)
            | EngineError::Archive(_)
            | EngineError::Io { .. } => ErrorKind::Input,
            EngineError::Config(_) => ErrorKind::Config,
            EngineError::Cancelled => ErrorKind::Cancelled,
            EngineError::Git(git) => match git {
                GitError::Cancelled => ErrorKind::Cancelled,
                GitError::InvalidUrl(_) | GitError::NotARepository(_) => ErrorKind::Input,
                _ => ErrorKind::External,
            },
        };
        let hint = error.hint();
        let mut app = AppError::new(kind, error.to_string());
        app.hint = hint;
        app
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        AppError::storage(error.to_string())
    }
}

impl From<CoreError> for AppError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::Config { .. } => AppError::config(error.to_string()),
            _ => AppError::input(error.to_string()),
        }
    }
}

impl From<AiError> for AppError {
    fn from(error: AiError) -> Self {
        let kind = match &error {
            AiError::Disabled
            | AiError::Configuration(_)
            | AiError::MissingApiKey(_)
            | AiError::RemoteNotAllowed { .. } => ErrorKind::Config,
            AiError::UnknownSubject(_) => ErrorKind::Usage,
            AiError::Cancelled => ErrorKind::Cancelled,
            _ => ErrorKind::External,
        };
        let hint = match &error {
            AiError::Disabled => Some(
                "AI is optional; every other command works without it. See docs/ai.md to configure a provider.",
            ),
            AiError::Timeout(_) => Some("Raise ai.timeout_seconds in your user configuration."),
            _ => None,
        };
        let mut app = AppError::new(kind, error.to_string());
        app.hint = hint.map(str::to_owned);
        app
    }
}

impl From<ReportError> for AppError {
    fn from(error: ReportError) -> Self {
        match error {
            ReportError::Options(_) => AppError::usage(error.to_string()),
            ReportError::Io { .. } => AppError::input(error.to_string()),
            _ => AppError::internal(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn maps_errors_to_exit_codes() {
        let codes: Vec<i32> = ErrorKind::ALL.iter().map(|k| k.exit_code()).collect();
        assert_eq!(codes, vec![1, 2, 3, 4, 5, 6, 7, 130]);
        let missing: AppError = EngineError::NotFound(PathBuf::from("x")).into();
        assert_eq!(missing.exit_code(), 3);
        assert!(missing.hint.is_some());
        let cancelled: AppError = EngineError::Cancelled.into();
        assert_eq!(cancelled.kind, ErrorKind::Cancelled);
        let git: AppError = EngineError::Git(GitError::NotInstalled).into();
        assert_eq!(git.kind, ErrorKind::External);
        let storage: AppError = StoreError::NoHome.into();
        assert_eq!(storage.exit_code(), 6);
        assert!(storage.hint.unwrap().contains("cache repair"));
        let ai: AppError = AiError::Disabled.into();
        assert_eq!(ai.kind, ErrorKind::Config);
        let subject: AppError = AiError::UnknownSubject("no".into()).into();
        assert_eq!(subject.kind, ErrorKind::Usage);
        assert!(ErrorKind::ALL.iter().all(|k| !k.meaning().is_empty()));
    }
}
