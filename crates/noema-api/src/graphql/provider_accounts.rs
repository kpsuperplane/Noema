use async_graphql::{InputObject, Result, SimpleObject};
use noema_providers::{
    CreateSecretProviderAccountRequest, ProviderAccountRecord, ProviderCapability,
    SaveProviderAccountSecretRequest,
};

use super::{
    errors::graphql_error,
    onboarding::{GraphqlProviderAccountStatus, GraphqlProviderAuthMethod},
    schema::GraphqlState,
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
                result_persistence: capability.features.result_persistence.as_str().to_string(),
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
            auth_method: account.auth_method.as_str().to_string(),
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
    pub preferred_auth_method: GraphqlProviderAuthMethod,
    pub supported_auth_methods: Vec<GraphqlProviderAuthMethod>,
    pub capabilities: Vec<GraphqlProviderCapability>,
}

/// Input for creating a provider account.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CreateProviderAccountInput")]
pub struct GraphqlCreateProviderAccountInput {
    pub provider_kind: String,
    pub display_name: Option<String>,
    pub secret: String,
    /// Authentication method represented by the supplied secret.
    pub auth_method: GraphqlProviderAuthMethod,
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

pub(super) async fn provider_accounts(state: &GraphqlState) -> Result<Vec<GraphqlProviderAccount>> {
    let accounts = state
        .provider_account_operations()?
        .active_accounts()
        .await
        .map_err(graphql_error)?;
    Ok(accounts.into_iter().map(Into::into).collect())
}

pub(super) async fn provider_account_catalog(
    state: &GraphqlState,
) -> Result<Vec<GraphqlProviderAccountCatalogEntry>> {
    Ok(state
        .provider_account_operations()?
        .account_catalog()
        .into_iter()
        .map(|entry| GraphqlProviderAccountCatalogEntry {
            provider_kind: entry.provider_kind,
            display_name: entry.display_name,
            preferred_auth_method: entry.preferred_auth_method.into(),
            supported_auth_methods: entry
                .supported_auth_methods
                .into_iter()
                .map(Into::into)
                .collect(),
            capabilities: entry.capabilities.into_iter().map(Into::into).collect(),
        })
        .collect())
}

pub(super) async fn create_provider_account(
    state: &GraphqlState,
    input: GraphqlCreateProviderAccountInput,
) -> Result<GraphqlProviderAccount> {
    if input.auth_method != GraphqlProviderAuthMethod::SecretInput {
        return Err(async_graphql::Error::new(
            "provider account secret requires secret input authentication",
        ));
    }
    let request = CreateSecretProviderAccountRequest::new(
        input.provider_kind,
        input.display_name,
        input.secret,
    )
    .map_err(graphql_error)?;
    state
        .provider_account_operations()?
        .create_secret_account(request)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn save_provider_secret_input(
    state: &GraphqlState,
    input: GraphqlProviderSecretInput,
) -> Result<GraphqlProviderAccount> {
    let request = SaveProviderAccountSecretRequest::new(input.provider_account_id, input.secret)
        .map_err(graphql_error)?;
    state
        .provider_account_operations()?
        .save_secret(request)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn clear_provider_secret(
    state: &GraphqlState,
    input: GraphqlClearProviderSecretInput,
) -> Result<GraphqlProviderAccount> {
    state
        .provider_account_operations()?
        .clear_secret(&input.provider_account_id)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn delete_provider_account(
    state: &GraphqlState,
    input: GraphqlDeleteProviderAccountInput,
) -> Result<bool> {
    state
        .provider_account_operations()?
        .delete_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)
}
