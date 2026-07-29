//! Onboarding status and orchestration derived from configured provider accounts.

use noema_providers::{
    LocalModelManager, LocalModelRuntimeStatus, ProviderAccountOperationsHandle,
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod,
};
use thiserror::Error;

const PROVIDER_LOGIN_STEP_ID: &str = "connect_provider_account";
const LOCAL_MODEL_STEP_ID: &str = "install_local_model";

/// Status of one onboarding step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStepStatus {
    /// The step is complete.
    Complete,
    /// The step blocks the user from being onboarded.
    Blocked,
}

/// One frontend-visible onboarding step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingStep {
    /// Stable onboarding step id.
    pub id: String,
    /// Current step completion state.
    pub status: OnboardingStepStatus,
    /// Provider family connected by this step, such as `codex`.
    pub provider_kind: Option<String>,
    /// Stable provider account id connected by this step.
    pub provider_account_id: Option<String>,
    /// Provider-local account key connected by this step.
    pub account_key: Option<String>,
    /// Human-readable provider account name connected by this step.
    pub display_name: Option<String>,
    /// Last known provider account readiness status.
    pub provider_account_status: Option<ProviderAccountStatus>,
    /// Authentication method expected for this provider account.
    pub auth_method: Option<ProviderAuthMethod>,
}

/// Frontend-visible onboarding status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingStatus {
    /// Whether the user can proceed past onboarding.
    pub is_user_onboarded: bool,
    /// Ordered onboarding steps for the frontend to render.
    pub steps: Vec<OnboardingStep>,
}

/// Cross-subsystem first-run onboarding operations.
#[derive(Clone)]
pub struct OnboardingService {
    store: noema_store::NoemaStore,
    provider_accounts: ProviderAccountOperationsHandle,
    local_models: LocalModelManager,
}

impl OnboardingService {
    /// Build onboarding operations from assembled service handles.
    #[must_use]
    pub fn new(
        store: noema_store::NoemaStore,
        provider_accounts: ProviderAccountOperationsHandle,
        local_models: LocalModelManager,
    ) -> Self {
        Self {
            store,
            provider_accounts,
            local_models,
        }
    }

    /// Resolve the current first-run status from canonical provider selection
    /// and live local-model availability.
    ///
    /// # Errors
    ///
    /// Returns [`OnboardingServiceError`] when canonical state or provider
    /// account reconciliation is unavailable.
    pub async fn status(&self) -> Result<OnboardingStatus, OnboardingServiceError> {
        let selected_preference = self
            .store
            .get_agent_runtime_preference("agent:primary")
            .await
            .map_err(|error| OnboardingServiceError::Store(error.to_string()))?;
        let accounts = self
            .provider_accounts
            .active_accounts()
            .await
            .map_err(|error| OnboardingServiceError::Provider(error.to_string()))?;
        let local_model_selected = selected_preference
            .as_ref()
            .is_some_and(|preference| preference.provider_kind == "local_models");
        let account = selected_preference
            .filter(|preference| preference.provider_kind != "local_models")
            .and_then(|preference| {
                accounts
                    .iter()
                    .find(|account| account.provider_account_id == preference.provider_account_id)
                    .map(|account| account.provider_account_id.clone())
            });
        let account = match account {
            Some(provider_account_id) => Some(
                self.provider_accounts
                    .reconcile_account(&provider_account_id)
                    .await
                    .map_err(|error| OnboardingServiceError::Provider(error.to_string()))?,
            ),
            None => None,
        };
        let local_model_ready = local_model_selected
            && local_model_runtime_is_ready(&self.local_models.runtime_status());

        Ok(onboarding_status_from_options(account, local_model_ready))
    }
}

fn local_model_runtime_is_ready(status: &LocalModelRuntimeStatus) -> bool {
    matches!(status, LocalModelRuntimeStatus::Ready { .. })
}

/// Host onboarding operation failure.
#[derive(Debug, Error)]
pub enum OnboardingServiceError {
    /// Canonical onboarding state could not be loaded.
    #[error("onboarding store operation failed: {0}")]
    Store(String),
    /// Provider authentication or reconciliation failed.
    #[error("onboarding provider operation failed: {0}")]
    Provider(String),
}

/// Build onboarding status from the two supported first-run paths.
///
/// A ready local model and an authenticated provider account are alternatives:
/// either one opens Noema. The ordered steps keep the local path first so the
/// product can recommend private on-device inference without making cloud
/// providers unavailable later.
fn onboarding_status_from_options(
    account: Option<ProviderAccountRecord>,
    local_model_ready: bool,
) -> OnboardingStatus {
    let (provider_ready, provider_step) = account.map_or_else(
        || (false, None),
        |account| {
            let ready = account.status == ProviderAccountStatus::Authenticated
                || (account.auth_method == ProviderAuthMethod::None
                    && account.status == ProviderAccountStatus::Unknown);
            let status = if ready {
                OnboardingStepStatus::Complete
            } else {
                OnboardingStepStatus::Blocked
            };
            (ready, Some(provider_account_step(account, status)))
        },
    );

    let mut steps = vec![local_model_step(local_model_ready)];
    steps.extend(provider_step);

    OnboardingStatus {
        is_user_onboarded: local_model_ready || provider_ready,
        steps,
    }
}

