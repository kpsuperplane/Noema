//! Narrow factory for concrete providers owned by the hosted-adapter feature.

use noema_home::SystemErrorLogger;

use super::{
    account_service::ProviderCredentialAccessHandle, codex::CodexResponsesProvider,
    foundation::FoundationLocalProvider, openai::OpenAiProvider, openrouter::OpenRouterProvider,
};
use crate::{
    ProviderAccountOperationsHandle, ProviderAccountPersistenceHandle, ProviderConfig,
    ProviderError, ProviderHandle, ProviderKind, erase_model_provider,
};

const DEFAULT_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";

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
) -> Result<(ProviderKind, ProviderHandle), ProviderError> {
    match config {
        ProviderConfig::Codex(mut config) => {
            config.system_errors = Some(system_errors);
            let provider = CodexResponsesProvider::new_with_credentials(
                config,
                DEFAULT_CODEX_PROVIDER_ACCOUNT_ID,
                credentials,
            )?;
            Ok((ProviderKind::Codex, erase_model_provider(provider)))
        }
        ProviderConfig::OpenAi(mut config) => {
            config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenAi,
                erase_model_provider(OpenAiProvider::new(config)?),
            ))
        }
        ProviderConfig::OpenRouter(mut config) => {
            config.system_errors = Some(system_errors);
            Ok((
                ProviderKind::OpenRouter,
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
                ProviderKind::FoundationLocal,
                erase_model_provider(FoundationLocalProvider::new(config)?),
            ))
        }
        ProviderConfig::LocalModels(_) => Err(ProviderError::InvalidRequest {
            message: "local_models is not a hosted provider adapter".to_string(),
        }),
    }
}
