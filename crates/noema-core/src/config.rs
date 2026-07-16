//! Configuration loading and provider selection.

mod defaults;
mod error;
mod file;
mod loading;
mod overrides;
mod raw;
mod web;

pub use defaults::DEFAULT_NOEMA_CONFIG_YAML;
pub use error::ConfigError;
pub use loading::Config;
pub use overrides::{ConfigOpenAiOverrides, ConfigOverrides};
pub use raw::{DaemonResolvedConfig, ResolvedConfig};
pub use web::{DEFAULT_WEB_HOST, DEFAULT_WEB_PORT, WebConfig};

#[cfg(test)]
use loading::{CONFIG_ENV_KEYS, load_raw_config_from_sources, normalize_config_env_key};
#[cfg(test)]
use noema_providers::{OPENAI_API_KEY_ENV, ProviderConfig, ProviderKind};

#[cfg(test)]
mod tests;
