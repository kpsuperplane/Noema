use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_providers::{
    CompleteProviderAuthCallbackRequest, NoemaModelUseCase, ProviderAccountStatus,
    ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    ProviderSelectionSnapshot, StartProviderAuthRequest,
};
use noema_store::{ProviderSetupRole, ReadyProviderSetupSelection};

use noema_host::{OnboardingStatus, OnboardingStepStatus};

use super::agents::{
    GraphqlAgentModelProfileOption, GraphqlAgentModelRecommendation,
    GraphqlModelPreferenceSelectionMode, GraphqlReasoningEffort, recommendations_from_account,
    resolve_preference_input, selectable_model_account, selectable_profiles_from_account,
};
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
    /// Provider family for this step.
    pub provider_kind: String,
    /// Provider account id for this step.
    pub provider_account_id: String,
    /// Provider-local account key for this step.
    pub account_key: String,
    /// Human-readable account name for this step.
    pub display_name: String,
    /// Last known provider account status.
    pub provider_account_status: GraphqlProviderAccountStatus,
    /// Auth method when the step can start auth.
    pub auth_method: GraphqlProviderAuthMethod,
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
                    provider_account_status: step.provider_account_status.into(),
                    auth_method: step.auth_method.into(),
                })
                .collect(),
        }
    }
}

/// One proposed or selected first-run model assignment.
#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
#[graphql(name = "OnboardingModelSelection")]
pub struct GraphqlOnboardingModelSelection {
    /// Whether Noema or the human chooses the concrete model.
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    /// Provider-specific model profile.
    pub model_profile: Option<String>,
    /// Provider-supported reasoning effort.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether this assignment requests faster service.
    pub fast_mode: bool,
}

/// Complete user-facing first-run model assignment set.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "OnboardingModelSelections")]
pub struct GraphqlOnboardingModelSelections {
    pub noema: GraphqlOnboardingModelSelection,
    pub simple_tasks: GraphqlOnboardingModelSelection,
    pub medium_tasks: GraphqlOnboardingModelSelection,
    pub difficult_tasks: GraphqlOnboardingModelSelection,
    pub task_reviewer: GraphqlOnboardingModelSelection,
    pub web_fetch_summarizer: GraphqlOnboardingModelSelection,
    pub tool_progress_audit: GraphqlOnboardingModelSelection,
    pub action_reviewer: Option<GraphqlOnboardingModelSelection>,
    pub memory_consolidation: GraphqlOnboardingModelSelection,
}

/// Ready account, selectable profiles, and Noema's first-run proposal.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "OnboardingModelSetup")]
pub struct GraphqlOnboardingModelSetup {
    pub provider_kind: String,
    pub provider_account_id: String,
    pub provider_display_name: String,
    pub profiles: Vec<GraphqlAgentModelProfileOption>,
    pub recommendations: Vec<GraphqlAgentModelRecommendation>,
    pub proposed_selections: GraphqlOnboardingModelSelections,
}

/// One model assignment submitted during first-run setup.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "OnboardingModelSelectionInput")]
pub struct GraphqlOnboardingModelSelectionInput {
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    pub model_profile: Option<String>,
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    pub fast_mode: bool,
}

