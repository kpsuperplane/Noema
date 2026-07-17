//! Host configuration loading and provider selection.

mod defaults;
#[cfg(feature = "composition")]
mod error;
#[cfg(feature = "composition")]
mod file;
#[cfg(feature = "composition")]
mod loading;
#[cfg(feature = "composition")]
mod overrides;
#[cfg(feature = "composition")]
mod raw;
mod web;

pub use defaults::DEFAULT_NOEMA_CONFIG_YAML;
#[cfg(feature = "composition")]
pub use error::ConfigError;
#[cfg(feature = "composition")]
pub use loading::Config;
#[cfg(feature = "composition")]
pub use overrides::{ConfigOpenAiOverrides, ConfigOverrides};
pub use web::{DEFAULT_WEB_HOST, DEFAULT_WEB_PORT, WebConfig};

use std::path::PathBuf;

/// Fully resolved host configuration.
///
/// Provider construction details stay private to the composition layer. Shells
/// may inspect only the web listener configuration and attach their packaged
/// local-model runtime root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub(crate) provider: noema_providers::ProviderConfig,
    web: WebConfig,
    pub(crate) local_model_runtime_root: Option<PathBuf>,
}

impl HostConfig {
    #[cfg(feature = "composition")]
    pub(crate) fn new(provider: noema_providers::ProviderConfig, web: WebConfig) -> Self {
        Self {
            provider,
            web,
            local_model_runtime_root: None,
        }
    }

    /// Return the local web listener configuration.
    #[must_use]
    pub const fn web(&self) -> &WebConfig {
        &self.web
    }

    /// Attach a shell-packaged llama.cpp resource root.
    #[must_use]
    pub fn with_local_model_runtime_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.local_model_runtime_root = Some(root.into());
        self
    }
}

#[cfg(all(test, feature = "composition"))]
use loading::{CONFIG_ENV_KEYS, load_raw_config_from_sources, normalize_config_env_key};
#[cfg(all(test, feature = "composition"))]
use noema_providers::{OPENAI_API_KEY_ENV, ProviderConfig, ProviderKind};

#[cfg(all(test, feature = "composition"))]
mod tests;
