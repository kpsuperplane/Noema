use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_providers::{
    ProviderAccountStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    StartProviderAuthRequest,
};

use crate::OnboardingStatus;

use super::{errors::graphql_error, schema::GraphqlState};

/// Provider auth method exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ProviderAuthMethod")]
pub enum GraphqlProviderAuthMethod {
    /// OAuth device-code flow.
    OauthDeviceCode,
    /// Secret input flow.
    SecretInput,
    /// External manual flow.
    ExternalManual,
    /// No auth required.
    None,
}

impl From<ProviderAuthMethod> for GraphqlProviderAuthMethod {
    fn from(method: ProviderAuthMethod) -> Self {
        match method {
            ProviderAuthMethod::OauthDeviceCode => Self::OauthDeviceCode,
            ProviderAuthMethod::SecretInput => Self::SecretInput,
            ProviderAuthMethod::ExternalManual => Self::ExternalManual,
            ProviderAuthMethod::None => Self::None,
        }
    }
}

impl From<GraphqlProviderAuthMethod> for ProviderAuthMethod {
    fn from(method: GraphqlProviderAuthMethod) -> Self {
        match method {
            GraphqlProviderAuthMethod::OauthDeviceCode => Self::OauthDeviceCode,
            GraphqlProviderAuthMethod::SecretInput => Self::SecretInput,
            GraphqlProviderAuthMethod::ExternalManual => Self::ExternalManual,
            GraphqlProviderAuthMethod::None => Self::None,
        }
    }
}

/// Provider account status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ProviderAccountStatus")]
pub enum GraphqlProviderAccountStatus {
    /// Status has not been checked.
    Unknown,
    /// Status is being checked.
    Checking,
    /// Account is authenticated.
    Authenticated,
    /// Account is unauthenticated.
    Unauthenticated,
    /// Provider is unavailable.
    Unavailable,
}

impl From<ProviderAccountStatus> for GraphqlProviderAccountStatus {
    fn from(status: ProviderAccountStatus) -> Self {
        match status {
            ProviderAccountStatus::Unknown => Self::Unknown,
            ProviderAccountStatus::Checking => Self::Checking,
            ProviderAccountStatus::Authenticated => Self::Authenticated,
            ProviderAccountStatus::Unauthenticated => Self::Unauthenticated,
            ProviderAccountStatus::Unavailable => Self::Unavailable,
        }
    }
}

/// Onboarding step status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "OnboardingStepStatus")]
pub enum GraphqlOnboardingStepStatus {
    /// Step is complete.
    Complete,
    /// Step blocks the user from continuing.
    Blocked,
}

/// One onboarding step.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "OnboardingStep")]
pub struct GraphqlOnboardingStep {
    /// Stable step id.
    pub id: String,
    /// Step status.
    pub status: GraphqlOnboardingStepStatus,
    /// Provider family when the step is provider-backed.
    pub provider_kind: Option<String>,
    /// Provider account id when the step is provider-backed.
    pub provider_account_id: Option<String>,
    /// Provider-local account key when the step is provider-backed.
    pub account_key: Option<String>,
    /// Human-readable account name when the step is provider-backed.
    pub display_name: Option<String>,
    /// Last known provider account status.
    pub provider_account_status: Option<GraphqlProviderAccountStatus>,
    /// Auth method when the step can start auth.
    pub auth_method: Option<GraphqlProviderAuthMethod>,
}

/// Onboarding status.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "OnboardingStatus")]
pub struct GraphqlOnboardingStatus {
    /// Whether chat can start.
    pub is_user_onboarded: bool,
    /// Ordered onboarding steps.
    pub steps: Vec<GraphqlOnboardingStep>,
}

impl From<OnboardingStatus> for GraphqlOnboardingStatus {
    fn from(status: OnboardingStatus) -> Self {
        Self {
            is_user_onboarded: status.is_user_onboarded,
            steps: status
                .steps
                .into_iter()
                .map(|step| GraphqlOnboardingStep {
                    id: step.id,
                    status: match step.status {
                        crate::OnboardingStepStatus::Complete => {
                            GraphqlOnboardingStepStatus::Complete
                        }
                        crate::OnboardingStepStatus::Blocked => {
                            GraphqlOnboardingStepStatus::Blocked
                        }
                    },
                    provider_kind: step.provider_kind,
                    provider_account_id: step.provider_account_id,
                    account_key: step.account_key,
                    display_name: step.display_name,
                    provider_account_status: step.provider_account_status.map(Into::into),
                    auth_method: step.auth_method.map(Into::into),
                })
                .collect(),
        }
    }
}

/// Input for starting a provider auth attempt.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "StartProviderAuthAttemptInput")]
pub struct GraphqlStartProviderAuthAttemptInput {
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Requested authentication method.
    pub method: GraphqlProviderAuthMethod,
}

