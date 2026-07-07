use async_graphql::{InputObject, Result, SimpleObject};

use crate::{
    FoundationLocalProvider, FoundationLocalProviderConfig, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthMethod,
    config::DEFAULT_FOUNDATION_LOCAL_PROFILE,
    provider::{
        ProviderCapability, ResultPersistencePolicy,
        adapters::foundation_bridge_process::FoundationBridgeError,
    },
    store::NewProviderAccount,
};

use super::{
    errors::graphql_error, onboarding::GraphqlProviderAccountStatus, schema::GraphqlState,
};

/// Provider capability metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderCapability")]
pub struct GraphqlProviderCapability {
    pub capability_id: String,
    pub status: String,
    pub reliability_contract: String,
    pub data_flow_class: String,
    pub features: GraphqlCapabilityFeatures,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CapabilityFeatures")]
pub struct GraphqlCapabilityFeatures {
    pub citations: bool,
    pub direct_url_fetch: bool,
    pub js_rendering: bool,
    pub authenticated_context: bool,
    pub result_persistence: String,
}

impl From<ProviderCapability> for GraphqlProviderCapability {
    fn from(capability: ProviderCapability) -> Self {
        Self {
            capability_id: capability.capability_id.as_str().to_string(),
            status: capability.status.as_str().to_string(),
            reliability_contract: capability.reliability_contract.as_str().to_string(),
            data_flow_class: capability.data_flow_class.as_str().to_string(),
            features: GraphqlCapabilityFeatures {
                citations: capability.features.citations,
                direct_url_fetch: capability.features.direct_url_fetch,
                js_rendering: capability.features.js_rendering,
                authenticated_context: capability.features.authenticated_context,
                result_persistence: result_persistence_label(
                    capability.features.result_persistence,
                )
                .to_string(),
            },
        }
    }
}

/// Provider account metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderAccount")]
pub struct GraphqlProviderAccount {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Provider-local account key.
    pub account_key: String,
    /// Human-readable account name.
    pub display_name: String,
    /// Authentication method used for this account.
    pub auth_method: String,
    /// Last known account readiness status.
    pub status: GraphqlProviderAccountStatus,
    /// Whether the account may be selected.
    pub is_active: bool,
    /// Whether the account is the default account for its provider.
    pub is_default: bool,
    /// Last time Noema checked the account status.
    pub last_checked_at: Option<String>,
    /// Last time Noema observed successful authentication.
    pub last_authenticated_at: Option<String>,
    /// Last non-secret provider error code.
    pub last_error_code: Option<String>,
    /// Last non-secret provider error message.
    pub last_error_message: Option<String>,
    /// Provider capabilities available through this account.
    pub capabilities: Vec<GraphqlProviderCapability>,
}

impl From<ProviderAccountRecord> for GraphqlProviderAccount {
    fn from(account: ProviderAccountRecord) -> Self {
        Self {
            provider_account_id: account.provider_account_id,
            provider_kind: account.provider_kind,
            account_key: account.account_key,
            display_name: account.display_name,
            auth_method: auth_method_label(account.auth_method).to_string(),
            status: account.status.into(),
            is_active: account.is_active,
            is_default: account.is_default,
            last_checked_at: account.last_checked_at,
            last_authenticated_at: account.last_authenticated_at,
            last_error_code: account.last_error_code,
            last_error_message: account.last_error_message,
            capabilities: account.capabilities.into_iter().map(Into::into).collect(),
        }
    }
}

/// Provider type that can be added in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderAccountCatalogEntry")]
pub struct GraphqlProviderAccountCatalogEntry {
    pub provider_kind: String,
    pub display_name: String,
    pub auth_method: String,
    pub capabilities: Vec<GraphqlProviderCapability>,
}

/// Input for creating a provider account.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateProviderAccountInput")]
pub struct GraphqlCreateProviderAccountInput {
    pub provider_kind: String,
    pub display_name: Option<String>,
    pub secret: String,
}

/// Input for saving a write-only provider secret.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ProviderSecretInput")]
pub struct GraphqlProviderSecretInput {
    pub provider_account_id: String,
    pub secret: String,
}

/// Input for clearing a write-only provider secret.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ClearProviderSecretInput")]
pub struct GraphqlClearProviderSecretInput {
    pub provider_account_id: String,
}

/// Input for hard-deleting a user-managed provider account.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "DeleteProviderAccountInput")]
pub struct GraphqlDeleteProviderAccountInput {
    pub provider_account_id: String,
}

const fn auth_method_label(method: ProviderAuthMethod) -> &'static str {
    method.as_str()
}

