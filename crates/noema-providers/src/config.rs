//! Resolved provider configuration.

use std::{fmt, path::PathBuf, str::FromStr};

use noema_home::SystemErrorLogger;
use serde::{Deserialize, Serialize};

use crate::{LocalModelBackend, ReasoningEffort};

/// Default provider used when config does not specify one.
pub const DEFAULT_PROVIDER: &str = "openai";
/// Product default reasoning effort shared by hosted model providers.
pub const DEFAULT_HOSTED_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Medium;
/// Default `OpenAI` model used when no model override is supplied.
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-luna";
/// Default `OpenAI` API base URL.
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
/// Default request timeout for `OpenAI` calls.
pub const DEFAULT_OPENAI_TIMEOUT_SECONDS: u64 = 120;
/// Environment variable used for `OpenAI` API credentials.
pub const OPENAI_API_KEY_ENV: &str = "NOEMA_OPENAI__API_KEY";

/// Default OpenRouter API base URL.
pub const DEFAULT_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
/// Default OpenRouter model/router.
pub const DEFAULT_OPENROUTER_MODEL: &str = "openrouter/auto";
/// Default OpenRouter request timeout.
pub const DEFAULT_OPENROUTER_TIMEOUT_SECONDS: u64 = 120;

/// Codex provider id used in user-facing auth state.
#[cfg(feature = "adapters")]
pub(crate) const CODEX_PROVIDER: &str = "codex";
/// Default Codex Responses API base URL.
pub const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
/// Default Codex Responses model used when no override is supplied.
pub(crate) const DEFAULT_CODEX_MODEL: &str = DEFAULT_OPENAI_MODEL;
/// Default request timeout for Codex Responses calls.
pub const DEFAULT_CODEX_TIMEOUT_SECONDS: u64 = 300;
/// Default Codex OAuth issuer.
const DEFAULT_CODEX_OAUTH_ISSUER: &str = "https://auth.openai.com";
/// Default Codex OAuth client id used by Codex/Hermes device auth.
const DEFAULT_CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Default Codex token endpoint.
const DEFAULT_CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
/// Refresh access tokens when JWT expiry is within this many seconds.
#[cfg(feature = "adapters")]
pub(crate) const CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS: u64 = 120;
/// Default request timeout for Codex OAuth calls.
const DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS: u64 = 20;

/// Default profile id for Apple Foundation Models.
pub const DEFAULT_FOUNDATION_LOCAL_PROFILE: &str = "default";
/// Default profile id for first-party local GGUF models.
pub const DEFAULT_LOCAL_MODELS_PROFILE: &str = "default";
/// Default context window exposed for local GGUF models.
pub const DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS: u32 = 8_192;
/// Default local generation request timeout.
pub const DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS: u64 = 600;
/// Default time allowed for `llama-server` to load a model.
pub const DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS: u64 = 180;

/// Supported provider identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Codex Responses provider.
    Codex,
    /// `OpenAI` Responses API provider.
    OpenAi,
    /// OpenRouter Responses provider.
    OpenRouter,
    /// Local Apple Foundation Models provider.
    FoundationLocal,
    /// First-party local GGUF model provider.
    LocalModels,
}

impl ProviderKind {
    /// Return the stable config string for this provider.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::FoundationLocal => "foundation_local",
            Self::LocalModels => "local_models",
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "openai" => Ok(Self::OpenAi),
            "openrouter" => Ok(Self::OpenRouter),
            "foundation_local" => Ok(Self::FoundationLocal),
            "local_models" => Ok(Self::LocalModels),
            other => Err(other.to_string()),
        }
    }
}

/// Configuration for the `OpenAI` provider.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenAiProviderConfig {
    /// API key sent as a bearer token.
    pub api_key: String,
    /// Base URL for an OpenAI-compatible Responses API.
    pub base_url: String,
    /// Optional `OpenAI` organization id.
    pub organization_id: Option<String>,
    /// Optional `OpenAI` project id.
    pub project_id: Option<String>,
    /// Default model used when a request does not override it.
    pub default_model: String,
    /// Optional model override for metadata-only tool classification.
    pub tool_classification_model: Option<String>,
    /// Optional explicit reasoning effort used only with an explicit model.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Developer diagnostic system error logger.
    pub system_errors: Option<SystemErrorLogger>,
}

