use crate::{
    FoundationLocalProviderConfig,
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, GenerateStreamEvent, ModelProvider,
        ProviderError,
    },
};

use super::foundation_bridge_process::{FoundationBridgeConfig, FoundationBridgeProcess};

/// Provider identifier for Apple Foundation Models.
pub const FOUNDATION_LOCAL_PROVIDER: &str = "foundation_local";

/// Apple Foundation Models provider facade.
#[derive(Debug, Clone)]
pub struct FoundationLocalProvider {
    config: FoundationLocalProviderConfig,
}

impl FoundationLocalProvider {
    /// Build a Foundation Local provider.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when configuration is invalid.
    pub fn new(config: FoundationLocalProviderConfig) -> Result<Self, ProviderError> {
        if config.default_profile.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "foundation local default profile cannot be empty".to_string(),
            });
        }
        Ok(Self { config })
    }

    async fn ensure_available(&self) -> Result<(), ProviderError> {
        let Some(path) = &self.config.bridge_path else {
            return Err(ProviderError::ProviderUnavailable {
                provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
                message: "Apple Foundation Models bridge path is not configured.".to_string(),
            });
        };

        FoundationBridgeProcess::start(FoundationBridgeConfig {
            bridge_path: path.clone(),
        })
        .await
        .map(|_| ())
        .map_err(|error| ProviderError::ProviderUnavailable {
            provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
            message: format!(
                "Apple Foundation Models bridge unavailable: {}",
                error.code()
            ),
        })
    }
}

impl ModelProvider for FoundationLocalProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let mut ignore_event = |_| {};
        self.generate_streaming(request, &mut ignore_event).await
    }

    async fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Result<GenerateResponse, ProviderError> {
        self.ensure_available().await?;
        let model = request
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| self.config.default_profile.clone());
        let GenerateInput::Text(text) = request.input;
        Ok(GenerateResponse {
            output: vec![crate::GenerateOutputItem::AssistantText { text }],
            provider: FOUNDATION_LOCAL_PROVIDER.to_string(),
            model,
            response_id: None,
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FoundationLocalProviderConfig, GenerateRequest, ModelProvider, ProviderError};

    #[tokio::test]
    async fn unavailable_stub_fails_cleanly() {
        let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: "default".to_string(),
            bridge_path: None,
        })
        .expect("provider");

        let error = provider
            .generate(GenerateRequest::text("hello"))
            .await
            .expect_err("stub should be unavailable");

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    }
}
