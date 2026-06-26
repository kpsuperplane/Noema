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

use crate::ProviderError;

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
    /// Account-specific Codex home used for portable credentials.
    pub account_home: PathBuf,
    /// Command used to invoke Codex.
    pub codex_command: String,
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
    #[ts(type = "\"oauth_device_code\" | \"secret_input\" | \"external_manual\" | \"none\"")]
    pub method: crate::ProviderAuthMethod,
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
        crate::providers::codex_auth::start_codex_device_auth(self.clone(), request).await
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

/// Prepare a Codex account home for portable file-backed credentials.
///
/// # Errors
///
/// Returns an error when the account directory or config file cannot be written.
pub fn ensure_codex_account_home(account_home: &Path) -> io::Result<()> {
    create_private_account_dir_all(account_home)?;
    let config_path = account_home.join("config.toml");
    if !config_path.exists() {
        fs::write(config_path, "cli_auth_credentials_store = \"file\"\n")?;
    }
    Ok(())
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
    use std::{fs, time::Duration};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn ensure_codex_account_home_writes_file_credential_config() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_codex_account_home(&account_home).expect("account home");

        let config = fs::read_to_string(account_home.join("config.toml")).expect("config");
        assert!(config.contains("cli_auth_credentials_store = \"file\""));
    }

    #[test]
    fn ensure_codex_account_home_preserves_existing_config() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        fs::create_dir_all(&account_home).expect("account home dir");
        let config_path = account_home.join("config.toml");
        let existing = "model = \"gpt-5\"\ncli_auth_credentials_store = \"file\"\n";
        fs::write(&config_path, existing).expect("existing config");

        ensure_codex_account_home(&account_home).expect("account home");

        let config = fs::read_to_string(config_path).expect("config");
        assert_eq!(config, existing);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_codex_account_home_sets_private_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_codex_account_home(&account_home).expect("account home");

        let mode = fs::metadata(&account_home)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[tokio::test]
    async fn auth_manager_reports_codex_device_code_progress() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nsleep 1\nexit 0\n",
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::WaitingForUser,
        )
        .await;

        assert_eq!(status.status, ProviderAuthAttemptStatus::WaitingForUser);
        assert_eq!(
            status.verification_url.as_deref(),
            Some("https://example.com/device")
        );
        assert_eq!(status.user_code.as_deref(), Some("ABCD-EFGH"));
    }

    #[tokio::test]
    async fn auth_manager_marks_successful_attempt_completed() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nexit 0\n",
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::Completed,
        )
        .await;
        assert_eq!(status.status, ProviderAuthAttemptStatus::Completed);
    }

    #[tokio::test]
    async fn auth_manager_never_exposes_raw_codex_output() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH with OPENAI_API_KEY=sk-secret at /tmp/noema/providers/codex/default/auth.json\\n'\nsleep 1\nexit 0\n",
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::WaitingForUser,
        )
        .await;

        assert_eq!(
            status.verification_url.as_deref(),
            Some("https://example.com/device")
        );
        assert_eq!(status.user_code.as_deref(), Some("ABCD-EFGH"));
        assert_eq!(
            status.instructions.as_deref(),
            Some("Complete the login in your browser.")
        );
        let view = serde_json::to_string(&status).expect("view json");
        assert!(!view.contains("sk-secret"));
        assert!(!view.contains("auth.json"));
        assert!(!view.contains("/providers/"));
        assert!(!view.contains("OPENAI_API_KEY"));
    }

    #[tokio::test]
    async fn auth_manager_does_not_treat_secret_token_as_user_code() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf 'Open https://example.com/device and enter OPENAI_API_KEY=sk-secret\\n'\nexit 0\n",
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::Completed,
        )
        .await;

        assert_eq!(status.user_code, None);
        let view = serde_json::to_string(&status).expect("view json");
        assert!(!view.contains("sk-secret"));
        assert!(!view.contains("OPENAI_API_KEY"));
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

    #[tokio::test]
    async fn auth_manager_cancel_kills_running_codex_attempt() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        let pid_path = dir.path().join("fake-codex.pid");
        fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nsleep 30\nexit 0\n",
                pid_path.display()
            ),
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");
        poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::WaitingForUser,
        )
        .await;

        let status = manager
            .cancel_attempt(&attempt.attempt_id)
            .await
            .expect("cancel")
            .expect("attempt exists");

        assert_eq!(status.status, ProviderAuthAttemptStatus::Cancelled);
        let pid = wait_for_pid(&pid_path).await;
        poll_until_process_exits(pid).await;
        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::Cancelled,
        )
        .await;
        assert_eq!(status.status, ProviderAuthAttemptStatus::Cancelled);
    }

    #[tokio::test]
    async fn auth_manager_expires_and_kills_timed_out_attempt() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        let pid_path = dir.path().join("fake-codex.pid");
        fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nsleep 30\nexit 0\n",
                pid_path.display()
            ),
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let mut request = codex_request(&dir, &fake);
        request.attempt_timeout = Some(Duration::from_secs(1));
        let attempt = manager
            .start_codex_device_code(request)
            .await
            .expect("start auth");
        let pid = wait_for_pid(&pid_path).await;

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::Expired,
        )
        .await;

        assert_eq!(status.error_code.as_deref(), Some("provider_auth_expired"));
        assert_eq!(
            status.error_message.as_deref(),
            Some("provider auth expired")
        );
        poll_until_process_exits(pid).await;
    }

    #[tokio::test]
    async fn auth_manager_nonzero_exit_uses_fixed_safe_failure() {
        let dir = TempDir::new().expect("temp dir");
        let fake = dir.path().join("fake-codex");
        fs::write(
            &fake,
            "#!/bin/sh\nprintf 'Open https://example.com/device and enter ABCD-EFGH\\n'\nprintf 'OPENAI_API_KEY=sk-secret failed at /tmp/noema/providers/codex/default/auth.json\\n' >&2\nexit 42\n",
        )
        .expect("fake codex");
        make_executable(&fake);

        let manager = ProviderAuthManager::new();
        let attempt = manager
            .start_codex_device_code(codex_request(&dir, &fake))
            .await
            .expect("start auth");

        let status = poll_until_status(
            &manager,
            &attempt.attempt_id,
            ProviderAuthAttemptStatus::Failed,
        )
        .await;

        assert_eq!(status.error_code.as_deref(), Some("codex_login_failed"));
        assert_eq!(status.error_message.as_deref(), Some("codex login failed"));
        let view = serde_json::to_string(&status).expect("view json");
        assert!(!view.contains("sk-secret"));
        assert!(!view.contains("auth.json"));
        assert!(!view.contains("/providers/"));
        assert!(!view.contains("OPENAI_API_KEY"));
        assert!(!view.contains("42"));
    }

    fn codex_request(dir: &TempDir, fake: &std::path::Path) -> CodexDeviceAuthRequest {
        CodexDeviceAuthRequest {
            provider_account_id: "provider_account:codex:default".to_string(),
            account_home: dir.path().join("providers/codex/default"),
            codex_command: fake.to_string_lossy().to_string(),
            attempt_timeout: None,
        }
    }

    async fn poll_until_status(
        manager: &ProviderAuthManager,
        attempt_id: &str,
        expected: ProviderAuthAttemptStatus,
    ) -> ProviderAuthAttemptView {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = manager
                    .poll_attempt(attempt_id)
                    .await
                    .expect("poll attempt")
                    .expect("attempt exists");
                if status.status == expected {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("status before timeout")
    }

    async fn wait_for_pid(path: &std::path::Path) -> u32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(pid) = std::fs::read_to_string(path)
                    && let Ok(pid) = pid.trim().parse::<u32>()
                {
                    return pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("pid before timeout")
    }

    async fn poll_until_process_exits(pid: u32) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if !process_is_running(pid) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("process exit before timeout");
    }

    #[cfg(unix)]
    fn process_is_running(pid: u32) -> bool {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("permissions");
    }
}