impl fmt::Debug for OpenAiProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiProviderConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &"[REDACTED URL]")
            .field("organization_id", &self.organization_id)
            .field("project_id", &self.project_id)
            .field("default_model", &self.default_model)
            .field("tool_classification_model", &self.tool_classification_model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}

/// Configuration for the OpenRouter Responses provider.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenRouterProviderConfig {
    /// OpenRouter API base URL.
    pub base_url: String,
    /// Default model or router.
    pub default_model: String,
    /// Optional model override for metadata-only tool classification.
    pub tool_classification_model: Option<String>,
    /// Optional reasoning effort used with the default model.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Developer diagnostic system error logger.
    pub system_errors: Option<SystemErrorLogger>,
}

impl Default for OpenRouterProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            default_model: DEFAULT_OPENROUTER_MODEL.to_string(),
            tool_classification_model: None,
            reasoning_effort: None,
            timeout_seconds: DEFAULT_OPENROUTER_TIMEOUT_SECONDS,
            system_errors: None,
        }
    }
}

impl fmt::Debug for OpenRouterProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenRouterProviderConfig")
            .field("base_url", &"[REDACTED URL]")
            .field("default_model", &self.default_model)
            .field("tool_classification_model", &self.tool_classification_model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}

/// Noema-owned Codex OAuth token file.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexOAuthTokens {
    /// Access token used as Responses API bearer token.
    pub access_token: String,
    /// Refresh token used to rotate access tokens.
    pub refresh_token: String,
    /// Unix seconds when the token file was last written.
    pub last_refresh: u64,
}

impl CodexOAuthTokens {
    /// Return whether both required token fields are non-empty.
    #[must_use]
    #[cfg(feature = "adapters")]
    pub(crate) fn has_required_fields(&self) -> bool {
        !self.access_token.trim().is_empty() && !self.refresh_token.trim().is_empty()
    }
}

impl fmt::Debug for CodexOAuthTokens {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexOAuthTokens")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("last_refresh", &self.last_refresh)
            .finish()
    }
}

/// Codex OAuth HTTP endpoints.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexOAuthConfig {
    /// OAuth issuer base URL.
    pub issuer: String,
    /// OAuth client id.
    pub client_id: String,
    /// OAuth token URL.
    pub token_url: String,
    /// HTTP timeout in seconds.
    pub timeout_seconds: u64,
}

impl Default for CodexOAuthConfig {
    fn default() -> Self {
        Self {
            issuer: DEFAULT_CODEX_OAUTH_ISSUER.to_string(),
            client_id: DEFAULT_CODEX_OAUTH_CLIENT_ID.to_string(),
            token_url: DEFAULT_CODEX_OAUTH_TOKEN_URL.to_string(),
            timeout_seconds: DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS,
        }
    }
}

impl fmt::Debug for CodexOAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexOAuthConfig")
            .field("issuer", &"[REDACTED URL]")
            .field("client_id", &"[REDACTED]")
            .field("token_url", &"[REDACTED URL]")
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

/// Configuration for the Codex direct Responses provider.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    /// Base URL for the Codex Responses API.
    pub base_url: String,
    /// Optional default model.
    pub default_model: Option<String>,
    /// Optional model override for metadata-only tool classification.
    pub tool_classification_model: Option<String>,
    /// Optional explicit reasoning effort used only with an explicit model.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Codex client version advertised to the subscription backend.
    pub client_version: Option<String>,
    /// OAuth endpoint configuration used for token refresh and login.
    pub oauth: CodexOAuthConfig,
    /// Developer diagnostic system error logger.
    pub system_errors: Option<SystemErrorLogger>,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            default_model: Some(DEFAULT_CODEX_MODEL.to_string()),
            tool_classification_model: None,
            reasoning_effort: Some(DEFAULT_HOSTED_REASONING_EFFORT),
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
            client_version: None,
            oauth: CodexOAuthConfig::default(),
            system_errors: None,
        }
    }
}