const fn result_persistence_label(policy: ResultPersistencePolicy) -> &'static str {
    policy.as_str()
}

pub(super) async fn provider_accounts(state: &GraphqlState) -> Result<Vec<GraphqlProviderAccount>> {
    refresh_foundation_local_availability(state).await;
    let store = state.store()?;
    let accounts = store
        .active_provider_accounts()
        .await
        .map_err(graphql_error)?;
    Ok(accounts.into_iter().map(Into::into).collect())
}

pub(super) async fn provider_account_catalog(
    state: &GraphqlState,
) -> Result<Vec<GraphqlProviderAccountCatalogEntry>> {
    let store = state.store()?;
    Ok(store
        .provider_account_catalog()
        .into_iter()
        .map(|entry| GraphqlProviderAccountCatalogEntry {
            provider_kind: entry.provider_kind,
            display_name: entry.display_name,
            auth_method: entry.auth_method.as_str().to_string(),
            capabilities: entry.capabilities.into_iter().map(Into::into).collect(),
        })
        .collect())
}

pub(super) async fn create_provider_account(
    state: &GraphqlState,
    input: GraphqlCreateProviderAccountInput,
) -> Result<GraphqlProviderAccount> {
    if input.provider_kind != "exa" {
        return Err(async_graphql::Error::new("unsupported provider kind"));
    }
    reject_blank_secret(&input.secret)?;
    let store = state.store()?;
    let account = store
        .create_provider_account(NewProviderAccount {
            provider_kind: input.provider_kind,
            display_name: input.display_name,
            auth_method: ProviderAuthMethod::SecretInput,
            status: ProviderAccountStatus::Unauthenticated,
            metadata: serde_json::json!({"secretConfigured": false}),
        })
        .await
        .map_err(graphql_error)?;
    save_secret_for_account(state, account, &input.secret).await
}

pub(super) async fn save_provider_secret_input(
    state: &GraphqlState,
    input: GraphqlProviderSecretInput,
) -> Result<GraphqlProviderAccount> {
    reject_blank_secret(&input.secret)?;
    let account = secret_input_account(state, &input.provider_account_id).await?;
    save_secret_for_account(state, account, &input.secret).await
}

pub(super) async fn clear_provider_secret(
    state: &GraphqlState,
    input: GraphqlClearProviderSecretInput,
) -> Result<GraphqlProviderAccount> {
    let store = state.store()?;
    let account = secret_input_account(state, &input.provider_account_id).await?;
    let paths = state.paths()?;
    let secret_store = crate::provider::secret_input::SecretInputStore::new(
        paths.provider_account_home(&account.provider_kind, &account.account_key),
    );
    secret_store
        .clear_api_key()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Unauthenticated,
            None,
            None,
        )
        .await
        .map_err(graphql_error)?;
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            serde_json::json!({"secretConfigured": false}),
        )
        .await
        .map_err(graphql_error)?;
    refreshed_provider_account(store, &account.provider_account_id).await
}

pub(super) async fn delete_provider_account(
    state: &GraphqlState,
    input: GraphqlDeleteProviderAccountInput,
) -> Result<bool> {
    let store = state.store()?;
    let Some(account) = store
        .get_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(false);
    };
    if account.is_default {
        return Err(async_graphql::Error::new(
            "default provider accounts cannot be deleted",
        ));
    }

    let paths = state.paths()?;
    let account_home = paths.provider_account_home(&account.provider_kind, &account.account_key);
    match std::fs::remove_dir_all(&account_home) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(async_graphql::Error::new(error.to_string())),
    }

    store
        .delete_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)
}

