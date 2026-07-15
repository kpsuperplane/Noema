use crate::SystemErrorLogger;
use crate::local_models::LocalModelBackend;
use crate::provider::adapters::{
    codex_oauth::DEFAULT_CODEX_BASE_URL, codex_responses::CodexProviderConfig,
    openai::OpenAiProviderConfig,
};
use std::{path::PathBuf, str::FromStr};

/// Default provider used when config does not specify one.
pub const DEFAULT_PROVIDER: &str = "openai";
/// Default `OpenAI` model used when no model override is supplied.
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.5";
/// Default `OpenAI` API base URL.
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
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
/// Environment variable used for `OpenAI` API credentials.
pub const OPENAI_API_KEY_ENV: &str = "NOEMA_OPENAI__API_KEY";

/// Supported provider identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    /// Codex Responses provider.
    Codex,
    /// `OpenAI` Responses API provider.
    OpenAi,
    /// Local Apple Foundation Models provider.
    FoundationLocal,
    /// First-party local GGUF model provider.
    LocalModels,
}

impl ProviderKind {
    /// Return the stable config string for this provider.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenAi => "openai",
            Self::FoundationLocal => "foundation_local",
            Self::LocalModels => "local_models",
        }
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "openai" => Ok(Self::OpenAi),
            "foundation_local" => Ok(Self::FoundationLocal),
            "local_models" => Ok(Self::LocalModels),
            other => Err(other.to_string()),
        }
    }
}

/// Apple Foundation Models provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationLocalProviderConfig {
    /// Default user-facing profile id when an agent has no preference.
    pub default_profile: String,
    /// Optional path to a manually built Swift bridge executable.
    pub bridge_path: Option<PathBuf>,
    /// Optional developer diagnostic logger for malformed provider output.
    pub system_errors: Option<SystemErrorLogger>,
}

/// First-party local GGUF provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Concrete configuration for the selected provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    /// Codex provider configuration.
    Codex(CodexProviderConfig),
    /// `OpenAI` provider configuration.
    OpenAi(OpenAiProviderConfig),
    /// Apple Foundation Models local provider configuration.
    FoundationLocal(FoundationLocalProviderConfig),
    /// First-party local GGUF provider configuration.
    LocalModels(LocalModelsProviderConfig),
}

impl ProviderConfig {
    /// Return the provider kind for this configuration.
    #[must_use]
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
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
            Self::FoundationLocal(config) => Some(config.default_profile.as_str()),
            Self::LocalModels(config) => Some(config.default_model.as_str()),
        }
    }
}

pub(super) fn codex_base_url_default() -> String {
    DEFAULT_CODEX_BASE_URL.to_string()
}