impl fmt::Debug for CodexProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexProviderConfig")
            .field("base_url", &"[REDACTED URL]")
            .field("default_model", &self.default_model)
            .field("tool_classification_model", &self.tool_classification_model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("client_version", &self.client_version)
            .field("oauth", &self.oauth)
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}

/// Apple Foundation Models provider configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct FoundationLocalProviderConfig {
    /// Default user-facing profile id when an agent has no preference.
    pub default_profile: String,
    /// Optional path to a manually built Swift bridge executable.
    pub bridge_path: Option<PathBuf>,
    /// Optional developer diagnostic logger for malformed provider output.
    pub system_errors: Option<SystemErrorLogger>,
}

impl fmt::Debug for FoundationLocalProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FoundationLocalProviderConfig")
            .field("default_profile", &self.default_profile)
            .field(
                "bridge_path",
                &self.bridge_path.as_ref().map(|_| "[REDACTED PATH]"),
            )
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}

/// First-party local GGUF provider configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalModelsProviderConfig {
    /// Installed model id used when a request has no model override.
    pub default_model: String,
    /// Absolute path to the installed, checksum-verified GGUF blob.
    pub model_path: Option<PathBuf>,
    /// Preferred backend for this installed build; CPU is tried as a fallback.
    pub preferred_backend: Option<LocalModelBackend>,
    /// Packaged llama.cpp resource root supplied by the desktop shell.
    pub runtime_root: Option<PathBuf>,
    /// Context window exposed to Noema prompt planning.
    pub context_window_tokens: u32,
    /// Generation request timeout.
    pub timeout_seconds: u64,
    /// Time allowed for `llama-server` to load the model.
    pub startup_timeout_seconds: u64,
    /// Optional developer diagnostic logger for malformed provider output.
    pub system_errors: Option<SystemErrorLogger>,
}

impl fmt::Debug for LocalModelsProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelsProviderConfig")
            .field("default_model", &self.default_model)
            .field(
                "model_path",
                &self.model_path.as_ref().map(|_| "[REDACTED PATH]"),
            )
            .field("preferred_backend", &self.preferred_backend)
            .field(
                "runtime_root",
                &self.runtime_root.as_ref().map(|_| "[REDACTED PATH]"),
            )
            .field("context_window_tokens", &self.context_window_tokens)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("startup_timeout_seconds", &self.startup_timeout_seconds)
            .field("system_errors_configured", &self.system_errors.is_some())
            .finish()
    }
}
/// Concrete configuration for the selected provider.
#[derive(Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    /// Codex provider configuration.
    Codex(CodexProviderConfig),
    /// `OpenAI` provider configuration.
    OpenAi(OpenAiProviderConfig),
    /// OpenRouter Responses API provider.
    OpenRouter(OpenRouterProviderConfig),
    /// Apple Foundation Models local provider configuration.
    FoundationLocal(FoundationLocalProviderConfig),
    /// First-party local GGUF provider configuration.
    LocalModels(LocalModelsProviderConfig),
}

