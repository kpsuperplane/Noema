//! Provider authentication support.

use std::{collections::HashMap, sync::Arc};

use crate::{
    CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderError,
};
use tokio::sync::{Mutex, broadcast, oneshot};

use super::codex::oauth::{self, CodexDeviceAuthSession};

/// Default maximum lifetime for a provider auth attempt.
pub const DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

/// In-memory manager for short-lived provider authentication attempts.
#[derive(Clone)]
pub(crate) struct ProviderAuthManager {
    state: Arc<Mutex<ProviderAuthState>>,
    events: broadcast::Sender<ProviderAuthAttemptView>,
}

impl Default for ProviderAuthManager {
    fn default() -> Self {
        let (events, _) = broadcast::channel(32);
        Self {
            state: Arc::new(Mutex::new(ProviderAuthState::default())),
            events,
        }
    }
}

#[derive(Default)]
struct ProviderAuthState {
    attempts: HashMap<String, ProviderAuthAttempt>,
    latest_attempt_by_account: HashMap<String, String>,
    shutting_down: bool,
}

struct ProviderAuthAttempt {
    view: ProviderAuthAttemptView,
    runtime: Option<ProviderAuthAttemptRuntime>,
    completion_claimed: bool,
}

pub(crate) struct ProviderAuthAttemptRuntime {
    cancel: oneshot::Sender<()>,
}

impl ProviderAuthAttemptRuntime {
    pub(crate) fn new(cancel: oneshot::Sender<()>) -> Self {
        Self { cancel }
    }

    fn cancel(self) {
        let _ = self.cancel.send(());
    }
}

