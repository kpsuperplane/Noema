//! Provider authentication support.

use std::{collections::HashMap, fs, io, path::Path, sync::Arc};

use crate::{
    CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderError,
};
use tokio::sync::{Mutex, oneshot};

use super::codex::oauth::{self, CodexDeviceAuthOutcome, CodexDeviceAuthSession};

/// Default maximum lifetime for a provider auth attempt.
pub const DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

/// In-memory manager for short-lived provider authentication attempts.
#[derive(Clone, Default)]
pub struct ProviderAuthManager {
    attempts: Arc<Mutex<HashMap<String, ProviderAuthAttemptView>>>,
    runtimes: Arc<Mutex<HashMap<String, ProviderAuthAttemptRuntime>>>,
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a Codex device-code auth attempt.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when OAuth client setup or the initial
    /// device-code request fails.
    pub async fn start_codex_device_code(
        &self,
        request: CodexDeviceAuthRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderError> {
        let session = self.begin_codex_device_code(request).await?;
        let attempt = session.attempt.clone();
        let attempt_id = attempt.attempt_id.clone();
        let manager = self.clone();
        tokio::spawn(async move {
            let (status, error_code, error_message) = match session.completion.await {
                CodexDeviceAuthOutcome::Completed(tokens) => {
                    drop(tokens);
                    (ProviderAuthAttemptStatus::Completed, None, None)
                }
                CodexDeviceAuthOutcome::Cancelled => {
                    (ProviderAuthAttemptStatus::Cancelled, None, None)
                }
                CodexDeviceAuthOutcome::Expired => (
                    ProviderAuthAttemptStatus::Expired,
                    Some("provider_auth_expired".to_string()),
                    Some("provider auth expired".to_string()),
                ),
                CodexDeviceAuthOutcome::Failed {
                    error_code,
                    error_message,
                } => (
                    ProviderAuthAttemptStatus::Failed,
                    Some(error_code),
                    Some(error_message),
                ),
            };
            manager
                .mark_attempt_terminal(&attempt_id, status, error_code, error_message)
                .await;
            manager.remove_attempt_runtime(&attempt_id).await;
        });

        Ok(attempt)
    }

    pub(crate) async fn begin_codex_device_code(
        &self,
        request: CodexDeviceAuthRequest,
    ) -> Result<CodexDeviceAuthSession, ProviderError> {
        oauth::begin_codex_device_auth(self.clone(), request).await
    }

    /// Return a safe auth attempt view, if it is still known.
    ///
    /// # Errors
    ///
    /// This currently has no fallible backing store, but returns a result to
    /// keep the public manager API compatible with future persistence.
    pub async fn poll_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<ProviderAuthAttemptView>, ProviderError> {
        let attempts = self.attempts.lock().await;
        Ok(attempts.get(attempt_id).cloned())
    }

    /// Mark an auth attempt as cancelled, if it is still known.
    ///
    /// # Errors
    ///
    /// This currently has no fallible backing store, but returns a result to
    /// keep the public manager API compatible with future cancellation hooks.
    pub async fn cancel_attempt(
        &self,
        attempt_id: &str,
    ) -> Result<Option<ProviderAuthAttemptView>, ProviderError> {
        let status = self
            .update_attempt(attempt_id, |view| {
                if !is_terminal_status(view.status) {
                    view.status = ProviderAuthAttemptStatus::Cancelled;
                    view.error_code = None;
                    view.error_message = None;
                }
            })
            .await;

        if status.is_some()
            && let Some(runtime) = self.remove_attempt_runtime(attempt_id).await
        {
            runtime.cancel();
        }

        Ok(status)
    }

    pub(crate) async fn upsert_attempt(&self, attempt: ProviderAuthAttemptView) {
        let mut attempts = self.attempts.lock().await;
        attempts.insert(attempt.attempt_id.clone(), attempt);
    }

    pub(crate) async fn upsert_attempt_runtime(
        &self,
        attempt_id: String,
        runtime: ProviderAuthAttemptRuntime,
    ) {
        let mut runtimes = self.runtimes.lock().await;
        runtimes.insert(attempt_id, runtime);
    }

    pub(crate) async fn remove_attempt_runtime(
        &self,
        attempt_id: &str,
    ) -> Option<ProviderAuthAttemptRuntime> {
        let mut runtimes = self.runtimes.lock().await;
        runtimes.remove(attempt_id)
    }

    pub(crate) async fn update_attempt(
        &self,
        attempt_id: &str,
        update: impl FnOnce(&mut ProviderAuthAttemptView),
    ) -> Option<ProviderAuthAttemptView> {
        let mut attempts = self.attempts.lock().await;
        let attempt = attempts.get_mut(attempt_id)?;
        update(attempt);
        Some(attempt.clone())
    }

    pub(crate) async fn mark_attempt_terminal(
        &self,
        attempt_id: &str,
        status: ProviderAuthAttemptStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Option<ProviderAuthAttemptView> {
        debug_assert!(is_terminal_status(status));
        self.update_attempt(attempt_id, |view| {
            if !is_terminal_status(view.status) {
                view.status = status;
                view.error_code = error_code;
                view.error_message = error_message;
            }
        })
        .await
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

/// Prepare a provider account home for Noema-owned credentials.
///
/// # Errors
///
/// Returns an error when the account directory cannot be written.
pub fn ensure_provider_account_home(account_home: &Path) -> io::Result<()> {
    create_private_account_dir_all(account_home)
}

#[cfg(unix)]
fn create_private_account_dir_all(account_home: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut missing_dirs = Vec::new();
    let mut current = Some(account_home);
    while let Some(path) = current {
        if path.exists() {
            break;
        }
        missing_dirs.push(path.to_path_buf());
        current = path.parent();
    }

    fs::create_dir_all(account_home)?;
    for path in missing_dirs.iter().rev() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(account_home, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_account_dir_all(account_home: &Path) -> io::Result<()> {
    fs::create_dir_all(account_home)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn ensure_provider_account_home_creates_private_directory() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_provider_account_home(&account_home).expect("account home");

        assert!(account_home.is_dir());
        assert!(!account_home.join("config.toml").exists());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_provider_account_home_sets_private_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_provider_account_home(&account_home).expect("account home");

        let mode = fs::metadata(&account_home)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

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
        manager.upsert_attempt(attempt).await;

        let status = manager
            .cancel_attempt("attempt-test")
            .await
            .expect("cancel")
            .expect("attempt exists");

        assert_eq!(status.status, ProviderAuthAttemptStatus::Cancelled);
    }
}
