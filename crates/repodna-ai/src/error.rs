//! Errors raised by the AI layer.

use thiserror::Error;

/// An error from the optional AI layer.
#[derive(Debug, Error)]
pub enum AiError {
    /// No AI provider is configured.
    #[error(
        "AI features are off: set ai.provider in your user configuration (`repodna config path` shows where it is)"
    )]
    Disabled,
    /// The configuration is incomplete or invalid.
    #[error("invalid AI configuration: {0}")]
    Configuration(String),
    /// A provider that sends data off this machine was selected without permission.
    #[error(
        "the {provider} provider would send repository evidence to {host}; allow it with privacy.remote_ai = true in your user configuration or --allow-remote-ai"
    )]
    RemoteNotAllowed {
        /// Provider identifier.
        provider: String,
        /// Host that would receive the data.
        host: String,
    },
    /// The environment variable that should hold the API key is not set.
    #[error("the API key environment variable {0} is not set")]
    MissingApiKey(String),
    /// The request could not be sent or the response could not be read.
    #[error("could not reach the AI provider: {0}")]
    Transport(String),
    /// The provider answered with an error status.
    #[error("the AI provider returned HTTP {status}: {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message reported by the provider.
        message: String,
    },
    /// The provider's response could not be understood.
    #[error("unexpected response from the AI provider: {0}")]
    InvalidResponse(String),
    /// The model declined to answer.
    #[error("the model declined to answer this request")]
    Refused,
    /// The answer was cut off by the output limit.
    #[error(
        "the model's answer was cut off at {limit} output tokens; raise ai.max_output_tokens in your user configuration"
    )]
    Truncated {
        /// The output limit that was reached.
        limit: u32,
    },
    /// The local command failed.
    #[error("the AI command failed: {0}")]
    Command(String),
    /// The provider did not answer in time.
    #[error("the AI provider did not answer within {0} seconds")]
    Timeout(u64),
    /// The request was cancelled.
    #[error("cancelled")]
    Cancelled,
    /// The explanation subject does not exist in the analysis.
    #[error("{0}")]
    UnknownSubject(String),
}
