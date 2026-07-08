//! Supermemory runtime configuration helpers.

/// Resolved runtime configuration for a Supermemory service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemoryRuntimeConfig {
    /// External service base URL.
    pub base_url: Option<String>,
    /// External service port.
    pub port: Option<u16>,
}

impl SupermemoryRuntimeConfig {
    /// Build runtime configuration from persisted memory service settings.
    #[must_use]
    pub fn from_settings(settings: &crate::MemoryServiceSettingsRecord) -> Self {
        Self {
            base_url: settings
                .base_url
                .as_deref()
                .map(|base_url| base_url.trim_end_matches('/').to_string()),
            port: settings.port,
        }
    }
}
