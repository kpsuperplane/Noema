//! Provider authentication support.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, oneshot};
use ts_rs::TS;

use super::accounts::ProviderAuthMethod;
use crate::{ProviderError, providers::codex_oauth::CodexOAuthConfig};

/// Default maximum lifetime for a provider auth attempt.
pub const DEFAULT_PROVIDER_AUTH_ATTEMPT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

/// Short-lived provider auth attempt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProviderAuthAttemptStatus {
    /// Attempt process is starting.
    Starting,
    /// Waiting for the user to complete an external auth step.
    WaitingForUser,
    /// Auth attempt completed successfully.
    Completed,
    /// Auth attempt failed.
    Failed,
    /// Auth attempt expired.
    Expired,
    /// Auth attempt was cancelled.
    Cancelled,
}

/// Request to start Codex device-code login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDeviceAuthRequest {
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Account-specific directory for Noema-owned Codex credentials.
    pub account_home: PathBuf,
    /// OAuth endpoint configuration.
    pub oauth: CodexOAuthConfig,
    /// Optional attempt timeout override. Defaults to 5 minutes.
    pub attempt_timeout: Option<std::time::Duration>,
}

/// Safe provider auth attempt state returned to the UI.
///
/// This type intentionally contains only typed state suitable for display. It
/// must not store raw provider stdout, stderr, credential paths, or redacted
/// provider output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProviderAuthAttemptView {
    /// Short-lived auth attempt id.
    pub attempt_id: String,
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Provider account auth method.
    pub method: ProviderAuthMethod,
    /// Current attempt status.
    pub status: ProviderAuthAttemptStatus,
    /// Typed verification URL parsed from provider output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub verification_url: Option<String>,
    /// Typed user code parsed from provider output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub user_code: Option<String>,
    /// Static UI-safe instruction text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instructions: Option<String>,
    /// Stable non-secret error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<String>,
    /// Fixed non-secret error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_message: Option<String>,
}

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
    /// Returns [`ProviderError`] when Codex account-home setup or process
    /// startup fails.
    pub async fn start_codex_device_code(
        &self,
        request: CodexDeviceAuthRequest,
    ) -> Result<ProviderAuthAttemptView, ProviderError> {
        crate::providers::codex_oauth::start_codex_device_auth(self.clone(), request).await
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
