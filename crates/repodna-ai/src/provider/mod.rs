//! Provider abstraction: a local command, OpenAI-compatible servers, and the Anthropic
//! Messages API. No provider gets tools; a request is text in, text out.

mod anthropic;
mod command;
mod http;
mod openai;

use repodna_core::CancellationToken;
use repodna_core::config::{AiConfig, AiProviderKind};

use crate::error::AiError;

pub use anthropic::{ANTHROPIC_ENDPOINT, AnthropicProvider};
pub use command::CommandProvider;
pub use http::Endpoint;
pub use openai::OpenAiCompatibleProvider;

/// Default output limit for Anthropic models, whose limit also covers thinking tokens.
pub const DEFAULT_ANTHROPIC_OUTPUT_TOKENS: u32 = 16_000;
/// Default output limit for other providers.
pub const DEFAULT_OUTPUT_TOKENS: u32 = 2_000;

/// One request to a model.
#[derive(Debug, Clone, Copy)]
pub struct CompletionRequest<'a> {
    /// Instructions.
    pub system: &'a str,
    /// Task and evidence.
    pub user: &'a str,
    /// Output limit.
    pub max_output_tokens: u32,
    /// Cancellation, honored by providers that can stop early.
    pub cancel: Option<&'a CancellationToken>,
}

/// A model's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// Answer text.
    pub text: String,
    /// Input tokens reported by the provider.
    pub input_tokens: Option<u64>,
    /// Output tokens reported by the provider.
    pub output_tokens: Option<u64>,
}

