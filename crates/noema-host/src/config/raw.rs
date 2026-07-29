use super::{HostConfig, WebConfig, error::ConfigError};
use noema_providers::{
    CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_CODEX_TIMEOUT_SECONDS,
    DEFAULT_FOUNDATION_LOCAL_PROFILE, DEFAULT_HOSTED_REASONING_EFFORT,
    DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS, DEFAULT_LOCAL_MODELS_PROFILE,
    DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS, DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
    DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_TIMEOUT_SECONDS,
    DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENROUTER_MODEL, DEFAULT_OPENROUTER_TIMEOUT_SECONDS,
    DEFAULT_PROVIDER, FoundationLocalProviderConfig, LocalModelBackend, LocalModelsProviderConfig,
    OPENAI_API_KEY_ENV, OpenAiProviderConfig, OpenRouterProviderConfig, ProviderConfig,
    ProviderKind, ReasoningEffort,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawConfig {
    provider: String,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    tool_classification_model: Option<String>,
    openai: RawOpenAiConfig,
    openrouter: RawOpenRouterConfig,
    codex: RawCodexConfig,
    foundation_local: RawFoundationLocalConfig,
    local_models: RawLocalModelsConfig,
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
            openrouter: RawOpenRouterConfig::default(),
            codex: RawCodexConfig::default(),
            foundation_local: RawFoundationLocalConfig::default(),
            local_models: RawLocalModelsConfig::default(),
            web: WebConfig::default(),
        }
    }
}

impl RawConfig {
    pub(super) fn resolve(self) -> Result<HostConfig, ConfigError> {
        let provider = ProviderKind::from_str(self.provider.trim()).map_err(|provider| {
            ConfigError::UnsupportedProvider {
                provider: provider.clone(),
            }
        })?;

        let provider = match provider {
            ProviderKind::OpenAi => ProviderConfig::OpenAi(self.resolve_openai_config()?),
            ProviderKind::OpenRouter => {
                ProviderConfig::OpenRouter(self.resolve_openrouter_config()?)
            }
            ProviderKind::Codex => ProviderConfig::Codex(self.resolve_codex_config()?),
            ProviderKind::FoundationLocal => {
                validate_reasoning_config(None, self.reasoning_effort, "foundation_local")?;
                ProviderConfig::FoundationLocal(self.resolve_foundation_local_config())
            }
            ProviderKind::LocalModels => {
                validate_reasoning_config(None, self.reasoning_effort, "local_models")?;
                ProviderConfig::LocalModels(self.resolve_local_models_config()?)
            }
        };

        Ok(HostConfig::new(provider, self.web))
    }

    pub(super) fn resolve_openai_config(&self) -> Result<OpenAiProviderConfig, ConfigError> {
        let explicit_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
        validate_reasoning_config(explicit_model.as_deref(), self.reasoning_effort, "openai")?;
        let reasoning_effort = self.reasoning_effort.or(explicit_model
            .is_none()
            .then_some(DEFAULT_HOSTED_REASONING_EFFORT));
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
            reasoning_effort,
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
        let reasoning_effort = reasoning_effort.or(explicit_model
            .is_none()
            .then_some(DEFAULT_HOSTED_REASONING_EFFORT));

        Ok(CodexProviderConfig {
            base_url,
            default_model: Some(explicit_model.unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string())),
            tool_classification_model: non_empty_option(self.tool_classification_model.as_deref())
                .or_else(|| non_empty_option(self.codex.tool_classification_model.as_deref()))
                .map(ToString::to_string),
            reasoning_effort,
            timeout_seconds,
            client_version: None,
            oauth: Default::default(),
            system_errors: None,
        })
    }

    fn resolve_openrouter_config(&self) -> Result<OpenRouterProviderConfig, ConfigError> {
        let explicit_model = non_empty_option(self.model.as_deref()).map(ToString::to_string);
        validate_reasoning_config(
            explicit_model.as_deref(),
            self.reasoning_effort,
            "openrouter",
        )?;
        Ok(OpenRouterProviderConfig {
            base_url: non_empty_option(Some(self.openrouter.base_url.as_str()))
                .unwrap_or(DEFAULT_OPENROUTER_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
            default_model: explicit_model.unwrap_or_else(|| DEFAULT_OPENROUTER_MODEL.to_string()),
            tool_classification_model: non_empty_option(self.tool_classification_model.as_deref())
                .or_else(|| non_empty_option(self.openrouter.tool_classification_model.as_deref()))
                .map(ToString::to_string),
            reasoning_effort: self.reasoning_effort,
            timeout_seconds: require_positive(
                self.openrouter.timeout_seconds,
                "NOEMA_OPENROUTER__TIMEOUT_SECONDS",
            )?,
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

    fn resolve_local_models_config(&self) -> Result<LocalModelsProviderConfig, ConfigError> {
        let default_model = non_empty_option(self.model.as_deref())
            .or_else(|| non_empty_option(Some(self.local_models.default_model.as_str())))
            .unwrap_or(DEFAULT_LOCAL_MODELS_PROFILE)
            .to_string();
        let preferred_backend = self
            .local_models
            .preferred_backend
            .as_deref()
            .map(parse_local_model_backend)
            .transpose()?;

        Ok(LocalModelsProviderConfig {
            default_model,
            model_path: None,
            preferred_backend,
            runtime_root: None,
            context_window_tokens: require_positive(
                self.local_models.context_window_tokens,
                "NOEMA_LOCAL_MODELS__CONTEXT_WINDOW_TOKENS",
            )?,
            timeout_seconds: require_positive(
                self.local_models.timeout_seconds,
                "NOEMA_LOCAL_MODELS__TIMEOUT_SECONDS",
            )?,
            startup_timeout_seconds: require_positive(
                self.local_models.startup_timeout_seconds,
                "NOEMA_LOCAL_MODELS__STARTUP_TIMEOUT_SECONDS",
            )?,
            system_errors: None,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawOpenAiConfig {
    api_key: Option<String>,
    base_url: String,
    organization_id: Option<String>,
    project_id: Option<String>,
    tool_classification_model: Option<String>,
    timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawOpenRouterConfig {
    base_url: String,
    tool_classification_model: Option<String>,
    timeout_seconds: u64,
}

impl Default for RawOpenRouterConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            tool_classification_model: None,
            timeout_seconds: DEFAULT_OPENROUTER_TIMEOUT_SECONDS,
        }
    }
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
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct RawLocalModelsConfig {
    default_model: String,
    preferred_backend: Option<String>,
    context_window_tokens: u32,
    timeout_seconds: u64,
    startup_timeout_seconds: u64,
}

impl Default for RawLocalModelsConfig {
    fn default() -> Self {
        Self {
            default_model: DEFAULT_LOCAL_MODELS_PROFILE.to_string(),
            preferred_backend: None,
            context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
        }
    }
}

fn non_empty_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn require_positive<T>(value: T, name: &str) -> Result<T, ConfigError>
where
    T: Copy + Default + PartialEq + ToString,
{
    if value == T::default() {
        Err(ConfigError::InvalidInteger {
            name: name.to_string(),
            value: value.to_string(),
        })
    } else {
        Ok(value)
    }
}

fn parse_local_model_backend(value: &str) -> Result<LocalModelBackend, ConfigError> {
    value
        .trim()
        .to_ascii_lowercase()
        .parse()
        .map_err(|_| ConfigError::InvalidConfig {
            message: format!("unsupported local_models preferred_backend `{value}`"),
        })
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
