//! Narrow factory for concrete providers owned by the hosted-adapter feature.

use noema_home::SystemErrorLogger;

use super::{
    account_service::ProviderCredentialAccessHandle, codex::CodexResponsesProvider,
    foundation::FoundationLocalProvider, openai::OpenAiProvider, openrouter::OpenRouterProvider,
};
use crate::{
    DEFAULT_CODEX_MODEL, ProviderAccountOperationsHandle, ProviderAccountPersistenceHandle,
    ProviderConfig, ProviderError, ProviderHandle, ProviderKind, erase_model_provider,
};

const DEFAULT_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";

/// Provider-owned startup classification for one configured default.
pub struct ProviderBootstrap {
    /// Configured provider kind used for canonical default selection.
    pub default_provider_kind: String,
    /// Configured default model/profile, when the provider has one.
    pub default_model_profile: Option<String>,
    /// Concrete hosted provider, or `None` for provider-managed local inference.
    pub hosted_provider: Option<(String, ProviderHandle)>,
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
    accounts: Option<ProviderAccountPersistenceHandle>,
    account_operations: Option<ProviderAccountOperationsHandle>,
    system_errors: SystemErrorLogger,
) -> Result<ProviderBootstrap, ProviderError> {
    let default_provider_kind = config.kind().as_str().to_string();
    let default_model_profile = match &config {
        ProviderConfig::Codex(config) => Some(
            config
                .default_model
                .as_deref()
                .unwrap_or(DEFAULT_CODEX_MODEL)
                .to_string(),
        ),
        _ => config.model().map(str::to_string),
    };
    let hosted_provider = if matches!(&config, ProviderConfig::LocalModels(_)) {
        None
    } else {
        Some(hosted_provider_from_config(
            config,
            credentials,
            accounts,
            account_operations,
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
    accounts: Option<ProviderAccountPersistenceHandle>,
    account_operations: Option<ProviderAccountOperationsHandle>,
    system_errors: SystemErrorLogger,
) -> Result<(String, ProviderHandle), ProviderError> {
    match config {
        ProviderConfig::Codex(mut config) => {
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
        ProviderConfig::OpenRouter(mut config) => {
            config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenRouter.as_str().to_string(),
                erase_model_provider(OpenRouterProvider::new(
                    config,
                    credentials,
                    accounts,
                    account_operations,
                )?),
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

    use super::*;
    use crate::{
        CodexProviderConfig, DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
        DEFAULT_LOCAL_MODELS_PROFILE, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
        DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, LocalModelsProviderConfig, ProviderCredentialAccess,
        ProviderCredentialFuture,
    };

    #[derive(Debug)]
    struct MissingCredentials;

    impl ProviderCredentialAccess for MissingCredentials {
        fn api_key<'a>(&'a self, _kind: &'a str, _id: &'a str) -> ProviderCredentialFuture<'a> {
            missing("exa")
        }

        fn codex_access_token<'a>(&'a self, _id: &'a str) -> ProviderCredentialFuture<'a> {
            missing("codex")
        }

        fn refresh_codex_access_token<'a>(&'a self, _id: &'a str) -> ProviderCredentialFuture<'a> {
            missing("codex")
        }
    }

    #[test]
    fn bootstrap_exposes_implicit_codex_model_and_defers_local_construction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let credentials = Arc::new(MissingCredentials);
        let codex = provider_bootstrap_from_config(
            ProviderConfig::Codex(CodexProviderConfig {
                default_model: None,
                ..CodexProviderConfig::default()
            }),
            credentials.clone(),
            None,
            None,
            SystemErrorLogger::new(temp.path().join("codex-errors.log")),
        )
        .expect("Codex bootstrap");
        assert_eq!(
            codex.default_model_profile.as_deref(),
            Some(DEFAULT_CODEX_MODEL),
            "effective implicit Codex model"
        );

        let local = provider_bootstrap_from_config(
            ProviderConfig::LocalModels(LocalModelsProviderConfig {
                default_model: DEFAULT_LOCAL_MODELS_PROFILE.to_string(),
                model_path: None,
                preferred_backend: None,
                runtime_root: None,
                context_window_tokens: DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
                timeout_seconds: DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
                startup_timeout_seconds: DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
                system_errors: None,
            }),
            credentials,
            None,
            None,
            SystemErrorLogger::new(temp.path().join("local-errors.log")),
        )
        .expect("local bootstrap");
        assert_eq!(local.default_provider_kind, "local_models");
        assert_eq!(
            local.default_model_profile.as_deref(),
            Some(DEFAULT_LOCAL_MODELS_PROFILE)
        );
        assert!(local.hosted_provider.is_none(), "manager owns construction");
    }

    fn missing(provider: &'static str) -> ProviderCredentialFuture<'static> {
        Box::pin(async move {
            Err(ProviderError::MissingCredentials {
                provider: provider.to_string(),
                credential: "provider account".to_string(),
            })
        })
    }
}
