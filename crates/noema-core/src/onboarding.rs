//! Onboarding status derived from configured provider accounts.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};

const PROVIDER_LOGIN_STEP_ID: &str = "connect_provider_account";
const LOCAL_MODEL_STEP_ID: &str = "install_local_model";
const DEFAULT_CODEX_PROVIDER_KIND: &str = "codex";
const DEFAULT_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";
const DEFAULT_CODEX_ACCOUNT_KEY: &str = "default";
const DEFAULT_CODEX_DISPLAY_NAME: &str = "Codex";

/// Status of one onboarding step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum OnboardingStepStatus {
    /// The step is complete.
    Complete,
    /// The step blocks the user from being onboarded.
    Blocked,
}

/// One frontend-visible onboarding step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct OnboardingStep {
    /// Stable onboarding step id.
    pub id: String,
    /// Current step completion state.
    pub status: OnboardingStepStatus,
    /// Provider family connected by this step, such as `codex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider_kind: Option<String>,
    /// Stable provider account id connected by this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider_account_id: Option<String>,
    /// Provider-local account key connected by this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account_key: Option<String>,
    /// Human-readable provider account name connected by this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub display_name: Option<String>,
    /// Last known provider account readiness status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider_account_status: Option<ProviderAccountStatus>,
    /// Authentication method expected for this provider account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auth_method: Option<ProviderAuthMethod>,
}

/// Frontend-visible onboarding status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct OnboardingStatus {
    /// Whether the user can proceed past onboarding.
    pub is_user_onboarded: bool,
    /// Ordered onboarding steps for the frontend to render.
    pub steps: Vec<OnboardingStep>,
}

/// Build onboarding status from the active provider account.
#[must_use]
pub fn onboarding_status_from_account(account: Option<ProviderAccountRecord>) -> OnboardingStatus {
    match account {
        Some(account)
            if account.status == ProviderAccountStatus::Authenticated
                || (account.auth_method == ProviderAuthMethod::None
                    && account.status == ProviderAccountStatus::Unknown) =>
        {
            OnboardingStatus {
                is_user_onboarded: true,
                steps: vec![provider_account_step(
                    account,
                    OnboardingStepStatus::Complete,
                )],
            }
        }
        Some(account) => OnboardingStatus {
            is_user_onboarded: false,
            steps: vec![provider_account_step(
                account,
                OnboardingStepStatus::Blocked,
            )],
        },
        None => OnboardingStatus {
            is_user_onboarded: false,
            steps: vec![default_codex_provider_step()],
        },
    }
}

/// Build onboarding status from the two supported first-run paths.
///
/// A ready local model and an authenticated provider account are alternatives:
/// either one opens Noema. The ordered steps keep the local path first so the
/// product can recommend private on-device inference without making cloud
/// providers unavailable later.
#[must_use]
pub fn onboarding_status_from_options(
    account: Option<ProviderAccountRecord>,
    local_model_ready: bool,
) -> OnboardingStatus {
    let provider_status = onboarding_status_from_account(account);
    let mut steps = Vec::with_capacity(provider_status.steps.len() + 1);
    steps.push(local_model_step(local_model_ready));
    steps.extend(provider_status.steps);

    OnboardingStatus {
        is_user_onboarded: local_model_ready || provider_status.is_user_onboarded,
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

fn default_codex_provider_step() -> OnboardingStep {
    OnboardingStep {
        id: PROVIDER_LOGIN_STEP_ID.to_string(),
        status: OnboardingStepStatus::Blocked,
        provider_kind: Some(DEFAULT_CODEX_PROVIDER_KIND.to_string()),
        provider_account_id: Some(DEFAULT_CODEX_PROVIDER_ACCOUNT_ID.to_string()),
        account_key: Some(DEFAULT_CODEX_ACCOUNT_KEY.to_string()),
        display_name: Some(DEFAULT_CODEX_DISPLAY_NAME.to_string()),
        provider_account_status: Some(ProviderAccountStatus::Unknown),
        auth_method: Some(ProviderAuthMethod::OauthDeviceCode),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ProviderAccountStatus, ProviderAuthMethod};

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
            capabilities: crate::provider::capabilities_for_provider_account(
                "codex", "default", status,
            ),
        }
    }

    #[test]
    fn onboarding_complete_when_provider_authenticated() {
        let status = onboarding_status_from_account(Some(codex_default_account(
            ProviderAccountStatus::Authenticated,
        )));

        assert!(status.is_user_onboarded);
        assert_eq!(status.steps[0].status, OnboardingStepStatus::Complete);
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
    fn onboarding_blocked_when_provider_unauthenticated() {
        let status = onboarding_status_from_account(Some(codex_default_account(
            ProviderAccountStatus::Unauthenticated,
        )));
        let step = serde_json::to_value(&status.steps[0]).expect("step json");

        assert!(!status.is_user_onboarded);
        assert_eq!(status.steps[0].id, "connect_provider_account");
        assert_eq!(status.steps[0].status, OnboardingStepStatus::Blocked);
        assert_eq!(status.steps[0].provider_kind.as_deref(), Some("codex"));
        assert_eq!(
            status.steps[0].provider_account_status,
            Some(ProviderAccountStatus::Unauthenticated)
        );
        assert_eq!(step["provider_account_status"], "unauthenticated");
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
                onboarding_status_from_account(Some(codex_default_account(account_status)));
            let step = serde_json::to_value(&status.steps[0]).expect("step json");

            assert!(!status.is_user_onboarded);
            assert_eq!(status.steps[0].status, OnboardingStepStatus::Blocked);
            assert_eq!(
                status.steps[0].provider_account_status,
                Some(account_status)
            );
            assert_eq!(
                step["provider_account_status"],
                serde_json::to_value(account_status).expect("account status json")
            );
        }
    }

    #[test]
    fn onboarding_blocked_when_no_active_account() {
        let status = onboarding_status_from_account(None);
        let step = serde_json::to_value(&status.steps[0]).expect("step json");

        assert!(!status.is_user_onboarded);
        assert_eq!(status.steps[0].id, "connect_provider_account");
        assert_eq!(status.steps[0].status, OnboardingStepStatus::Blocked);
        assert_eq!(status.steps[0].provider_kind.as_deref(), Some("codex"));
        assert_eq!(
            status.steps[0].provider_account_id.as_deref(),
            Some("provider_account:codex:default")
        );
        assert_eq!(status.steps[0].display_name.as_deref(), Some("Codex"));
        assert_eq!(
            status.steps[0].auth_method,
            Some(ProviderAuthMethod::OauthDeviceCode)
        );
        assert_eq!(
            status.steps[0].provider_account_status,
            Some(ProviderAccountStatus::Unknown)
        );
        assert_eq!(step["provider_account_status"], "unknown");
    }
}