/// Complete first-run model assignment input.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ConfirmOnboardingModelSelectionsInput")]
pub struct GraphqlConfirmOnboardingModelSelectionsInput {
    pub provider_account_id: String,
    pub noema: GraphqlOnboardingModelSelectionInput,
    pub simple_tasks: GraphqlOnboardingModelSelectionInput,
    pub medium_tasks: GraphqlOnboardingModelSelectionInput,
    pub difficult_tasks: GraphqlOnboardingModelSelectionInput,
    pub task_reviewer: GraphqlOnboardingModelSelectionInput,
    pub web_fetch_summarizer: GraphqlOnboardingModelSelectionInput,
    pub tool_progress_audit: GraphqlOnboardingModelSelectionInput,
    pub action_reviewer: Option<GraphqlOnboardingModelSelectionInput>,
    pub memory_consolidation: GraphqlOnboardingModelSelectionInput,
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

pub(super) async fn onboarding_model_setup(
    state: &GraphqlState,
    provider_account_id: String,
) -> Result<GraphqlOnboardingModelSetup> {
    let account = selectable_model_account(state, &provider_account_id).await?;
    let profiles = selectable_profiles_from_account(state.store()?, &account).await?;
    let proposed_selections = proposed_model_selections(&account.provider_kind, &profiles)?;
    let recommendations = recommendations_from_account(&account, &profiles);
    Ok(GraphqlOnboardingModelSetup {
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id,
        provider_display_name: account.display_name,
        profiles,
        recommendations,
        proposed_selections,
    })
}

fn proposed_model_selections(
    provider_kind: &str,
    profiles: &[GraphqlAgentModelProfileOption],
) -> Result<GraphqlOnboardingModelSelections> {
    if matches!(provider_kind, "codex" | "openai" | "openrouter") {
        let recommended = GraphqlOnboardingModelSelection {
            selection_mode: GraphqlModelPreferenceSelectionMode::NoemaRecommended,
            model_profile: None,
            reasoning_effort: None,
            fast_mode: false,
        };
        return Ok(GraphqlOnboardingModelSelections {
            noema: recommended.clone(),
            simple_tasks: recommended.clone(),
            medium_tasks: recommended.clone(),
            difficult_tasks: recommended.clone(),
            task_reviewer: recommended.clone(),
            web_fetch_summarizer: recommended.clone(),
            tool_progress_audit: recommended.clone(),
            action_reviewer: Some(recommended.clone()),
            memory_consolidation: recommended,
        });
    }
    let first = profiles
        .iter()
        .find(|profile| profile.disabled_reason.is_none())
        .ok_or_else(|| async_graphql::Error::new("provider account has no available models"))?;
    let primary = first;
    let lightweight = first;
    let primary = proposed_model_selection(primary);
    let lightweight = proposed_model_selection(lightweight);
    Ok(GraphqlOnboardingModelSelections {
        noema: primary.clone(),
        simple_tasks: lightweight.clone(),
        medium_tasks: primary.clone(),
        difficult_tasks: primary.clone(),
        task_reviewer: primary.clone(),
        web_fetch_summarizer: lightweight.clone(),
        tool_progress_audit: lightweight.clone(),
        action_reviewer: (provider_kind != "local_models").then(|| primary.clone()),
        memory_consolidation: lightweight,
    })
}

fn proposed_model_selection(
    profile: &GraphqlAgentModelProfileOption,
) -> GraphqlOnboardingModelSelection {
    GraphqlOnboardingModelSelection {
        selection_mode: GraphqlModelPreferenceSelectionMode::ExplicitProfile,
        model_profile: Some(profile.id.clone()),
        reasoning_effort: profile
            .default_reasoning_effort
            .or_else(|| profile.reasoning_efforts.first().copied()),
        fast_mode: false,
    }
}

pub(super) async fn confirm_onboarding_model_selections(
    state: &GraphqlState,
    input: GraphqlConfirmOnboardingModelSelectionsInput,
) -> Result<GraphqlOnboardingStatus> {
    let setup = onboarding_model_setup(state, input.provider_account_id.clone()).await?;
    if setup.provider_kind == "local_models" && input.action_reviewer.is_some() {
        return Err(async_graphql::Error::new(
            "local model setup keeps governed action review with the human",
        ));
    }
    if setup.provider_kind != "local_models" && input.action_reviewer.is_none() {
        return Err(async_graphql::Error::new(
            "action reviewer model is required",
        ));
    }
    let requested = [
        (
            ProviderSetupRole::Noema,
            input.noema,
            NoemaModelUseCase::Primary,
        ),
        (
            ProviderSetupRole::SimpleTasks,
            input.simple_tasks,
            NoemaModelUseCase::TaskSimple,
        ),
        (
            ProviderSetupRole::MediumTasks,
            input.medium_tasks,
            NoemaModelUseCase::TaskMedium,
        ),
        (
            ProviderSetupRole::DifficultTasks,
            input.difficult_tasks,
            NoemaModelUseCase::TaskDifficult,
        ),
        (
            ProviderSetupRole::TaskReviewer,
            input.task_reviewer,
            NoemaModelUseCase::TaskReviewer,
        ),
        (
            ProviderSetupRole::WebFetchSummarizer,
            input.web_fetch_summarizer,
            NoemaModelUseCase::WebFetchSummarizer,
        ),
        (
            ProviderSetupRole::ToolProgressAudit,
            input.tool_progress_audit,
            NoemaModelUseCase::ToolProgressAudit,
        ),
        (
            ProviderSetupRole::MemoryConsolidation,
            input.memory_consolidation,
            NoemaModelUseCase::MemoryConsolidation,
        ),
    ];
    let mut assignments = Vec::with_capacity(9);
    for (role, selected, proposed) in requested {
        assignments.push(ready_setup_selection(state, &setup, role, selected, proposed).await?);
    }
    if let Some(selected) = input.action_reviewer {
        assignments.push(
            ready_setup_selection(
                state,
                &setup,
                ProviderSetupRole::ActionReviewer,
                selected,
                NoemaModelUseCase::ActionReviewer,
            )
            .await?,
        );
    }
    state
        .store()?
        .confirm_provider_setup_selections(&assignments)
        .await
        .map_err(graphql_error)?;
    onboarding_status(state).await
}

async fn ready_setup_selection(
    state: &GraphqlState,
    setup: &GraphqlOnboardingModelSetup,
    role: ProviderSetupRole,
    selected: GraphqlOnboardingModelSelectionInput,
    use_case: NoemaModelUseCase,
) -> Result<ReadyProviderSetupSelection> {
    let account = selectable_model_account(state, &setup.provider_account_id).await?;
    let (preference, model_profile, reasoning) = resolve_preference_input(
        state.store()?,
        &account,
        selected.selection_mode,
        selected.model_profile,
        selected.reasoning_effort,
        use_case,
    )
    .await?;
    let ready = super::provider_selection::prove_ready_selection(
        state,
        &setup.provider_kind,
        &setup.provider_account_id,
        &model_profile,
        reasoning,
        "graphql_onboarding_model_selection",
    )
    .await?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        &setup.provider_kind,
        &setup.provider_account_id,
        model_profile,
        reasoning,
        Some("onboarding_model_selection".to_string()),
    );
    selection.provider_instance_key = Some(ready.key().clone());
    selection.fast_mode = selected.fast_mode;
    Ok(ReadyProviderSetupSelection {
        role,
        selection,
        ready,
        preference,
    })
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

#[cfg(test)]
mod proposal_tests {
    use super::*;

