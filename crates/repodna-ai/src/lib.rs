//! Optional AI explanations for RepoDNA.
//!
//! AI sits above the deterministic analysis and is off by default. When a provider is
//! configured, RepoDNA selects a small set of numbered facts from the analysis artifact,
//! sends them as data (never as instructions), and asks for a structured answer whose
//! every statement cites those facts. Answers are checked against the evidence that was
//! sent: statements without valid citations are labeled unsupported, and statements the
//! model marks as inferences are labeled as such. The model has no tools: it cannot run
//! commands, read files, or reach the network through RepoDNA.
//!
//! Providers: a local command that reads the prompt on standard input (for example
//! `ollama run <model>`), any OpenAI-compatible HTTP server, and the Anthropic Messages
//! API. Providers that send data off the machine require explicit permission.

pub mod context;
pub mod error;
pub mod prompt;
pub mod provider;
pub mod response;
pub mod task;
pub mod text;

#[cfg(test)]
mod testing;

pub use context::{Context, ContextOptions, EvidenceItem, EvidenceKind, select_context};
pub use error::AiError;
pub use prompt::{PROMPT_VERSION, Prompt, build_prompt};
pub use provider::{AiProvider, Completion, CompletionRequest, build_provider, max_output_tokens};
pub use response::{Answer, AnswerPoint, Support, check_answer};
pub use task::AiTask;