async fn save_secret_for_account(
    state: &GraphqlState,
    account: ProviderAccountRecord,
    secret: &str,
) -> Result<GraphqlProviderAccount> {
    let store = state.store()?;
    let paths = state.paths()?;
    let secret_store = crate::provider::secret_input::SecretInputStore::new(
        paths.provider_account_home(&account.provider_kind, &account.account_key),
    );
    secret_store
        .save_api_key(secret)
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    store
        .update_provider_account_status(
            &account.provider_account_id,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .map_err(graphql_error)?;
    store
        .update_provider_account_metadata(
            &account.provider_account_id,
            serde_json::json!({"secretConfigured": true}),
        )
        .await
        .map_err(graphql_error)?;
    refreshed_provider_account(store, &account.provider_account_id).await
}

async fn secret_input_account(
    state: &GraphqlState,
    provider_account_id: &str,
) -> Result<ProviderAccountRecord> {
    let store = state.store()?;
    let account = store
        .get_provider_account(provider_account_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    if !account.is_active {
        return Err(async_graphql::Error::new("provider account not found"));
    }
    if account.provider_kind != "exa" {
        return Err(async_graphql::Error::new("unsupported provider kind"));
    }
    if account.auth_method != ProviderAuthMethod::SecretInput {
        return Err(async_graphql::Error::new(
            "provider account auth method mismatch",
        ));
    }
    Ok(account)
}

async fn refreshed_provider_account(
    store: &crate::NoemaStore,
    provider_account_id: &str,
) -> Result<GraphqlProviderAccount> {
    store
        .get_provider_account(provider_account_id)
        .await
        .map_err(graphql_error)?
        .map(Into::into)
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))
}

fn reject_blank_secret(secret: &str) -> Result<()> {
    if secret.trim().is_empty() {
        return Err(async_graphql::Error::new("api key is required"));
    }
    Ok(())
}

pub(super) async fn refresh_foundation_local_availability(state: &GraphqlState) {
    if state.paths().is_err() {
        return;
    }
    let Ok(store) = state.store() else {
        return;
    };
    let Ok(accounts) = store.active_default_provider_accounts().await else {
        return;
    };
    for account in accounts {
        if account.provider_kind != "foundation_local"
            || !account.is_active
            || !account.is_default
            || account.status == ProviderAccountStatus::Authenticated
        {
            continue;
        }
        let provider = match FoundationLocalProvider::new(FoundationLocalProviderConfig {
            default_profile: DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string(),
            bridge_path: None,
            system_errors: None,
        }) {
            Ok(provider) => provider,
            Err(error) => {
                let _ = store
                    .update_provider_account_status(
                        &account.provider_account_id,
                        ProviderAccountStatus::Unavailable,
                        Some("invalid_foundation_config"),
                        Some(&error.to_string()),
                    )
                    .await;
                continue;
            }
        };
        let update = match provider.check_availability().await {
            Ok(()) => (ProviderAccountStatus::Authenticated, None, None),
            Err(error) => (
                ProviderAccountStatus::Unavailable,
                Some(foundation_availability_error_code(&error).to_string()),
                Some(error.to_string()),
            ),
        };
        let _ = store
            .update_provider_account_status(
                &account.provider_account_id,
                update.0,
                update.1.as_deref(),
                update.2.as_deref(),
            )
            .await;
    }
}

fn foundation_availability_error_code(error: &FoundationBridgeError) -> &'static str {
    match error.code() {
        "foundation_unavailable" => "foundation_models_unavailable",
        code => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod,
        provider::capabilities_for_provider_account,
    };
    use serde_json::json;

    #[test]
    fn graphql_provider_account_exposes_capabilities() {
        let account = ProviderAccountRecord {
            provider_account_id: "provider_account:openai:default".to_string(),
            provider_kind: "openai".to_string(),
            account_key: "default".to_string(),
            display_name: "OpenAI".to_string(),
            auth_method: ProviderAuthMethod::SecretInput,
            is_active: true,
            is_default: true,
            status: ProviderAccountStatus::Authenticated,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
            capabilities: capabilities_for_provider_account(
                "openai",
                "default",
                ProviderAccountStatus::Authenticated,
            ),
        };

        let graphql = GraphqlProviderAccount::from(account);

        assert!(graphql.capabilities.iter().any(|capability| {
            capability.capability_id == "web.search"
                && capability.status == "available"
                && capability.features.citations
                && !capability.features.direct_url_fetch
        }));
    }
}
