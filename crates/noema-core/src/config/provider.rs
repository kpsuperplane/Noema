use crate::SystemErrorLogger;
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
}

impl ProviderKind {
    /// Return the stable config string for this provider.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::OpenAi => "openai",
            Self::FoundationLocal => "foundation_local",
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

/// Concrete configuration for the selected provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    /// Codex provider configuration.
    Codex(CodexProviderConfig),
    /// `OpenAI` provider configuration.
    OpenAi(OpenAiProviderConfig),
    /// Apple Foundation Models local provider configuration.
    FoundationLocal(FoundationLocalProviderConfig),
}

impl ProviderConfig {
    /// Return the provider kind for this configuration.
    #[must_use]
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Codex(_) => ProviderKind::Codex,
            Self::OpenAi(_) => ProviderKind::OpenAi,
            Self::FoundationLocal(_) => ProviderKind::FoundationLocal,
        }
    }

    /// Return the default model configured for this provider.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(config) => config.default_model.as_deref(),
            Self::OpenAi(config) => Some(config.default_model.as_str()),
            Self::FoundationLocal(config) => Some(config.default_profile.as_str()),
        }
    }
}

pub(super) fn codex_base_url_default() -> String {
    DEFAULT_CODEX_BASE_URL.to_string()
}