impl ProviderAuthManager {
    /// Build an empty auth attempt manager.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn begin_codex_device_code(
        &self,
        request: CodexDeviceAuthRequest,
    ) -> Result<CodexDeviceAuthSession, ProviderError> {
        if self.is_shutting_down().await {
            return Err(auth_service_shutting_down());
        }
        oauth::begin_codex_device_auth(self.clone(), request).await
    }

    /// Return a safe auth attempt view, if it is still known.
    ///
    /// # Errors
    ///
    /// This currently has no fallible backing store; the result keeps the
    /// service boundary ready for future persistence.
    pub(crate) async fn poll_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<ProviderAuthAttemptView>, ProviderError> {
        let state = self.state.lock().await;
        Ok(state
            .attempts
            .get(attempt_id)
            .map(|attempt| attempt.view.clone()))
    }

    /// Mark an auth attempt as cancelled, if it is still known.
    ///
    /// # Errors
    ///
    /// This currently has no fallible backing store; the result keeps the
    /// service boundary ready for future cancellation hooks.
    pub(crate) async fn cancel_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<ProviderAuthAttemptView>, ProviderError> {
        let (view, runtime) = {
            let mut state = self.state.lock().await;
            let Some(attempt) = state.attempts.get_mut(attempt_id) else {
                return Ok(None);
            };
            if !is_terminal_status(attempt.view.status) && !attempt.completion_claimed {
                attempt.view.status = ProviderAuthAttemptStatus::Cancelled;
                attempt.view.error_code = None;
                attempt.view.error_message = None;
                (attempt.view.clone(), attempt.runtime.take())
            } else {
                (attempt.view.clone(), None)
            }
        };
        if let Some(runtime) = runtime {
            runtime.cancel();
        }
        let _ = self.events.send(view.clone());
        Ok(Some(view))
    }

    pub(crate) async fn cancel_all_attempts(&self) -> Vec<ProviderAuthAttemptView> {
        let (views, runtimes) = {
            let mut state = self.state.lock().await;
            state.shutting_down = true;
            let mut views = Vec::new();
            let mut runtimes = Vec::new();
            for attempt in state.attempts.values_mut() {
                if is_terminal_status(attempt.view.status) || attempt.completion_claimed {
                    continue;
                }
                attempt.view.status = ProviderAuthAttemptStatus::Cancelled;
                attempt.view.error_code = None;
                attempt.view.error_message = None;
                views.push(attempt.view.clone());
                if let Some(runtime) = attempt.runtime.take() {
                    runtimes.push(runtime);
                }
            }
            (views, runtimes)
        };
        for runtime in runtimes {
            runtime.cancel();
        }
        for view in &views {
            let _ = self.events.send(view.clone());
        }
        views
    }

    pub(crate) async fn register_attempt(
        &self,
        attempt: ProviderAuthAttemptView,
        runtime: ProviderAuthAttemptRuntime,
    ) -> bool {
        let attempt_id = attempt.attempt_id.clone();
        let (previous_runtime, previous_view) = {
            let mut state = self.state.lock().await;
            if state.shutting_down {
                drop(state);
                runtime.cancel();
                return false;
            }
            let account_id = attempt.provider_account_id.clone();
            let mut previous_view = None;
            let previous_runtime = state
                .latest_attempt_by_account
                .get(&account_id)
                .cloned()
                .and_then(|previous_id| state.attempts.get_mut(&previous_id))
                .and_then(|previous| {
                    if is_terminal_status(previous.view.status) || previous.completion_claimed {
                        return None;
                    }
                    previous.view.status = ProviderAuthAttemptStatus::Cancelled;
                    previous.view.error_code = None;
                    previous.view.error_message = None;
                    previous_view = Some(previous.view.clone());
                    previous.runtime.take()
                });
            state
                .latest_attempt_by_account
                .insert(account_id, attempt.attempt_id.clone());
            state.attempts.insert(
                attempt.attempt_id.clone(),
                ProviderAuthAttempt {
                    view: attempt,
                    runtime: Some(runtime),
                    completion_claimed: false,
                },
            );
            (previous_runtime, previous_view)
        };
        if let Some(runtime) = previous_runtime {
            runtime.cancel();
        }
        if let Some(previous) = previous_view {
            let _ = self.events.send(previous);
        }
        let current = self
            .poll_attempt(&attempt_id)
            .await
            .ok()
            .flatten()
            .expect("registered auth attempt");
        let _ = self.events.send(current);
        true
    }

    /// Claim the right to publish and finish an attempt.
    ///
    /// Cancellation, supersession, and completion all compete under the same
    /// lock, so a cancelled or superseded attempt can never claim publication.
    pub(crate) async fn claim_attempt_completion(&self, attempt_id: &str) -> bool {
        let mut state = self.state.lock().await;
        let Some(attempt) = state.attempts.get(attempt_id) else {
            return false;
        };
        let is_latest = state
            .latest_attempt_by_account
            .get(&attempt.view.provider_account_id)
            .is_some_and(|latest_id| latest_id == attempt_id);
        if !is_latest || is_terminal_status(attempt.view.status) || attempt.completion_claimed {
            return false;
        }
        let attempt = state
            .attempts
            .get_mut(attempt_id)
            .expect("attempt checked above");
        attempt.completion_claimed = true;
        attempt.runtime.take();
        true
    }

    pub(crate) async fn finish_claimed_attempt(
        &self,
        attempt_id: &str,
        status: ProviderAuthAttemptStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Option<ProviderAuthAttemptView> {
        debug_assert!(is_terminal_status(status));
        let mut state = self.state.lock().await;
        let attempt = state.attempts.get_mut(attempt_id)?;
        if !attempt.completion_claimed || is_terminal_status(attempt.view.status) {
            return Some(attempt.view.clone());
        }
        attempt.view.status = status;
        attempt.view.error_code = error_code;
        attempt.view.error_message = error_message;
        let view = attempt.view.clone();
        drop(state);
        let _ = self.events.send(view.clone());
        Some(view)
    }

    pub(crate) async fn is_latest_attempt(&self, attempt_id: &str) -> bool {
        let state = self.state.lock().await;
        let Some(attempt) = state.attempts.get(attempt_id) else {
            return false;
        };
        state
            .latest_attempt_by_account
            .get(&attempt.view.provider_account_id)
            .is_some_and(|latest_id| latest_id == attempt_id)
    }

    pub(crate) async fn is_shutting_down(&self) -> bool {
        self.state.lock().await.shutting_down
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<ProviderAuthAttemptView> {
        self.events.subscribe()
    }
}

fn auth_service_shutting_down() -> ProviderError {
    ProviderError::ProviderUnavailable {
        provider: crate::CODEX_PROVIDER.to_string(),
        message: "provider authentication service is shutting down".to_string(),
    }
}

pub(crate) fn is_terminal_status(status: ProviderAuthAttemptStatus) -> bool {
    matches!(
        status,
        ProviderAuthAttemptStatus::Completed
            | ProviderAuthAttemptStatus::Failed
            | ProviderAuthAttemptStatus::Expired
            | ProviderAuthAttemptStatus::Cancelled
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn auth_manager_cancel_marks_attempt_cancelled() {
        let manager = ProviderAuthManager::new();
        let attempt = ProviderAuthAttemptView {
            attempt_id: "attempt-test".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: crate::ProviderAuthMethod::OauthDeviceCode,
            status: ProviderAuthAttemptStatus::WaitingForUser,
            verification_url: Some("https://example.com/device".to_string()),
            user_code: Some("ABD-EFGH".to_string()),
            instructions: Some("Complete the login in your browser.".to_string()),
            error_code: None,
            error_message: None,
        };
        let (cancel, _cancelled) = oneshot::channel();
        assert!(
            manager
                .register_attempt(attempt, ProviderAuthAttemptRuntime::new(cancel))
                .await
        );

        let status = manager
            .cancel_attempt("attempt-test")
            .await
            .expect("cancel")
            .expect("attempt exists");

        assert_eq!(status.status, ProviderAuthAttemptStatus::Cancelled);
        assert!(!manager.claim_attempt_completion("attempt-test").await);
    }

    #[tokio::test]
    async fn auth_manager_completion_claim_prevents_late_cancellation() {
        let manager = ProviderAuthManager::new();
        let attempt = ProviderAuthAttemptView {
            attempt_id: "attempt-test".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: crate::ProviderAuthMethod::OauthDeviceCode,
            status: ProviderAuthAttemptStatus::WaitingForUser,
            verification_url: Some("https://example.com/device".to_string()),
            user_code: Some("ABD-EFGH".to_string()),
            instructions: Some("Complete the login in your browser.".to_string()),
            error_code: None,
            error_message: None,
        };
        let (cancel, _cancelled) = oneshot::channel();
        assert!(
            manager
                .register_attempt(attempt, ProviderAuthAttemptRuntime::new(cancel))
                .await
        );

        assert!(manager.claim_attempt_completion("attempt-test").await);
        let status = manager
            .cancel_attempt("attempt-test")
            .await
            .expect("cancel")
            .expect("attempt exists");

        assert_eq!(
            status.status,
            ProviderAuthAttemptStatus::WaitingForUser,
            "completion owns the transition once claimed"
        );
        let completed = manager
            .finish_claimed_attempt(
                "attempt-test",
                ProviderAuthAttemptStatus::Completed,
                None,
                None,
            )
            .await
            .expect("attempt");
        assert_eq!(completed.status, ProviderAuthAttemptStatus::Completed);
    }

    #[tokio::test]
    async fn auth_manager_cancel_all_is_atomic_with_completion_claims() {
        let manager = ProviderAuthManager::new();
        for attempt_id in ["cancel-me", "completion-owned"] {
            let attempt = ProviderAuthAttemptView {
                attempt_id: attempt_id.to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: format!("provider_account:codex:{attempt_id}"),
                method: crate::ProviderAuthMethod::OauthDeviceCode,
                status: ProviderAuthAttemptStatus::WaitingForUser,
                verification_url: Some("https://example.com/device".to_string()),
                user_code: Some("ABD-EFGH".to_string()),
                instructions: Some("Complete the login in your browser.".to_string()),
                error_code: None,
                error_message: None,
            };
            let (cancel, _cancelled) = oneshot::channel();
            assert!(
                manager
                    .register_attempt(attempt, ProviderAuthAttemptRuntime::new(cancel))
                    .await
            );
        }
        assert!(manager.claim_attempt_completion("completion-owned").await);

        let cancelled = manager.cancel_all_attempts().await;

        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].attempt_id, "cancel-me");
        assert_eq!(
            manager
                .poll_attempt("cancel-me")
                .await
                .expect("poll")
                .expect("attempt")
                .status,
            ProviderAuthAttemptStatus::Cancelled
        );
        assert_eq!(
            manager
                .poll_attempt("completion-owned")
                .await
                .expect("poll")
                .expect("attempt")
                .status,
            ProviderAuthAttemptStatus::WaitingForUser
        );

        let replacement = ProviderAuthAttemptView {
            attempt_id: "after-shutdown".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:after-shutdown".to_string(),
            method: crate::ProviderAuthMethod::OauthDeviceCode,
            status: ProviderAuthAttemptStatus::WaitingForUser,
            verification_url: None,
            user_code: None,
            instructions: None,
            error_code: None,
            error_message: None,
        };
        let (cancel, cancelled) = oneshot::channel();
        assert!(
            !manager
                .register_attempt(replacement, ProviderAuthAttemptRuntime::new(cancel))
                .await
        );
        assert!(cancelled.await.is_ok());
    }
}