impl ProviderConfig {
    /// Return the provider kind for this configuration.
    #[must_use]
    pub const fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
            Self::OpenRouter(_) => ProviderKind::OpenRouter,
            Self::FoundationLocal(_) => ProviderKind::FoundationLocal,
            Self::LocalModels(_) => ProviderKind::LocalModels,
        }
    }

    /// Return the default model configured for this provider.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(config) => config.default_model.as_deref(),
            Self::OpenAi(config) => Some(config.default_model.as_str()),
            Self::OpenRouter(config) => Some(config.default_model.as_str()),
            Self::FoundationLocal(config) => Some(config.default_profile.as_str()),
            Self::LocalModels(config) => Some(config.default_model.as_str()),
        }
    }

    /// Return the default reasoning effort configured for this provider.
    #[must_use]
    pub const fn reasoning_effort(&self) -> Option<ReasoningEffort> {
        match self {
            Self::Codex(config) => config.reasoning_effort,
            Self::OpenAi(config) => config.reasoning_effort,
            Self::OpenRouter(config) => config.reasoning_effort,
            Self::FoundationLocal(_) | Self::LocalModels(_) => None,
        }
    }

    /// Resolve process-manager settings for provider-owned local inference.
    ///
    /// The selected local-model configuration supplies its explicit tuning.
    /// Other configured defaults still construct the dormant local-model
    /// control plane with product defaults so an installed local model can be
    /// activated later without restarting the host.
    #[cfg(feature = "local-models")]
    #[must_use]
    pub fn local_model_manager_config(
        &self,
        packaged_runtime_root: Option<PathBuf>,
        system_errors: SystemErrorLogger,
    ) -> crate::LocalModelManagerConfig {
        match self {
            Self::LocalModels(config) => crate::LocalModelManagerConfig {
                runtime_root: config.runtime_root.clone().or(packaged_runtime_root),
                context_window_tokens: config.context_window_tokens,
                timeout_seconds: config.timeout_seconds,
                startup_timeout_seconds: config.startup_timeout_seconds,
                system_errors: Some(system_errors),
            },
            _ => crate::LocalModelManagerConfig {
                runtime_root: packaged_runtime_root,
                context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
                timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
                startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
                system_errors: Some(system_errors),
            },
        }
    }
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codex(config) => formatter.debug_tuple("Codex").field(config).finish(),
            Self::OpenAi(config) => formatter.debug_tuple("OpenAi").field(config).finish(),
            Self::OpenRouter(config) => formatter.debug_tuple("OpenRouter").field(config).finish(),
            Self::FoundationLocal(config) => formatter
                .debug_tuple("FoundationLocal")
                .field(config)
                .finish(),
            Self::LocalModels(config) => {
                formatter.debug_tuple("LocalModels").field(config).finish()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_reachable_debug_output_is_redacted() {
        let openai = OpenAiProviderConfig {
            api_key: "openai-secret".to_string(),
            base_url: "https://url-user:url-secret@example.test/v1?token=query-secret".to_string(),
            organization_id: None,
            project_id: None,
            default_model: DEFAULT_OPENAI_MODEL.to_string(),
            tool_classification_model: None,
            reasoning_effort: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
            system_errors: None,
        };
        let tokens = CodexOAuthTokens {
            access_token: "access-secret".to_string(),
            refresh_token: "refresh-secret".to_string(),
            last_refresh: 1,
        };
        let oauth = CodexOAuthConfig {
            issuer: "https://issuer-user:issuer-secret@example.test?token=issuer-query".to_string(),
            client_id: "client-secret".to_string(),
            token_url: "https://token-user:token-secret@example.test/oauth?token=token-query"
                .to_string(),
            timeout_seconds: DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS,
        };
        let local = LocalModelsProviderConfig {
            default_model: "local".to_string(),
            model_path: Some(PathBuf::from("/private/model-path")),
            preferred_backend: None,
            runtime_root: Some(PathBuf::from("/private/runtime-root")),
            context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: None,
        };
        let codex = CodexProviderConfig {
            base_url: "https://codex-user:codex-secret@example.test?token=codex-query".to_string(),
            oauth: oauth.clone(),
            ..CodexProviderConfig::default()
        };
        let provider_config = ProviderConfig::OpenAi(openai.clone());

        let debug =
            format!("{openai:?} {tokens:?} {oauth:?} {local:?} {codex:?} {provider_config:?}");
        for secret in [
            "openai-secret",
            "access-secret",
            "refresh-secret",
            "client-secret",
            "url-secret",
            "query-secret",
            "issuer-secret",
            "issuer-query",
            "token-secret",
            "token-query",
            "model-path",
            "runtime-root",
            "codex-secret",
            "codex-query",
            "codex-account",
        ] {
            assert!(!debug.contains(secret));
        }
        assert!(debug.contains("[REDACTED]"));
        assert!(debug.contains("[REDACTED URL]"));
        assert!(debug.contains("[REDACTED PATH]"));
    }
}