fn local_model_step(ready: bool) -> OnboardingStep {
    OnboardingStep {
        id: LOCAL_MODEL_STEP_ID.to_string(),
        status: if ready {
            OnboardingStepStatus::Complete
        } else {
            OnboardingStepStatus::Blocked
        },
        provider_kind: Some("local_models".to_string()),
        provider_account_id: Some("provider_account:local_models:default".to_string()),
        account_key: Some("default".to_string()),
        display_name: Some("Local models".to_string()),
        provider_account_status: if ready {
            Some(ProviderAccountStatus::Authenticated)
        } else {
            Some(ProviderAccountStatus::Unknown)
        },
        auth_method: Some(ProviderAuthMethod::None),
    }
}

fn provider_account_step(
    account: ProviderAccountRecord,
    status: OnboardingStepStatus,
) -> OnboardingStep {
    let provider_account_status = account.status;

    OnboardingStep {
        id: PROVIDER_LOGIN_STEP_ID.to_string(),
        status,
        provider_kind: Some(account.provider_kind),
        provider_account_id: Some(account.provider_account_id),
        account_key: Some(account.account_key),
        display_name: Some(account.display_name),
        provider_account_status: Some(provider_account_status),
        auth_method: Some(account.auth_method),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use noema_providers::{ProviderAccountStatus, ProviderAuthMethod};

    fn codex_default_account(status: ProviderAccountStatus) -> ProviderAccountRecord {
        ProviderAccountRecord {
            provider_account_id: "provider_account:codex:default".to_string(),
            provider_kind: "codex".to_string(),
            account_key: "default".to_string(),
            display_name: "Codex".to_string(),
            auth_method: ProviderAuthMethod::OauthDeviceCode,
            is_active: true,
            is_default: true,
            status,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
            capabilities: noema_providers::capabilities_for_provider_account(
                "codex", "default", status,
            ),
        }
    }

    #[test]
    fn local_model_and_cloud_are_alternative_onboarding_paths() {
        let local = onboarding_status_from_options(None, true);
        assert!(local.is_user_onboarded);
        assert_eq!(local.steps[0].id, "install_local_model");
        assert_eq!(local.steps[0].status, OnboardingStepStatus::Complete);

        let cloud = onboarding_status_from_options(
            Some(codex_default_account(ProviderAccountStatus::Authenticated)),
            false,
        );
        assert!(cloud.is_user_onboarded);
        assert_eq!(cloud.steps[0].status, OnboardingStepStatus::Blocked);
        assert_eq!(cloud.steps[1].id, "connect_provider_account");
        assert_eq!(cloud.steps[1].status, OnboardingStepStatus::Complete);
    }

    #[test]
    fn onboarding_requires_live_local_model_runtime() {
        let cases = [
            (LocalModelRuntimeStatus::Stopped, false),
            (
                LocalModelRuntimeStatus::Ready {
                    backend: noema_providers::LocalModelBackend::Metal,
                    endpoint: "http://127.0.0.1:1".to_string(),
                    model_id: "test-local".to_string(),
                },
                true,
            ),
        ];
        for (runtime, onboarded) in cases {
            assert_eq!(
                onboarding_status_from_options(None, local_model_runtime_is_ready(&runtime))
                    .is_user_onboarded,
                onboarded
            );
        }
    }

    #[test]
    fn onboarding_blocks_and_preserves_provider_readiness_status() {
        for account_status in [
            ProviderAccountStatus::Unknown,
            ProviderAccountStatus::Checking,
            ProviderAccountStatus::Unauthenticated,
            ProviderAccountStatus::Unavailable,
        ] {
            let status =
                onboarding_status_from_options(Some(codex_default_account(account_status)), false);
            assert!(!status.is_user_onboarded);
            assert_eq!(status.steps[1].status, OnboardingStepStatus::Blocked);
            assert_eq!(
                status.steps[1].provider_account_status,
                Some(account_status)
            );
        }
    }

    #[test]
    fn onboarding_does_not_fabricate_an_account_when_none_is_connected() {
        let status = onboarding_status_from_options(None, false);
        assert!(!status.is_user_onboarded);
        assert_eq!(status.steps.len(), 1);
        assert_eq!(status.steps[0].id, "install_local_model");
    }
}
