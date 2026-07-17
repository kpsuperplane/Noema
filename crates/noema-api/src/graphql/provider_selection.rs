//! Registry-backed readiness proofs for GraphQL provider-selection writes.

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelInstallationStatus, ProviderReadySelection,
    ProviderSelectionSnapshot, ReasoningEffort,
};

use super::{GraphqlState, errors::graphql_error};

pub(super) async fn prove_ready_selection(
    state: &GraphqlState,
    provider_kind: &str,
    provider_account_id: &str,
    model_profile: &str,
    reasoning_effort: Option<ReasoningEffort>,
    selection_source: &'static str,
) -> async_graphql::Result<ProviderReadySelection> {
    let provider_instance_key = if provider_kind == "local_models" {
        if provider_account_id != LOCAL_MODELS_PROVIDER_ACCOUNT_ID {
            return Err(async_graphql::Error::new(
                "local-model provider account is invalid",
            ));
        }
        state
            .store()?
            .list_local_model_installations()
            .await
            .map_err(graphql_error)?
            .into_iter()
            .find(|installation| {
                installation.model_id == model_profile
                    && installation.status == LocalModelInstallationStatus::Installed
                    && installation.is_active
                    && installation.runtime_retired_at.is_none()
                    && installation.retirement_claimed_at.is_none()
            })
            .map(|installation| installation.provider_instance_key)
            .ok_or_else(|| async_graphql::Error::new("local-model instance is unavailable"))?
    } else {
        noema_providers::provider_account_instance_key(provider_account_id)
            .map_err(graphql_error)?
    };
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort,
        Some(selection_source.to_string()),
    );
    selection.provider_instance_key = Some(provider_instance_key);
    state
        .provider_registry()?
        .prove_ready_selection(selection)
        .map_err(graphql_error)
}
