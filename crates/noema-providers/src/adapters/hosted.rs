//! Narrow factory for concrete providers owned by the hosted-adapter feature.

use noema_home::SystemErrorLogger;

use super::{
    account_service::ProviderCredentialAccessHandle, codex::CodexResponsesProvider,
    foundation::FoundationLocalProvider, openai::OpenAiProvider,
};
use crate::{ProviderConfig, ProviderError, ProviderHandle, ProviderKind, erase_model_provider};

const DEFAULT_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";

/// Provider-owned startup classification for one configured default.
pub struct ProviderBootstrap {
    default_provider_kind: String,
    default_model_profile: Option<String>,
    hosted_provider: Option<(String, ProviderHandle)>,
}

impl ProviderBootstrap {
    /// Return the configured provider kind used for canonical default selection.
    #[must_use]
    pub fn default_provider_kind(&self) -> &str {
        &self.default_provider_kind
    }

    /// Return the configured default model/profile, when the provider has one.
    #[must_use]
    pub fn default_model_profile(&self) -> Option<&str> {
        self.default_model_profile.as_deref()
    }

    /// Take the concrete hosted provider, or `None` for provider-managed local inference.
    #[must_use]
    pub fn into_hosted_provider(self) -> Option<(String, ProviderHandle)> {
        self.hosted_provider
    }
}

impl std::fmt::Debug for ProviderBootstrap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderBootstrap")
            .field("default_provider_kind", &self.default_provider_kind)
            .field("default_model_profile", &self.default_model_profile)
            .field(
                "hosted_provider_configured",
                &self.hosted_provider.is_some(),
            )
            .finish()
    }
}

/// Classify the configured default and construct a hosted adapter when applicable.
///
/// Local GGUF inference is represented by the same configured provider kind and
/// model profile, but its concrete instance is reconstructed by
/// `LocalModelManager` from durable installation state.
///
/// # Errors
///
/// Returns a hosted adapter-construction error.
pub fn provider_bootstrap_from_config(
    config: ProviderConfig,
    credentials: ProviderCredentialAccessHandle,
    system_errors: SystemErrorLogger,
) -> Result<ProviderBootstrap, ProviderError> {
    let default_provider_kind = config.kind().as_str().to_string();
    let default_model_profile = config.model().map(str::to_string);
    let hosted_provider = if matches!(&config, ProviderConfig::LocalModels(_)) {
        None
    } else {
        Some(hosted_provider_from_config(
            config,
            credentials,
            system_errors,
        )?)
    };
    Ok(ProviderBootstrap {
        default_provider_kind,
        default_model_profile,
        hosted_provider,
    })
}

/// Construct one concrete hosted provider behind the provider operation handle.
///
/// Local-model construction is deliberately rejected because its manager
/// reconstructs exact installed instances from durable state.
///
/// # Errors
///
/// Returns a provider configuration or adapter-construction error, or
/// [`ProviderError::InvalidRequest`] for the local-model variant.
pub fn hosted_provider_from_config(
    config: ProviderConfig,
    credentials: ProviderCredentialAccessHandle,
    system_errors: SystemErrorLogger,
) -> Result<(String, ProviderHandle), ProviderError> {
    match config {
        ProviderConfig::Codex(mut config) => {
            config.account_home = None;
            config.system_errors = Some(system_errors);
            let provider = CodexResponsesProvider::new_with_credentials(
                config,
                DEFAULT_CODEX_PROVIDER_ACCOUNT_ID,
                credentials,
            )?;
            Ok((
                ProviderKind::Codex.as_str().to_string(),
                erase_model_provider(provider),
            ))
        }
        ProviderConfig::OpenAi(mut config) => {
            config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenAi.as_str().to_string(),
                erase_model_provider(OpenAiProvider::new(config)?),
            ))
        }
        ProviderConfig::FoundationLocal(mut config) => {
            config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::FoundationLocal.as_str().to_string(),
                erase_model_provider(FoundationLocalProvider::new(config)?),
            ))
        }
        ProviderConfig::LocalModels(_) => Err(ProviderError::InvalidRequest {
            message: "local_models is not a hosted provider adapter".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use noema_home::SystemErrorLogger;

    use super::{hosted_provider_from_config, provider_bootstrap_from_config};
    use crate::{
        CodexProviderConfig, DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
        DEFAULT_LOCAL_MODELS_PROFILE, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
        DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, LocalModelsProviderConfig, ProviderConfig,
        ProviderCredentialAccess, ProviderCredentialAccessHandle, ProviderCredentialFuture,
        ProviderError,
    };

    #[derive(Debug)]
    struct PathlessCredentials;

    impl ProviderCredentialAccess for PathlessCredentials {
        fn exa_api_key<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderCredentialFuture<'a> {
            missing_credentials("exa")
        }

        fn codex_access_token<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderCredentialFuture<'a> {
            missing_credentials("codex")
        }

        fn refresh_codex_access_token<'a>(
            &'a self,
            _provider_account_id: &'a str,
        ) -> ProviderCredentialFuture<'a> {
            missing_credentials("codex")
        }
    }

    #[test]
    fn codex_factory_builds_with_pathless_credentials() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config = CodexProviderConfig {
            account_home: Some(temp.path().join("legacy-account-home")),
            ..CodexProviderConfig::default()
        };

        let (kind, _provider) = hosted_provider_from_config(
            ProviderConfig::Codex(config),
            credential_access(),
            SystemErrorLogger::new(temp.path().join("errors.log")),
        )
        .expect("Codex hosted provider");

        assert_eq!(kind, "codex");
    }

    #[test]
    fn local_models_bootstrap_defers_construction_to_the_provider_manager() {
        let temp = tempfile::tempdir().expect("temp dir");
        let config = LocalModelsProviderConfig {
            default_model: DEFAULT_LOCAL_MODELS_PROFILE.to_string(),
            model_path: None,
            preferred_backend: None,
            runtime_root: None,
            context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: None,
        };

        let bootstrap = provider_bootstrap_from_config(
            ProviderConfig::LocalModels(config),
            credential_access(),
            SystemErrorLogger::new(temp.path().join("errors.log")),
        )
        .expect("local-model bootstrap");

        assert_eq!(bootstrap.default_provider_kind(), "local_models");
        assert_eq!(
            bootstrap.default_model_profile(),
            Some(DEFAULT_LOCAL_MODELS_PROFILE)
        );
        assert!(bootstrap.into_hosted_provider().is_none());
    }

    fn credential_access() -> ProviderCredentialAccessHandle {
        Arc::new(PathlessCredentials)
    }

    fn missing_credentials(provider: &'static str) -> ProviderCredentialFuture<'static> {
        Box::pin(async move {
            Err(ProviderError::MissingCredentials {
                provider: provider.to_string(),
                credential: "provider account".to_string(),
            })
        })
    }
}
