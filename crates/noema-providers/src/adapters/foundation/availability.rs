use crate::ProviderError;

use super::{
    FoundationLocalProvider,
    adapter::FOUNDATION_LOCAL_PROVIDER,
    bridge::{
        FoundationBridgeConfig, FoundationBridgeError, FoundationBridgeProcess,
        bridge_config_for_provider,
    },
};

impl FoundationLocalProvider {
    pub(super) async fn start_bridge(&self) -> Result<FoundationBridgeProcess, ProviderError> {
        let config = self.bridge_config();
        let diagnostic_path = config.bridge_path.clone();

        self.start_bridge_process(config).await.map_err(|error| {
            ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: format!(
                    "Apple Foundation Models bridge unavailable: {} ({}) at {}",
                    error.code(),
                    error,
                    diagnostic_path.display()
                ),
            }
        })
    }

    async fn start_bridge_process(
        &self,
        config: FoundationBridgeConfig,
    ) -> Result<FoundationBridgeProcess, FoundationBridgeError> {
        if !cfg!(target_os = "macos") {
            return Err(FoundationBridgeError::UnsupportedPlatform);
        }

        FoundationBridgeProcess::start(config).await
    }

    pub(super) fn bridge_config(&self) -> FoundationBridgeConfig {
        bridge_config_for_provider(&self.config)
    }

    /// Check whether the local Foundation Models bridge can be launched and is healthy.
    ///
    /// # Errors
    ///
    /// Returns [`FoundationBridgeError`] when the platform, bridge, or
    /// Foundation Models runtime is unavailable.
    pub async fn check_availability(&self) -> Result<(), FoundationBridgeError> {
        let bridge = self.start_bridge_process(self.bridge_config()).await?;
        drop(bridge);
        Ok(())
    }
}
