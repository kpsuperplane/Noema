use super::{
    error::ConfigError,
    provider::{
        DEFAULT_FOUNDATION_LOCAL_PROFILE, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
        DEFAULT_PROVIDER, FoundationLocalProviderConfig, OPENAI_API_KEY_ENV, ProviderConfig,
        ProviderKind, codex_base_url_default,
    },
    web::WebConfig,
};
use crate::provider::ReasoningEffort;
use crate::provider::adapters::{
    codex_oauth::DEFAULT_CODEX_BASE_URL,
    codex_responses::{CodexProviderConfig, DEFAULT_CODEX_TIMEOUT_SECONDS},
    openai::{DEFAULT_OPENAI_TIMEOUT_SECONDS, OpenAiProviderConfig},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

/// Fully resolved configuration used by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// Selected provider configuration.
    pub provider: ProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

/// Fully resolved configuration needed by the local daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonResolvedConfig {
    /// Provider configuration used for daemon conversations.
    pub provider: ProviderConfig,
    /// Local web UI configuration.
    pub web: WebConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawConfig {
    provider: String,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    tool_classification_model: Option<String>,
    openai: RawOpenAiConfig,
    codex: RawCodexConfig,
    foundation_local: RawFoundationLocalConfig,
    web: WebConfig,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_PROVIDER.to_string(),
            model: None,
            reasoning_effort: None,
            tool_classification_model: None,
            openai: RawOpenAiConfig::default(),
            codex: RawCodexConfig::default(),
            foundation_local: RawFoundationLocalConfig::default(),
            web: WebConfig::default(),
        }
    }
}

impl RawConfig {
    pub(super) fn resolve(self) -> Result<ResolvedConfig, ConfigError> {
        let provider = ProviderKind::from_str(self.provider.trim()).map_err(|provider| {
            ConfigError::UnsupportedProvider {
                provider: provider.clone(),
            }
        })?;

        let provider = match provider {
            ProviderKind::OpenAi => ProviderConfig::OpenAi(self.resolve_openai_config()?),
            ProviderKind::Codex => ProviderConfig::Codex(self.resolve_codex_config()?),
            ProviderKind::FoundationLocal => {
                validate_reasoning_config(None, self.reasoning_effort, "foundation_local")?;
                ProviderConfig::FoundationLocal(self.resolve_foundation_local_config())
            }
        };

        Ok(ResolvedConfig {
            provider,
            web: self.web,
        })
    }

    pub(super) fn resolve_daemon_config(self) -> Result<DaemonResolvedConfig, ConfigError> {
        let resolved = self.resolve()?;

        Ok(DaemonResolvedConfig {
            provider: resolved.provider,
            web: resolved.web,
        })
    }

    pub(super) fn resolve_openai_config(&self) -> Result<OpenAiProviderConfig, ConfigError> {
        let explicit_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
        validate_reasoning_config(explicit_model.as_deref(), self.reasoning_effort, "openai")?;
        let model = explicit_model
            .clone()
            .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());
        let base_url = non_empty_option(Some(self.openai.base_url.as_str()))
            .unwrap_or(DEFAULT_OPENAI_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        let api_key = non_empty_option(self.openai.api_key.as_deref()).ok_or_else(|| {
            ConfigError::MissingCredential {
                provider: "openai".to_string(),
                credential: OPENAI_API_KEY_ENV.to_string(),
            }
        })?;
        let timeout_seconds =
            require_positive(self.openai.timeout_seconds, "NOEMA_OPENAI__TIMEOUT_SECONDS")?;

        Ok(OpenAiProviderConfig {
            api_key: api_key.to_string(),
            base_url,
            organization_id: non_empty_option(self.openai.organization_id.as_deref())
                .map(ToString::to_string),
            project_id: non_empty_option(self.openai.project_id.as_deref())
                .map(ToString::to_string),
            default_model: model,
            tool_classification_model: non_empty_option(self.tool_classification_model.as_deref())
                .or_else(|| non_empty_option(self.openai.tool_classification_model.as_deref()))
                .map(ToString::to_string),
            reasoning_effort: self.reasoning_effort,
            timeout_seconds,
            system_errors: None,
        })
    }