/// Provider auth attempt status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ProviderAuthAttemptStatus")]
pub enum GraphqlProviderAuthAttemptStatus {
    /// Attempt is starting.
    Starting,
    /// Waiting for the user.
    WaitingForUser,
    /// Attempt completed.
    Completed,
    /// Attempt failed.
    Failed,
    /// Attempt expired.
    Expired,
    /// Attempt was cancelled.
    Cancelled,
}

impl From<ProviderAuthAttemptStatus> for GraphqlProviderAuthAttemptStatus {
    fn from(status: ProviderAuthAttemptStatus) -> Self {
        match status {
            ProviderAuthAttemptStatus::Starting => Self::Starting,
            ProviderAuthAttemptStatus::WaitingForUser => Self::WaitingForUser,
            ProviderAuthAttemptStatus::Completed => Self::Completed,
            ProviderAuthAttemptStatus::Failed => Self::Failed,
            ProviderAuthAttemptStatus::Expired => Self::Expired,
            ProviderAuthAttemptStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Provider auth attempt view.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProviderAuthAttempt")]
pub struct GraphqlProviderAuthAttempt {
    /// Short-lived auth attempt id.
    pub attempt_id: String,
    /// Provider family.
    pub provider_kind: String,
    /// Provider account id.
    pub provider_account_id: String,
    /// Provider account auth method.
    pub method: GraphqlProviderAuthMethod,
    /// Attempt status.
    pub status: GraphqlProviderAuthAttemptStatus,
    /// Verification URL when available.
    pub verification_url: Option<String>,
    /// User code when available.
    pub user_code: Option<String>,
    /// Static UI-safe instruction text.
    pub instructions: Option<String>,
    /// Stable non-secret error code.
    pub error_code: Option<String>,
    /// Failure message when available.
    pub error_message: Option<String>,
}

impl From<ProviderAuthAttemptView> for GraphqlProviderAuthAttempt {
    fn from(view: ProviderAuthAttemptView) -> Self {
        Self {
            attempt_id: view.attempt_id,
            provider_kind: view.provider_kind,
            provider_account_id: view.provider_account_id,
            method: view.method.into(),
            status: view.status.into(),
            verification_url: view.verification_url,
            user_code: view.user_code,
            instructions: view.instructions,
            error_code: view.error_code,
            error_message: view.error_message,
        }
    }
}

pub(super) async fn onboarding_status(state: &GraphqlState) -> Result<GraphqlOnboardingStatus> {
    let store = state.store()?;
    let provider_account_operations = state.provider_account_operations()?;
    let local_model_ready = store
        .list_local_model_installations()
        .await
        .map_err(graphql_error)?
        .into_iter()
        .any(|installation| {
            installation.is_active
                && installation.status == noema_providers::LocalModelInstallationStatus::Installed
        });
    let selected_preference = store
        .get_agent_runtime_preference("agent:primary")
        .await
        .map_err(graphql_error)?;
    let accounts = provider_account_operations
        .active_accounts()
        .await
        .map_err(graphql_error)?;
    let account = if let Some(preference) =
        selected_preference.filter(|preference| preference.provider_kind != "local_models")
    {
        accounts
            .iter()
            .find(|account| account.provider_account_id == preference.provider_account_id)
    } else {
        let fallback_provider = state
            .runtime()
            .map_or("codex", crate::daemon::CodexRuntimeHandle::provider_kind);
        accounts
            .iter()
            .find(|account| account.provider_kind == fallback_provider && account.is_default)
    };
    let account = match account {
        Some(account) => Some(
            provider_account_operations
                .reconcile_account(&account.provider_account_id)
                .await
                .map_err(graphql_error)?,
        ),
        None => None,
    };

    Ok(crate::onboarding_status_from_options(account, local_model_ready).into())
}

pub(super) async fn provider_auth_attempt(
    state: &GraphqlState,
    attempt_id: String,
) -> Result<Option<GraphqlProviderAuthAttempt>> {
    state
        .provider_account_operations()?
        .auth_attempt(&attempt_id)
        .await
        .map(|attempt| attempt.map(Into::into))
        .map_err(graphql_error)
}

pub(super) async fn start_provider_auth_attempt(
    state: &GraphqlState,
    input: GraphqlStartProviderAuthAttemptInput,
) -> Result<GraphqlProviderAuthAttempt> {
    let request = StartProviderAuthRequest {
        provider_kind: input.provider_kind,
        provider_account_id: input.provider_account_id,
        method: input.method.into(),
    };
    state
        .provider_account_operations()?
        .start_auth(request)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(crate) fn is_user_onboarded_for_chat(
    account: Option<noema_providers::ProviderAccountRecord>,
) -> bool {
    account.is_some_and(|account| {
        account.is_active
            && account.is_default
            && (account.status == ProviderAccountStatus::Authenticated
                || (account.auth_method == ProviderAuthMethod::None
                    && account.status == ProviderAccountStatus::Unknown))
    })
}
