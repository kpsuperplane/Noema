use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_providers::{
    CompleteProviderAuthCallbackRequest, ProviderAccountStatus, ProviderAuthAttemptStatus,
    ProviderAuthAttemptView, ProviderAuthMethod, StartProviderAuthRequest,
};

use noema_host::{OnboardingStatus, OnboardingStepStatus};

use super::{errors::graphql_error, schema::GraphqlState};

pub(super) fn provider_operation_graphql_error(
    error: noema_providers::ProviderAccountOperationError,
) -> async_graphql::Error {
    graphql_error(error)
}

/// Provider auth method exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ProviderAuthMethod")]
pub enum GraphqlProviderAuthMethod {
    /// OAuth device-code flow.
    OauthDeviceCode,
    /// OAuth authorization-code flow with PKCE.
    OauthPkce,
    /// Secret input flow.
    SecretInput,
    /// External manual flow.
    ExternalManual,
    /// No auth required.
    None,
}

graphql_enum_bidi!(ProviderAuthMethod => GraphqlProviderAuthMethod {
    OauthDeviceCode => OauthDeviceCode,
    OauthPkce => OauthPkce,
    SecretInput => SecretInput,
    ExternalManual => ExternalManual,
    None => None,
});

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

graphql_enum_from!(ProviderAccountStatus => GraphqlProviderAccountStatus {
    Unknown => Unknown,
    Checking => Checking,
    Authenticated => Authenticated,
    Unauthenticated => Unauthenticated,
    Unavailable => Unavailable,
});

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
                        OnboardingStepStatus::Complete => GraphqlOnboardingStepStatus::Complete,
                        OnboardingStepStatus::Blocked => GraphqlOnboardingStepStatus::Blocked,
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
    /// Stable provider account id when reconnecting an existing account.
    pub provider_account_id: Option<String>,
    /// Requested authentication method.
    pub method: GraphqlProviderAuthMethod,
}

/// Input for cancelling a provider authentication attempt.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "CancelProviderAuthAttemptInput")]
pub struct GraphqlCancelProviderAuthAttemptInput {
    /// Short-lived attempt identifier.
    pub attempt_id: String,
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

graphql_enum_from!(ProviderAuthAttemptStatus => GraphqlProviderAuthAttemptStatus {
    Starting => Starting,
    WaitingForUser => WaitingForUser,
    Completed => Completed,
    Failed => Failed,
    Expired => Expired,
    Cancelled => Cancelled,
});

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
    state
        .onboarding()?
        .status()
        .await
        .map(Into::into)
        .map_err(graphql_error)
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
        provider_account_id: input
            .provider_account_id
            .unwrap_or_else(|| format!("provider_account:{}:default", input.provider_kind)),
        provider_kind: input.provider_kind,
        method: input.method.into(),
        callback_url: match input.method {
            GraphqlProviderAuthMethod::OauthPkce => {
                Some(state.provider_oauth_callback_url()?.to_string())
            }
            _ => None,
        },
    };
    state
        .provider_account_operations()?
        .start_auth(request)
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

pub(super) async fn cancel_provider_auth_attempt(
    state: &GraphqlState,
    input: GraphqlCancelProviderAuthAttemptInput,
) -> Result<Option<GraphqlProviderAuthAttempt>> {
    state
        .provider_account_operations()?
        .cancel_auth_attempt(&input.attempt_id)
        .await
        .map(|attempt| attempt.map(Into::into))
        .map_err(graphql_error)
}

/// Complete a provider OAuth callback owned by the serving shell.
///
/// # Errors
///
/// Returns a GraphQL error when the callback boundary or auth attempt is invalid.
pub async fn complete_provider_oauth_callback(
    state: &GraphqlState,
    callback_url: &str,
) -> Result<GraphqlProviderAuthAttempt> {
    let mut callback = url::Url::parse(callback_url)
        .map_err(|_| async_graphql::Error::new("invalid provider OAuth callback"))?;
    let expected = url::Url::parse(state.provider_oauth_callback_url()?)
        .map_err(|_| async_graphql::Error::new("provider OAuth callback is unavailable"))?;
    if callback.scheme() != expected.scheme()
        || callback.host_str() != expected.host_str()
        || callback.port_or_known_default() != expected.port_or_known_default()
        || callback.path() != expected.path()
    {
        return Err(async_graphql::Error::new(
            "provider OAuth callback does not match this Noema process",
        ));
    }
    let attempt_id = callback
        .query_pairs()
        .find(|(name, _)| name == "attemptId")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| async_graphql::Error::new("missing provider OAuth attempt id"))?;
    let code = callback
        .query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| async_graphql::Error::new("missing provider OAuth code"))?;
    callback.set_query(None);
    state
        .provider_account_operations()?
        .complete_auth_callback(CompleteProviderAuthCallbackRequest { attempt_id, code })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}