/// A model provider.
pub trait AiProvider: Send + Sync {
    /// Provider identifier, e.g. `anthropic`.
    fn id(&self) -> &'static str;
    /// Model identifier.
    fn model(&self) -> &str;
    /// `true` when requests leave this machine.
    fn remote(&self) -> bool;
    /// Where requests go: an endpoint URL or a command name.
    fn destination(&self) -> String;
    /// Sends one request.
    fn complete(&self, request: &CompletionRequest<'_>) -> Result<Completion, AiError>;
}

/// The output limit for `config`, with provider defaults.
pub fn max_output_tokens(config: &AiConfig) -> u32 {
    config.max_output_tokens.unwrap_or(match config.provider {
        AiProviderKind::Anthropic => DEFAULT_ANTHROPIC_OUTPUT_TOKENS,
        _ => DEFAULT_OUTPUT_TOKENS,
    })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn check_remote(endpoint: &Endpoint, provider: &str, allow_remote: bool) -> Result<(), AiError> {
    if endpoint.loopback || allow_remote {
        Ok(())
    } else {
        Err(AiError::RemoteNotAllowed {
            provider: provider.to_owned(),
            host: endpoint.host.clone(),
        })
    }
}

fn api_key(name: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<String, AiError> {
    env(name)
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .ok_or_else(|| AiError::MissingApiKey(name.to_owned()))
}

/// Builds the configured provider, reading API keys from the process environment.
///
/// Providers whose endpoint is not on this machine are refused unless `allow_remote` is
/// set (from `privacy.remote_ai` or `--allow-remote-ai`).
pub fn build_provider(
    config: &AiConfig,
    allow_remote: bool,
) -> Result<Box<dyn AiProvider>, AiError> {
    build_provider_with_env(config, allow_remote, &|name| std::env::var(name).ok())
}

/// Like [`build_provider`], with an explicit environment lookup.
pub fn build_provider_with_env(
    config: &AiConfig,
    allow_remote: bool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Box<dyn AiProvider>, AiError> {
    match config.provider {
        AiProviderKind::None => Err(AiError::Disabled),
        AiProviderKind::Command => {
            let (program, args) = config.command.split_first().ok_or_else(|| {
                AiError::Configuration("ai.command must name the program to run".to_owned())
            })?;
            Ok(Box::new(CommandProvider::new(
                program.clone(),
                args.to_vec(),
                non_empty(config.model.as_deref()).map(str::to_owned),
                config.timeout_seconds,
            )))
        }
        AiProviderKind::OpenaiCompatible => {
            let raw = non_empty(config.endpoint.as_deref()).ok_or_else(|| {
                AiError::Configuration(
                    "ai.endpoint must be set for the openai-compatible provider".to_owned(),
                )
            })?;
            let model = non_empty(config.model.as_deref()).ok_or_else(|| {
                AiError::Configuration(
                    "ai.model must be set for the openai-compatible provider".to_owned(),
                )
            })?;
            let endpoint = Endpoint::parse(raw)?;
            check_remote(&endpoint, "openai-compatible", allow_remote)?;
            let key = match non_empty(config.api_key_env.as_deref()) {
                Some(name) => Some(api_key(name, env)?),
                None => None,
            };
            Ok(Box::new(OpenAiCompatibleProvider::new(
                endpoint,
                model.to_owned(),
                key,
                config.timeout_seconds,
            )))
        }
        AiProviderKind::Anthropic => {
            let model = non_empty(config.model.as_deref()).ok_or_else(|| {
                AiError::Configuration("ai.model must be set for the anthropic provider".to_owned())
            })?;
            let endpoint = Endpoint::parse(
                non_empty(config.endpoint.as_deref()).unwrap_or(ANTHROPIC_ENDPOINT),
            )?;
            check_remote(&endpoint, "anthropic", allow_remote)?;
            let key_name = non_empty(config.api_key_env.as_deref())
                .map(str::to_owned)
                .or_else(|| {
                    (endpoint.base == ANTHROPIC_ENDPOINT).then(|| "ANTHROPIC_API_KEY".to_owned())
                });
            let key = match key_name {
                Some(name) => Some(api_key(&name, env)?),
                None => None,
            };
            Ok(Box::new(AnthropicProvider::new(
                endpoint,
                model.to_owned(),
                key,
                config.timeout_seconds,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(name: &str) -> Option<String> {
        (name == "TEST_KEY" || name == "ANTHROPIC_API_KEY").then(|| "k".to_owned())
    }

    fn config(provider: AiProviderKind) -> AiConfig {
        AiConfig {
            provider,
            ..AiConfig::default()
        }
    }

    #[test]
    fn builds_providers_and_enforces_permissions() {
        assert!(matches!(
            build_provider_with_env(&config(AiProviderKind::None), true, &env),
            Err(AiError::Disabled)
        ));

        let mut command = config(AiProviderKind::Command);
        assert!(build_provider_with_env(&command, false, &env).is_err());
        command.command = vec!["ollama".into(), "run".into(), "llama3.2".into()];
        let provider = build_provider_with_env(&command, false, &env).unwrap();
        assert_eq!(provider.id(), "command");
        assert!(!provider.remote());
        assert_eq!(provider.model(), "ollama");

        let mut local = config(AiProviderKind::OpenaiCompatible);
        local.endpoint = Some("http://127.0.0.1:11434/v1".into());
        assert!(build_provider_with_env(&local, false, &env).is_err());
        local.model = Some("llama3.2".into());
        let provider = build_provider_with_env(&local, false, &env).unwrap();
        assert!(!provider.remote());
        assert_eq!(
            provider.destination(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );

        let mut remote = local.clone();
        remote.endpoint = Some("https://models.example.com/v1".into());
        assert!(matches!(
            build_provider_with_env(&remote, false, &env),
            Err(AiError::RemoteNotAllowed { ref host, .. }) if host == "models.example.com"
        ));
        remote.api_key_env = Some("MISSING".into());
        assert!(matches!(
            build_provider_with_env(&remote, true, &env),
            Err(AiError::MissingApiKey(ref name)) if name == "MISSING"
        ));
        remote.api_key_env = Some("TEST_KEY".into());
        assert!(
            build_provider_with_env(&remote, true, &env)
                .unwrap()
                .remote()
        );

        let mut anthropic = config(AiProviderKind::Anthropic);
        assert!(matches!(
            build_provider_with_env(&anthropic, true, &env),
            Err(AiError::Configuration(ref message)) if message.contains("ai.model")
        ));
        anthropic.model = Some("test-model".into());
        assert!(matches!(
            build_provider_with_env(&anthropic, false, &env),
            Err(AiError::RemoteNotAllowed { .. })
        ));
        let provider = build_provider_with_env(&anthropic, true, &env).unwrap();
        assert_eq!(provider.model(), "test-model");
        assert!(matches!(
            build_provider_with_env(&anthropic, true, &|_| None),
            Err(AiError::MissingApiKey(ref name)) if name == "ANTHROPIC_API_KEY"
        ));
    }

    #[test]
    fn chooses_output_limits_per_provider() {
        assert_eq!(
            max_output_tokens(&config(AiProviderKind::Anthropic)),
            DEFAULT_ANTHROPIC_OUTPUT_TOKENS
        );
        let mut openai = config(AiProviderKind::OpenaiCompatible);
        assert_eq!(max_output_tokens(&openai), DEFAULT_OUTPUT_TOKENS);
        openai.max_output_tokens = Some(900);
        assert_eq!(max_output_tokens(&openai), 900);
    }
}
