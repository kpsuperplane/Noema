//! Codex device-code authentication adapter.

use std::{
    io::ErrorKind,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    time,
};

use crate::{
    ProviderAuthMethod, ProviderError,
    provider_auth::{
        CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
        ProviderAuthManager, ensure_codex_account_home,
    },
};

const CODEX_PROVIDER: &str = "codex";
const LOGIN_FAILED_MESSAGE: &str = "codex login failed";
const LOGIN_INSTRUCTIONS: &str = "Complete the login in your browser.";
const INITIAL_PROGRESS_TIMEOUT: Duration = Duration::from_secs(2);

static NEXT_ATTEMPT_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) async fn start_codex_device_auth(
    manager: ProviderAuthManager,
    request: CodexDeviceAuthRequest,
) -> Result<ProviderAuthAttemptView, ProviderError> {
    ensure_codex_account_home(&request.account_home).map_err(|_| {
        ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: LOGIN_FAILED_MESSAGE.to_string(),
        }
    })?;

    let mut command = Command::new(request.codex_command.trim());
    command
        .arg("login")
        .arg("--device-auth")
        .env("CODEX_HOME", &request.account_home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);

    let mut child = command.spawn().map_err(|source| match source.kind() {
        ErrorKind::NotFound => ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: "codex command not found".to_string(),
        },
        _ => ProviderError::ProviderUnavailable {
            provider: CODEX_PROVIDER.to_string(),
            message: LOGIN_FAILED_MESSAGE.to_string(),
        },
    })?;

    let stdout = child.stdout.take().ok_or_else(codex_login_unavailable)?;
    let stderr = child.stderr.take().ok_or_else(codex_login_unavailable)?;

    let attempt = ProviderAuthAttemptView {
        attempt_id: next_attempt_id(),
        provider_kind: CODEX_PROVIDER.to_string(),
        provider_account_id: request.provider_account_id,
        method: ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::Starting,
        verification_url: None,
        user_code: None,
        instructions: None,
        error_code: None,
        error_message: None,
    };
    manager.upsert_attempt(attempt.clone()).await;

    let attempt_id = attempt.attempt_id.clone();
    let task_manager = manager.clone();
    tokio::spawn(async move {
        let stdout_lines = read_device_auth_lines(stdout, task_manager.clone(), attempt_id.clone());
        let stderr_lines = read_device_auth_lines(stderr, task_manager.clone(), attempt_id.clone());
        let (_, _, wait_result) = tokio::join!(stdout_lines, stderr_lines, child.wait());

        match wait_result {
            Ok(status) if status.success() => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Completed,
                    None,
                    None,
                )
                .await;
            }
            Ok(_) => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Failed,
                    Some("codex_login_failed"),
                    Some(LOGIN_FAILED_MESSAGE),
                )
                .await;
            }
            Err(_) => {
                mark_terminal(
                    &task_manager,
                    &attempt_id,
                    ProviderAuthAttemptStatus::Failed,
                    Some("codex_login_wait_failed"),
                    Some(LOGIN_FAILED_MESSAGE),
                )
                .await;
            }
        }
    });

    Ok(wait_for_initial_progress(&manager, &attempt.attempt_id)
        .await
        .unwrap_or(attempt))
}

async fn read_device_auth_lines<R>(reader: R, manager: ProviderAuthManager, attempt_id: String)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(info) = parse_device_auth_line(&line) {
            manager
                .update_attempt(&attempt_id, |view| {
                    if view.status != ProviderAuthAttemptStatus::Cancelled {
                        view.status = ProviderAuthAttemptStatus::WaitingForUser;
                        view.verification_url = Some(info.verification_url);
                        view.user_code = Some(info.user_code);
                        view.instructions = Some(LOGIN_INSTRUCTIONS.to_string());
                        view.error_code = None;
                        view.error_message = None;
                    }
                })
                .await;
        }
    }
}

async fn mark_terminal(
    manager: &ProviderAuthManager,
    attempt_id: &str,
    status: ProviderAuthAttemptStatus,
    error_code: Option<&str>,
    error_message: Option<&str>,
) {
    manager
        .update_attempt(attempt_id, |view| {
            if !matches!(
                view.status,
                ProviderAuthAttemptStatus::Cancelled | ProviderAuthAttemptStatus::Expired
            ) {
                view.status = status;
                view.error_code = error_code.map(str::to_string);
                view.error_message = error_message.map(str::to_string);
            }
        })
        .await;
}

fn codex_login_unavailable() -> ProviderError {
    ProviderError::ProviderUnavailable {
        provider: CODEX_PROVIDER.to_string(),
        message: LOGIN_FAILED_MESSAGE.to_string(),
    }
}

async fn wait_for_initial_progress(
    manager: &ProviderAuthManager,
    attempt_id: &str,
) -> Option<ProviderAuthAttemptView> {
    time::timeout(INITIAL_PROGRESS_TIMEOUT, async {
        loop {
            if let Ok(Some(view)) = manager.poll_attempt(attempt_id).await
                && view.status != ProviderAuthAttemptStatus::Starting
            {
                return Some(view);
            }
            time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .ok()
    .flatten()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexDeviceAuthInfo {
    verification_url: String,
    user_code: String,
}

fn parse_device_auth_line(line: &str) -> Option<CodexDeviceAuthInfo> {
    let verification_url = line.split_whitespace().find_map(|part| {
        let part = part.trim_matches(|ch: char| matches!(ch, '.' | ',' | ';' | ':' | ')'));
        (part.starts_with("https://") || part.starts_with("http://")).then(|| part.to_string())
    })?;

    let words = line.split_whitespace().collect::<Vec<_>>();
    let enter_index = words
        .iter()
        .position(|word| word.eq_ignore_ascii_case("enter"))?;
    let user_code = words
        .get(enter_index + 1)?
        .trim_matches(|ch: char| matches!(ch, '.' | ',' | ';' | ':' | ')' | '('));

    if user_code.is_empty() {
        return None;
    }

    Some(CodexDeviceAuthInfo {
        verification_url,
        user_code: user_code.to_string(),
    })
}

fn next_attempt_id() -> String {
    let counter = NEXT_ATTEMPT_ID.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    format!("provider_auth_attempt_{now}_{counter}")
}