    pub(super) fn resolve_codex_config(&self) -> Result<CodexProviderConfig, ConfigError> {
        let timeout_seconds =
            require_positive(self.codex.timeout_seconds, "NOEMA_CODEX__TIMEOUT_SECONDS")?;
        let base_url = non_empty_option(Some(self.codex.base_url.as_str()))
            .unwrap_or(DEFAULT_CODEX_BASE_URL)
            .trim_end_matches('/')
            .to_string();
        let top_level_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
        let codex_model = non_empty_option(self.codex.model.as_deref()).map(ToString::to_string);
        let explicit_model = top_level_model.or(codex_model);
        let reasoning_effort = self.reasoning_effort.or(self.codex.reasoning_effort);
        validate_reasoning_config(explicit_model.as_deref(), reasoning_effort, "codex")?;

        Ok(CodexProviderConfig {
            base_url,
            default_model: explicit_model,
            tool_classification_model: non_empty_option(self.tool_classification_model.as_deref())
                .or_else(|| non_empty_option(self.codex.tool_classification_model.as_deref()))
                .map(ToString::to_string),
            reasoning_effort,
            timeout_seconds,
            client_version: None,
            account_home: None,
            oauth: Default::default(),
            system_errors: None,
        })
    }

    fn resolve_foundation_local_config(&self) -> FoundationLocalProviderConfig {
        let default_profile =
            non_empty_option(Some(self.foundation_local.default_profile.as_str()))
                .unwrap_or(DEFAULT_FOUNDATION_LOCAL_PROFILE)
                .to_string();
        FoundationLocalProviderConfig {
            default_profile,
            bridge_path: self.foundation_local.bridge_path.clone(),
            system_errors: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawOpenAiConfig {
    api_key: Option<String>,
    base_url: String,
    organization_id: Option<String>,
    project_id: Option<String>,
    tool_classification_model: Option<String>,
    timeout_seconds: u64,
}

impl Default for RawOpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            organization_id: None,
            project_id: None,
            tool_classification_model: None,
            timeout_seconds: DEFAULT_OPENAI_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawCodexConfig {
    base_url: String,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    tool_classification_model: Option<String>,
    timeout_seconds: u64,
}

impl Default for RawCodexConfig {
    fn default() -> Self {
        Self {
            base_url: codex_base_url_default(),
            model: None,
            reasoning_effort: None,
            tool_classification_model: None,
            timeout_seconds: DEFAULT_CODEX_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawFoundationLocalConfig {
    default_profile: String,
    bridge_path: Option<PathBuf>,
}

impl Default for RawFoundationLocalConfig {
    fn default() -> Self {
        Self {
            default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
            bridge_path: None,
        }
    }
}

fn non_empty_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn require_positive(value: u64, name: &str) -> Result<u64, ConfigError> {
    if value == 0 {
        Err(ConfigError::InvalidInteger {
            name: name.to_string(),
            value: value.to_string(),
        })
    } else {
        Ok(value)
    }
}

fn validate_reasoning_config(
    explicit_model: Option<&str>,
    reasoning_effort: Option<ReasoningEffort>,
    provider_kind: &str,
) -> Result<(), ConfigError> {
    if explicit_model.is_none() && reasoning_effort.is_some() {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} reasoning_effort requires an explicit model"),
        });
    }
    if explicit_model.is_some()
        && matches!(provider_kind, "codex" | "openai")
        && reasoning_effort.is_none()
    {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} explicit model requires reasoning_effort"),
        });
    }
    if reasoning_effort.is_some() && !matches!(provider_kind, "codex" | "openai") {
        return Err(ConfigError::InvalidConfig {
            message: format!("{provider_kind} does not support reasoning_effort"),
        });
    }
    Ok(())
}
