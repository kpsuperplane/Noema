//! Host configuration loading and provider selection.

mod browser;
mod defaults;
#[cfg(feature = "composition")]
mod error;
#[cfg(feature = "composition")]
mod file;
#[cfg(feature = "composition")]
mod loading;
mod mcp;
#[cfg(feature = "composition")]
mod raw;
#[cfg(feature = "composition")]
mod recovery;
mod web;

pub use browser::BrowserConfig;
pub use defaults::DEFAULT_NOEMA_CONFIG_YAML;
#[cfg(feature = "composition")]
pub use error::ConfigError;
#[cfg(feature = "composition")]
pub use loading::Config;
pub use mcp::McpConfig;
#[cfg(feature = "composition")]
pub use recovery::{RecoveryCodeError, RecoveryCodeStore};
pub use web::WebConfig;

use std::path::PathBuf;

/// Fully resolved host configuration.
///
/// Provider construction details stay private to the composition layer. Shells
/// may inspect only the web listener configuration and attach their packaged
/// local-model runtime root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub(crate) provider: noema_providers::ProviderConfig,
    pub(crate) browser: BrowserConfig,
    web: WebConfig,
    mcp: McpConfig,
    pub(crate) local_model_runtime_root: Option<PathBuf>,
}

impl HostConfig {
    #[cfg(feature = "composition")]
    pub(crate) fn new(
        provider: noema_providers::ProviderConfig,
        browser: BrowserConfig,
        web: WebConfig,
        mcp: McpConfig,
    ) -> Self {
        Self {
            provider,
            browser,
            web,
            mcp,
            local_model_runtime_root: None,
        }
    }

    /// Return the local web listener configuration.
    #[must_use]
    pub const fn web(&self) -> &WebConfig {
        &self.web
    }

    /// Return the MCP transport configuration.
    #[must_use]
    pub const fn mcp(&self) -> &McpConfig {
        &self.mcp
    }
}

#[cfg(all(test, feature = "composition"))]
use loading::{CONFIG_ENV_KEYS, load_raw_config_from_sources, normalize_config_env_key};
#[cfg(all(test, feature = "composition"))]
use noema_providers::{OPENAI_API_KEY_ENV, ProviderConfig, ProviderKind};

#[cfg(all(test, feature = "composition"))]
mod tests;
