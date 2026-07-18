use super::{HostConfig, error::ConfigError, file::validate_file_config, raw::RawConfig};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Yaml},
};
use noema_home::{NOEMA_HOME_ENV, NoemaPaths};
use std::{env, path::PathBuf};

pub(super) const CONFIG_ENV_KEYS: &[&str] = &[
    "provider",
    "model",
    "reasoning_effort",
    "tool_classification_model",
    "openai.api_key",
    "openai.base_url",
    "openai.timeout_seconds",
    "openai.organization_id",
    "openai.project_id",
    "openai.tool_classification_model",
    "codex.model",
    "codex.reasoning_effort",
    "codex.tool_classification_model",
    "codex.base_url",
    "codex.timeout_seconds",
    "foundation_local.default_profile",
    "foundation_local.bridge_path",
    "local_models.default_model",
    "local_models.preferred_backend",
    "local_models.context_window_tokens",
    "local_models.timeout_seconds",
    "local_models.startup_timeout_seconds",
    "web.host",
    "web.port",
];

/// Configuration loader.
#[derive(Debug, Clone, Default)]
pub struct Config;

impl Config {
    /// Load the configured provider from defaults, config file, and environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when path resolution fails, the config file is
    /// missing or invalid, environment values are invalid, credentials are
    /// missing for the selected provider, or the provider is unsupported.
    pub fn load(path_override: Option<PathBuf>) -> Result<HostConfig, ConfigError> {
        let raw = load_raw_config(path_override)?;
        raw.resolve()
    }
}

fn load_raw_config(path_override: Option<PathBuf>) -> Result<RawConfig, ConfigError> {
    load_raw_config_from_sources(
        path_override,
        default_config_path()?,
        Figment::from(config_env_provider()),
    )
}

pub(super) fn load_raw_config_from_sources(
    path_override: Option<PathBuf>,
    default_config_path: Option<PathBuf>,
    env: Figment,
) -> Result<RawConfig, ConfigError> {
    let mut figment = Figment::from(Serialized::defaults(RawConfig::default()));

    if let Some(config_path) = resolved_config_path(path_override, default_config_path)? {
        validate_file_config(&config_path)?;
        figment = figment.merge(Yaml::file(config_path));
    }

    figment = figment.merge(env);

    Ok(figment.extract()?)
}

fn resolved_config_path(
    path_override: Option<PathBuf>,
    default_config_path: Option<PathBuf>,
) -> Result<Option<PathBuf>, ConfigError> {
    match path_override {
        Some(path) => {
            if path.exists() {
                Ok(Some(path))
            } else {
                Err(ConfigError::ConfigFileNotFound { path })
            }
        }
        None => Ok(default_config_path.filter(|path| path.exists())),
    }
}

fn default_config_path() -> Result<Option<PathBuf>, ConfigError> {
    let noema_home = env::var_os(NOEMA_HOME_ENV);
    let home = env::var_os("HOME");

    if noema_home.is_none() && home.is_none() {
        return Ok(None);
    }

    Ok(Some(
        NoemaPaths::from_env_values(noema_home, home)?.config_path(),
    ))
}

fn config_env_provider() -> Env {
    Env::prefixed("NOEMA_")
        .split("__")
        .ignore(&["home"])
        .filter_map(|key| normalize_config_env_key(key.as_str()).map(Into::into))
}

pub(super) fn normalize_config_env_key(key: &str) -> Option<String> {
    let key = key.to_ascii_lowercase();
    let normalized = key.as_str();

    CONFIG_ENV_KEYS
        .contains(&normalized)
        .then(|| normalized.to_string())
}
