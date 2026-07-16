//! Narrow factory for concrete providers owned by the hosted-adapter feature.

use noema_home::SystemErrorLogger;

use super::{
    account_service::ProviderCredentialAccessHandle, codex::CodexResponsesProvider,
    foundation::FoundationLocalProvider, openai::OpenAiProvider,
};
use crate::{ProviderConfig, ProviderError, ProviderHandle, ProviderKind, erase_model_provider};

const DEFAULT_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";

/// Construct one concrete hosted provider behind the provider operation handle.
///
/// The still-core-owned local-model implementation is deliberately rejected so
/// core can retain that transitional branch until the local-model phase moves
/// it into this crate.
///
/// # Errors
///
/// Returns a provider configuration or adapter-construction error, or
/// [`ProviderError::InvalidRequest`] for the transitional local-model variant.
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

    use super::hosted_provider_from_config;
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
    fn local_models_remains_an_explicit_core_owned_branch() {
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

        let error = hosted_provider_from_config(
            ProviderConfig::LocalModels(config),
            credential_access(),
            SystemErrorLogger::new(temp.path().join("errors.log")),
        )
        .expect_err("local models must remain core-owned during the transition");

        assert!(matches!(
            error,
            ProviderError::InvalidRequest { message }
                if message == "local_models is not a hosted provider adapter"
        ));
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
