//! Configuration loading and provider selection.

mod defaults;
mod error;
mod file;
mod loading;
mod overrides;
mod provider;
mod raw;
mod web;

pub use defaults::DEFAULT_NOEMA_CONFIG_YAML;
pub use error::ConfigError;
pub use loading::Config;
pub use overrides::{ConfigOpenAiOverrides, ConfigOverrides};
pub use provider::{
    DEFAULT_FOUNDATION_LOCAL_PROFILE, DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
    DEFAULT_LOCAL_MODELS_PROFILE, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
    DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
    DEFAULT_PROVIDER, FoundationLocalProviderConfig, LocalModelsProviderConfig, OPENAI_API_KEY_ENV,
    ProviderConfig, ProviderKind,
};
pub use raw::{DaemonResolvedConfig, ResolvedConfig};
pub use web::{DEFAULT_WEB_HOST, DEFAULT_WEB_PORT, WebConfig};

#[cfg(test)]
use loading::{CONFIG_ENV_KEYS, load_raw_config_from_sources, normalize_config_env_key};

#[cfg(test)]
mod tests;
