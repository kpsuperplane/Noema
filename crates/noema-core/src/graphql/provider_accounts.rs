use async_graphql::{Result, SimpleObject};

use crate::{ProviderAccountRecord, ProviderAuthMethod};

use super::{errors::graphql_error, onboarding::GraphqlProviderAccountStatus, schema::GraphqlState};

/// Provider account metadata safe to show in Settings.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlProviderAccount {
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
}

impl From<ProviderAccountRecord> for GraphqlProviderAccount {
    fn from(account: ProviderAccountRecord) -> Self {
        Self {
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
        }
    }
}

const fn auth_method_label(method: ProviderAuthMethod) -> &'static str {
    method.as_str()
}

pub(super) async fn provider_accounts(
    state: &GraphqlState,
) -> Result<Vec<GraphqlProviderAccount>> {
    let store = state.store()?;
    let account = store
        .active_provider_account("codex")
        .await
        .map_err(graphql_error)?;
    Ok(account.into_iter().map(Into::into).collect())
}