    fn profile(id: &str, effort: Option<GraphqlReasoningEffort>) -> GraphqlAgentModelProfileOption {
        GraphqlAgentModelProfileOption {
            id: id.to_string(),
            label: id.to_string(),
            reasoning_efforts: effort.into_iter().collect(),
            default_reasoning_effort: effort,
            disabled_reason: None,
        }
    }

    #[test]
    fn onboarding_proposals_are_role_aware_for_each_first_run_provider() {
        let profiles = vec![profile(
            "available-model",
            Some(GraphqlReasoningEffort::Low),
        )];
        for provider in ["codex", "openai", "openrouter"] {
            let proposal = proposed_model_selections(provider, &profiles).expect("proposal");
            for selection in [
                &proposal.noema,
                &proposal.simple_tasks,
                &proposal.medium_tasks,
                &proposal.difficult_tasks,
                &proposal.task_reviewer,
                &proposal.web_fetch_summarizer,
                &proposal.tool_progress_audit,
                proposal.action_reviewer.as_ref().expect("action reviewer"),
                &proposal.memory_consolidation,
            ] {
                assert_eq!(
                    selection.selection_mode,
                    GraphqlModelPreferenceSelectionMode::NoemaRecommended
                );
                assert_eq!(selection.model_profile, None);
                assert_eq!(selection.reasoning_effort, None);
            }
        }

        let local = proposed_model_selections("local_models", &profiles).expect("local proposal");
        assert_eq!(
            local.noema.selection_mode,
            GraphqlModelPreferenceSelectionMode::ExplicitProfile
        );
        assert_eq!(
            local.noema.model_profile.as_deref(),
            Some("available-model")
        );
        assert!(local.action_reviewer.is_none());
    }
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
    let callback = url::Url::parse(callback_url)
        .map_err(|_| async_graphql::Error::new("invalid provider OAuth callback"))?;
    let expected = url::Url::parse(state.provider_oauth_callback_url()?)
        .map_err(|_| async_graphql::Error::new("provider OAuth callback is unavailable"))?;
    let (attempt_id, code) = provider_oauth_callback_parameters(&callback, &expected)?;
    state
        .provider_account_operations()?
        .complete_auth_callback(CompleteProviderAuthCallbackRequest { attempt_id, code })
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

fn provider_oauth_callback_parameters(
    callback: &url::Url,
    expected: &url::Url,
) -> Result<(String, String)> {
    let expected_path = expected.path().trim_end_matches('/');
    let attempt_id = callback
        .path()
        .strip_prefix(expected_path)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .filter(|value| {
            value.len() == 32
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        });
    if callback.scheme() != expected.scheme()
        || callback.host_str() != expected.host_str()
        || callback.port_or_known_default() != expected.port_or_known_default()
        || attempt_id.is_none()
    {
        return Err(async_graphql::Error::new(
            "provider OAuth callback does not match this Noema process",
        ));
    }
    let code = callback
        .query_pairs()
        .find(|(name, _)| name == "code")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| async_graphql::Error::new("missing provider OAuth code"))?;
    Ok((attempt_id.expect("validated attempt id").to_string(), code))
}

#[cfg(test)]
mod tests {
    use super::provider_oauth_callback_parameters;

    #[test]
    fn provider_callback_keeps_attempt_identity_in_the_path() {
        let expected = url::Url::parse("http://localhost:3737/provider/oauth/callback")
            .expect("expected callback");
        let attempt_id = "abcdEFGH01234567ijklMNOP89012345";
        let callback = url::Url::parse(&format!(
            "http://localhost:3737/provider/oauth/callback/{attempt_id}?code=secret"
        ))
        .expect("callback");
        assert_eq!(
            provider_oauth_callback_parameters(&callback, &expected).expect("valid callback"),
            (attempt_id.to_string(), "secret".to_string())
        );

        for invalid in [
            "http://localhost:3737/provider/oauth/callback?code=secret",
            "http://localhost:3737/provider/oauth/callback/too-short?code=secret",
            "http://localhost:3738/provider/oauth/callback/abcdEFGH01234567ijklMNOP89012345?code=secret",
        ] {
            let callback = url::Url::parse(invalid).expect("invalid callback fixture");
            assert!(provider_oauth_callback_parameters(&callback, &expected).is_err());
        }
    }
}
