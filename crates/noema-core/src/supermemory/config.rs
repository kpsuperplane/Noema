//! Supermemory runtime configuration helpers.

/// Resolved runtime configuration for a Supermemory service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemoryRuntimeConfig {
    /// Service base URL.
    pub base_url: String,
    /// Managed service port.
    pub port: u16,
}

impl SupermemoryRuntimeConfig {
    /// Build runtime configuration from persisted memory service settings.
    #[must_use]
    pub fn from_settings(settings: &crate::MemoryServiceSettingsRecord) -> Self {
        let port = settings.port.unwrap_or(6767);
        Self {
            base_url: settings.base_url.trim_end_matches('/').to_string(),
            port,
        }
    }
}
