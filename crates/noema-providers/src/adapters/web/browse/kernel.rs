use crate::{WebBrowseError, WebBrowseOwner, web::KERNEL_BROWSER_PROVIDER_ID};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use noema_capabilities::web::{
    browse::{
        BrowseCommand, BrowseHistoryAction, BrowseInteractionAction, BrowseResponse,
        BrowseScreenshot, BrowseSnapshot, BrowseWaitUntil, MAX_INTERACTIVE_ELEMENTS,
        MAX_SNAPSHOT_CHARS,
    },
    url_policy::validate_public_url,
};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

use crate::web::public_url::validate_public_url as validate_public_url_with_dns;

const KERNEL_API_BASE_URL: &str = "https://api.onkernel.com";
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_RESPONSE_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_SCREENSHOT_BYTES: usize = 900_000;
const POST_NAVIGATION_SETTLE_MS: u64 = 250;
#[cfg(not(test))]
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

/// Hosted Kernel browser backend.
pub(crate) struct KernelBrowseBackend {
    inner: Arc<BackendInner>,
}

struct BackendInner {
    client: Client,
    api_key: String,
    base_url: String,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    changed: Notify,
    next_generation: AtomicU64,
    shutdown: AtomicBool,
    capacity: Arc<Semaphore>,
}

struct Session {
    generation: u64,
    remote_id: String,
    deadline: Mutex<Instant>,
    revision: AtomicU64,
    command: Mutex<()>,
    _capacity: OwnedSemaphorePermit,
}

#[derive(Serialize)]
struct CreateBrowserRequest {
    headless: bool,
    stealth: bool,
    timeout_seconds: u64,
}

#[derive(Deserialize)]
struct CreateBrowserResponse {
    session_id: String,
}

#[derive(Serialize)]
struct ExecutePlaywrightRequest {
    code: String,
    timeout_sec: u64,
}

#[derive(Deserialize)]
struct ExecutePlaywrightResponse {
    success: bool,
    #[serde(default)]
    result: Option<Value>,
}

#[derive(Deserialize)]
struct CommandResult {
    ok: bool,
    #[serde(default)]
    snapshot: Option<RawSnapshot>,
    #[serde(default)]
    element_found: Option<bool>,
    #[serde(default)]
    history_available: Option<bool>,
}

#[derive(Deserialize)]
struct RawSnapshot {
    url: String,
    title: String,
    text: String,
    elements: Vec<RawElement>,
    #[serde(default)]
    screenshot: Option<String>,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}

#[derive(Deserialize)]
struct RawElement {
    #[serde(rename = "ref")]
    reference: String,
    role: String,
    name: String,
    href: Option<String>,
    disabled: bool,
}

impl KernelBrowseBackend {
    /// Construct a Kernel browser backend with the production API host.
    pub(crate) fn new(api_key: String, max_sessions: usize) -> Self {
        Self::with_base_url(api_key, max_sessions, KERNEL_API_BASE_URL.to_string())
    }

    fn with_base_url(api_key: String, max_sessions: usize, base_url: String) -> Self {
        let inner = Arc::new(BackendInner {
            client: Client::new(),
            api_key,
            base_url,
            sessions: Mutex::new(HashMap::new()),
            changed: Notify::new(),
            next_generation: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            capacity: Arc::new(Semaphore::new(max_sessions.clamp(1, 8))),
        });
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(expire_sessions(Arc::downgrade(&inner)));
        }
        Self { inner }
    }

    pub(crate) async fn execute(
        &self,
        owner: &WebBrowseOwner,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        match command {
            BrowseCommand::Open(request) => self.open(owner, request).await,
            BrowseCommand::Close => self.close(owner).await,
            command => self.execute_active(owner, command).await,
        }
    }

    async fn open(
        &self,
        owner: &WebBrowseOwner,
        request: noema_capabilities::web::browse::BrowseNavigationRequest,
    ) -> Result<BrowseResponse, WebBrowseError> {
        let checked = validate_public_url_with_dns(&request.url)
            .await
            .map_err(map_url_error)?;
        self.expire_now().await;

        let owner_key = owner.as_str().to_string();
        let (session, created) = {
            let mut sessions = self.inner.sessions.lock().await;
            if let Some(session) = sessions.get(&owner_key) {
                (session.clone(), false)
            } else {
                let capacity = self
                    .inner
                    .capacity
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| WebBrowseError::Capacity)?;
                let remote_id = self.create_remote_browser().await?;
                let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
                let session = Arc::new(Session {
                    generation,
                    remote_id,
                    deadline: Mutex::new(Instant::now() + IDLE_TIMEOUT),
                    revision: AtomicU64::new(0),
                    command: Mutex::new(()),
                    _capacity: capacity,
                });
                sessions.insert(owner_key.clone(), session.clone());
                (session, true)
            }
        };

        let _command = session.command.lock().await;
        let result = self
            .execute_open(&session, checked.url.as_str(), request.wait_until)
            .await;
        match result {
            Ok(response) => {
                self.refresh(&owner_key, session.generation).await;
                Ok(response)
            }
            Err(error) => {
                if created || should_remove(&error) {
                    self.remove(&owner_key, session.generation).await;
                }
                Err(error)
            }
        }
    }

    async fn execute_active(
        &self,
        owner: &WebBrowseOwner,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        let owner_key = owner.as_str().to_string();
        let session = self.session(&owner_key).await?;
        let _command = session.command.lock().await;
        let result = self.execute_active_command(&session, command).await;

        match result {
            Ok(response) => {
                self.refresh(&owner_key, session.generation).await;
                Ok(response)
            }
            Err(error) => {
                if should_remove(&error) {
                    self.remove(&owner_key, session.generation).await;
                }
                Err(error)
            }
        }
    }

    async fn execute_active_command(
        &self,
        session: &Session,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        match command {
            BrowseCommand::Snapshot { max_chars } => {
                let value = self
                    .execute_playwright(
                        &session.remote_id,
                        with_request_guard(SNAPSHOT_SCRIPT),
                        WebBrowseError::Unavailable,
                    )
                    .await?;
                self.command_snapshot(session, value, max_chars).await
            }
            BrowseCommand::Interact(request) => {
                Self::require_revision(session, request.snapshot_revision)?;
                let value = self
                    .execute_playwright(
                        &session.remote_id,
                        interaction_script(&request.reference, request.action, request.value),
                        WebBrowseError::OutcomeUncertain,
                    )
                    .await?;
                let result = parse_command_result(value, WebBrowseError::OutcomeUncertain)?;
                if !result.ok {
                    return if result.element_found == Some(false) {
                        Err(WebBrowseError::ElementNotFound)
                    } else {
                        Err(WebBrowseError::OutcomeUncertain)
                    };
                }
                let snapshot = result.snapshot.ok_or(WebBrowseError::OutcomeUncertain)?;
                self.finish_snapshot(session, snapshot, MAX_DEFAULT_SNAPSHOT_CHARS)
                    .await
            }
            BrowseCommand::Wait(request) => {
                let value = self
                    .execute_playwright(
                        &session.remote_id,
                        wait_script(
                            request.text.as_deref(),
                            request.reference.as_deref(),
                            request.timeout_ms,
                        ),
                        WebBrowseError::Timeout,
                    )
                    .await?;
                let result = parse_command_result(value, WebBrowseError::Timeout)?;
                let snapshot = result.snapshot.ok_or(WebBrowseError::Timeout)?;
                self.finish_snapshot(session, snapshot, MAX_DEFAULT_SNAPSHOT_CHARS)
                    .await
            }
            BrowseCommand::History(request) => {
                Self::require_revision(session, request.snapshot_revision)?;
                let value = self
                    .execute_playwright(
                        &session.remote_id,
                        history_script(request.action),
                        WebBrowseError::OutcomeUncertain,
                    )
                    .await?;
                let result = parse_command_result(value, WebBrowseError::OutcomeUncertain)?;
                if !result.ok {
                    return if result.history_available == Some(false) {
                        Err(WebBrowseError::HistoryUnavailable)
                    } else {
                        Err(WebBrowseError::OutcomeUncertain)
                    };
                }
                let snapshot = result.snapshot.ok_or(WebBrowseError::OutcomeUncertain)?;
                self.finish_snapshot(session, snapshot, MAX_DEFAULT_SNAPSHOT_CHARS)
                    .await
            }
            BrowseCommand::Open(_) | BrowseCommand::Close => {
                unreachable!("active browser execution excludes open and close")
            }
        }
    }

    async fn execute_open(
        &self,
        session: &Session,
        url: &str,
        wait_until: BrowseWaitUntil,
    ) -> Result<BrowseResponse, WebBrowseError> {
        let value = self
            .execute_playwright(
                &session.remote_id,
                open_script(url, wait_until),
                WebBrowseError::NavigationFailed,
            )
            .await?;
        let result = parse_command_result(value, WebBrowseError::NavigationFailed)?;
        if !result.ok {
            return Err(WebBrowseError::NavigationFailed);
        }
        let snapshot = result.snapshot.ok_or(WebBrowseError::NavigationFailed)?;
        self.finish_snapshot(session, snapshot, MAX_DEFAULT_SNAPSHOT_CHARS)
            .await
    }

    async fn command_snapshot(
        &self,
        session: &Session,
        value: Value,
        max_chars: usize,
    ) -> Result<BrowseResponse, WebBrowseError> {
        let result = parse_command_result(value, WebBrowseError::Unavailable)?;
        if !result.ok {
            return Err(WebBrowseError::Unavailable);
        }
        let snapshot = result.snapshot.ok_or(WebBrowseError::Unavailable)?;
        self.finish_snapshot(session, snapshot, max_chars).await
    }

    async fn finish_snapshot(
        &self,
        session: &Session,
        raw: RawSnapshot,
        max_chars: usize,
    ) -> Result<BrowseResponse, WebBrowseError> {
        validate_public_url_with_dns(&raw.url)
            .await
            .map_err(map_resulting_url_error)?;
        let revision = session.revision.fetch_add(1, Ordering::Relaxed) + 1;
        let max_chars = max_chars.clamp(1_000, MAX_SNAPSHOT_CHARS);
        let (text, text_truncated) = truncate_chars(raw.text, max_chars);
        let element_truncated = raw.elements.len() > MAX_INTERACTIVE_ELEMENTS;
        let elements = raw
            .elements
            .into_iter()
            .take(MAX_INTERACTIVE_ELEMENTS)
            .map(
                |element| noema_capabilities::web::browse::BrowseInteractiveElement {
                    reference: element.reference,
                    role: element.role,
                    name: truncate_chars(element.name, 500).0,
                    href: element.href.and_then(public_display_url),
                    disabled: element.disabled,
                },
            )
            .collect();
        let screenshot = raw.screenshot.and_then(|data| {
            let bytes = STANDARD.decode(data).ok()?;
            if bytes.len() > MAX_SCREENSHOT_BYTES {
                return None;
            }
            Some(BrowseScreenshot {
                media_type: "image/png".to_string(),
                data: STANDARD.encode(bytes),
                width: raw.width,
                height: raw.height,
            })
        });
        Ok(BrowseResponse {
            provider: KERNEL_BROWSER_PROVIDER_ID.to_string(),
            state: "open".to_string(),
            snapshot: Some(BrowseSnapshot {
                url: raw.url,
                title: truncate_chars(raw.title, 500).0,
                text,
                snapshot_revision: revision,
                elements,
                truncated: text_truncated || element_truncated,
            }),
            screenshot,
        })
    }

    async fn create_remote_browser(&self) -> Result<String, WebBrowseError> {
        let response = self
            .inner
            .client
            .post(endpoint(&self.inner.base_url, "/browsers"))
            .bearer_auth(&self.inner.api_key)
            .json(&CreateBrowserRequest {
                headless: false,
                stealth: true,
                timeout_seconds: IDLE_TIMEOUT.as_secs(),
            })
            .timeout(COMMAND_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;
        let value = response_json(response, WebBrowseError::Unavailable).await?;
        let browser: CreateBrowserResponse =
            serde_json::from_value(value).map_err(|_| WebBrowseError::Unavailable)?;
        if is_safe_session_id(&browser.session_id) {
            Ok(browser.session_id)
        } else {
            Err(WebBrowseError::Unavailable)
        }
    }

    async fn execute_playwright(
        &self,
        session_id: &str,
        code: String,
        failure: WebBrowseError,
    ) -> Result<Value, WebBrowseError> {
        let response = self
            .inner
            .client
            .post(endpoint(
                &self.inner.base_url,
                &format!("/browsers/{session_id}/playwright/execute"),
            ))
            .bearer_auth(&self.inner.api_key)
            .json(&ExecutePlaywrightRequest {
                code,
                timeout_sec: COMMAND_TIMEOUT.as_secs().max(1),
            })
            .timeout(COMMAND_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;
        let value = response_json(response, WebBrowseError::SessionNotFound).await?;
        let execution: ExecutePlaywrightResponse =
            serde_json::from_value(value).map_err(|_| failure.clone())?;
        if !execution.success {
            return Err(failure);
        }
        execution.result.ok_or(failure)
    }

    async fn session(&self, owner: &str) -> Result<Arc<Session>, WebBrowseError> {
        self.expire_now().await;
        self.inner
            .sessions
            .lock()
            .await
            .get(owner)
            .cloned()
            .ok_or(WebBrowseError::SessionNotFound)
    }

    fn require_revision(session: &Session, revision: u64) -> Result<(), WebBrowseError> {
        if session.revision.load(Ordering::Acquire) == revision {
            Ok(())
        } else {
            Err(WebBrowseError::StaleSnapshot)
        }
    }

    async fn refresh(&self, owner: &str, generation: u64) {
        let sessions = self.inner.sessions.lock().await;
        if let Some(session) = sessions.get(owner)
            && session.generation == generation
        {
            *session.deadline.lock().await = Instant::now() + IDLE_TIMEOUT;
            self.inner.changed.notify_one();
        }
    }

    async fn remove(&self, owner: &str, generation: u64) {
        let session = {
            let mut sessions = self.inner.sessions.lock().await;
            let remove = sessions
                .get(owner)
                .is_some_and(|session| session.generation == generation);
            let session = remove.then(|| sessions.remove(owner)).flatten();
            if session.is_some() {
                self.inner.changed.notify_one();
            }
            session
        };
        if let Some(session) = session {
            let _ = delete_remote(&self.inner, &session.remote_id).await;
        }
    }

    async fn expire_now(&self) {
        let expired = collect_expired(&self.inner).await;
        for session in expired {
            let _ = delete_remote(&self.inner, &session.remote_id).await;
        }
    }

    pub(crate) async fn has_session(&self, owner: &WebBrowseOwner) -> bool {
        self.expire_now().await;
        self.inner
            .sessions
            .lock()
            .await
            .contains_key(owner.as_str())
    }

    async fn close(&self, owner: &WebBrowseOwner) -> Result<BrowseResponse, WebBrowseError> {
        let session = {
            let mut sessions = self.inner.sessions.lock().await;
            let session = sessions.remove(owner.as_str());
            if session.is_some() {
                self.inner.changed.notify_one();
            }
            session
        };
        if let Some(session) = session {
            let _ = delete_remote(&self.inner, &session.remote_id).await;
        }
        Ok(BrowseResponse {
            provider: KERNEL_BROWSER_PROVIDER_ID.to_string(),
            state: "closed".to_string(),
            snapshot: None,
            screenshot: None,
        })
    }
}

impl Drop for KernelBrowseBackend {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        self.inner.changed.notify_waiters();
    }
}

async fn expire_sessions(inner: Weak<BackendInner>) {
    loop {
        let Some(inner) = inner.upgrade() else { return };
        let (expired, next_deadline, shutdown) = {
            let mut sessions = inner.sessions.lock().await;
            let now = Instant::now();
            let mut expired_owners = Vec::new();
            let mut deadlines = Vec::new();
            for (owner, session) in sessions.iter() {
                let deadline = *session.deadline.lock().await;
                if deadline <= now {
                    expired_owners.push(owner.clone());
                } else {
                    deadlines.push(deadline);
                }
            }
            let expired = expired_owners
                .into_iter()
                .filter_map(|owner| sessions.remove(&owner))
                .collect::<Vec<_>>();
            let shutdown = inner.shutdown.load(Ordering::Acquire);
            let next_deadline = deadlines.into_iter().min();
            (expired, next_deadline, shutdown)
        };
        for session in expired {
            let _ = delete_remote(&inner, &session.remote_id).await;
        }
        if shutdown {
            let remaining = {
                let mut sessions = inner.sessions.lock().await;
                sessions
                    .drain()
                    .map(|(_, session)| session)
                    .collect::<Vec<_>>()
            };
            for session in remaining {
                let _ = delete_remote(&inner, &session.remote_id).await;
            }
            return;
        }
        match next_deadline {
            Some(deadline) => {
                tokio::select! {
                    () = tokio::time::sleep_until(deadline) => {}
                    () = inner.changed.notified() => {}
                }
            }
            None => inner.changed.notified().await,
        }
    }
}

async fn collect_expired(inner: &BackendInner) -> Vec<Arc<Session>> {
    let mut sessions = inner.sessions.lock().await;
    let now = Instant::now();
    let mut expired_owners = Vec::new();
    for (owner, session) in sessions.iter() {
        if *session.deadline.lock().await <= now {
            expired_owners.push(owner.clone());
        }
    }
    let expired = expired_owners
        .into_iter()
        .filter_map(|owner| sessions.remove(&owner))
        .collect::<Vec<_>>();
    if !expired.is_empty() {
        inner.changed.notify_one();
    }
    expired
}

async fn delete_remote(inner: &BackendInner, session_id: &str) -> Result<(), WebBrowseError> {
    let response = inner
        .client
        .delete(endpoint(
            &inner.base_url,
            &format!("/browsers/{session_id}"),
        ))
        .bearer_auth(&inner.api_key)
        .timeout(COMMAND_TIMEOUT)
        .send()
        .await
        .map_err(map_request_error)?;
    if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
        Ok(())
    } else {
        Err(map_status(response.status(), WebBrowseError::Unavailable))
    }
}

async fn response_json(
    response: Response,
    not_found: WebBrowseError,
) -> Result<Value, WebBrowseError> {
    let status = response.status();
    if !status.is_success() {
        return Err(map_status(status, not_found));
    }
    if response
        .content_length()
        .is_some_and(|length| length as usize > MAX_RESPONSE_BYTES)
    {
        return Err(WebBrowseError::Unavailable);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(map_request_error)?;
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(WebBrowseError::Unavailable);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| WebBrowseError::Unavailable)
}

fn parse_command_result(
    value: Value,
    failure: WebBrowseError,
) -> Result<CommandResult, WebBrowseError> {
    serde_json::from_value(value).map_err(|_| failure)
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

fn is_safe_session_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn map_request_error(error: reqwest::Error) -> WebBrowseError {
    if error.is_timeout() {
        WebBrowseError::Timeout
    } else {
        WebBrowseError::Unavailable
    }
}

fn map_status(status: StatusCode, not_found: WebBrowseError) -> WebBrowseError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => WebBrowseError::Unauthenticated,
        StatusCode::NOT_FOUND => not_found,
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => WebBrowseError::Timeout,
        _ => WebBrowseError::Unavailable,
    }
}

fn should_remove(error: &WebBrowseError) -> bool {
    matches!(
        error,
        WebBrowseError::BlockedTarget
            | WebBrowseError::SessionNotFound
            | WebBrowseError::Unavailable
            | WebBrowseError::Unauthenticated
            | WebBrowseError::OutcomeUncertain
    )
}

fn map_url_error(error: crate::WebFetchError) -> WebBrowseError {
    match error {
        crate::WebFetchError::BlockedTarget | crate::WebFetchError::RedirectBlocked => {
            WebBrowseError::BlockedTarget
        }
        crate::WebFetchError::Timeout => WebBrowseError::Timeout,
        crate::WebFetchError::UnsupportedScheme | crate::WebFetchError::MalformedUrl => {
            WebBrowseError::InvalidUrl
        }
        _ => WebBrowseError::Unavailable,
    }
}

fn map_resulting_url_error(error: crate::WebFetchError) -> WebBrowseError {
    match map_url_error(error) {
        WebBrowseError::InvalidUrl | WebBrowseError::BlockedTarget => WebBrowseError::BlockedTarget,
        error => error,
    }
}

fn public_display_url(raw: String) -> Option<String> {
    validate_public_url(&raw).ok().map(|url| url.to_string())
}

fn truncate_chars(value: String, limit: usize) -> (String, bool) {
    let mut characters = value.char_indices();
    let Some((byte_index, _)) = characters.nth(limit) else {
        return (value, false);
    };
    (value[..byte_index].to_string(), true)
}

fn wait_until_value(wait_until: BrowseWaitUntil) -> &'static str {
    match wait_until {
        BrowseWaitUntil::Load => "load",
        BrowseWaitUntil::Domcontentloaded => "domcontentloaded",
        BrowseWaitUntil::Networkidle0 => "networkidle",
    }
}

fn with_request_guard(script: &str) -> String {
    format!("{REQUEST_GUARD}\n{script}")
}

fn open_script(url: &str, wait_until: BrowseWaitUntil) -> String {
    format!(
        "{REQUEST_GUARD}\nawait page.goto({}, {{waitUntil: {}}});\nawait page.waitForTimeout({POST_NAVIGATION_SETTLE_MS});\n{SNAPSHOT_SCRIPT}",
        json!(url),
        json!(wait_until_value(wait_until)),
    )
}

fn interaction_script(
    reference: &str,
    action: BrowseInteractionAction,
    value: Option<String>,
) -> String {
    let operation = match action {
        BrowseInteractionAction::Click => "await locator.click();",
        BrowseInteractionAction::Fill => "await locator.fill(value);",
        BrowseInteractionAction::Type => "await locator.type(value);",
        BrowseInteractionAction::PressKey => {
            "const keys = {enter:'Enter',backspace:'Backspace',arrowup:'ArrowUp',arrowdown:'ArrowDown',arrowleft:'ArrowLeft',arrowright:'ArrowRight',escape:'Escape',tab:'Tab',delete:'Delete',home:'Home',end:'End',pageup:'PageUp',pagedown:'PageDown'}; await locator.press(keys[value.toLowerCase()] || value);"
        }
        BrowseInteractionAction::SelectOption => "await locator.selectOption(value);",
    };
    format!(
        r#"{REQUEST_GUARD}
const reference = {};
const value = {};
if (!/^e\d{{1,3}}$/.test(reference)) return {{ok:false,element_found:false}};
const selector = '[data-noema-ref="' + reference.replaceAll('"', '\\"') + '"]';
const locator = page.locator(selector);
if (await locator.count() !== 1) return {{ok:false,element_found:false}};
try {{
  {operation}
  await page.waitForTimeout({POST_NAVIGATION_SETTLE_MS});
  {SNAPSHOT_SCRIPT}
}} catch (_) {{
  return {{ok:false,element_found:true}};
}}"#,
        json!(reference),
        json!(value.unwrap_or_default()),
    )
}

fn wait_script(text: Option<&str>, reference: Option<&str>, timeout_ms: u64) -> String {
    let condition = if text.is_some() {
        "await page.waitForFunction(text => Boolean(document.body && document.body.innerText.includes(text)), text, {timeout: timeoutMs});"
    } else {
        "await page.waitForFunction(reference => Array.from(document.querySelectorAll('[data-noema-ref]')).some(element => element.dataset.noemaRef === reference), reference, {timeout: timeoutMs});"
    };
    format!(
        r#"{REQUEST_GUARD}
const text = {};
const reference = {};
const timeoutMs = {};
{condition}
{SNAPSHOT_SCRIPT}"#,
        json!(text),
        json!(reference),
        timeout_ms,
    )
}

fn history_script(action: BrowseHistoryAction) -> String {
    let operation = match action {
        BrowseHistoryAction::Back => "await page.goBack({waitUntil:'load'})",
        BrowseHistoryAction::Forward => "await page.goForward({waitUntil:'load'})",
        BrowseHistoryAction::Reload => "await page.reload({waitUntil:'load'})",
    };
    format!(
        r#"{REQUEST_GUARD}
const response = await ({operation});
if (!response && {action_is_history}) return {{ok:false,history_available:false}};
await page.waitForTimeout({POST_NAVIGATION_SETTLE_MS});
{SNAPSHOT_SCRIPT}"#,
        action_is_history = !matches!(action, BrowseHistoryAction::Reload),
    )
}

const REQUEST_GUARD: &str = r#"
await context.unroute('**/*').catch(() => {});
await context.route('**/*', async route => {
  const raw = route.request().url();
  let allowed = false;
  try {
    const parsed = new URL(raw);
    if (['data:', 'blob:', 'about:', 'chrome-extension:'].includes(parsed.protocol)) allowed = true;
    if (['http:', 'https:', 'ws:', 'wss:'].includes(parsed.protocol)) {
      const host = parsed.hostname.toLowerCase().replace(/^\[|\]$/g, '').replace(/\.$/, '');
      const parts = host.split('.').map(Number);
      const ipv4 = parts.length === 4 && parts.every(Number.isInteger) && parts.every(part => part >= 0 && part <= 255);
      const privateIpv4 = ipv4 && (
        parts[0] === 0 ||
        parts[0] === 10 ||
        (parts[0] === 100 && parts[1] >= 64 && parts[1] <= 127) ||
        parts[0] === 127 ||
        (parts[0] === 169 && parts[1] === 254) ||
        (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31) ||
        (parts[0] === 192 && parts[1] === 0) ||
        (parts[0] === 192 && parts[1] === 168) ||
        (parts[0] === 198 && parts[1] >= 18 && parts[1] <= 19) ||
        parts[0] >= 224
      );
      const privateIpv6 =
        host === '::' ||
        host === '::1' ||
        host.startsWith('::ffff:') ||
        host.startsWith('100:') ||
        host.startsWith('2001:db8:') ||
        host.startsWith('2002:') ||
        host.startsWith('64:ff9b:') ||
        host.startsWith('fc') ||
        host.startsWith('fd') ||
        host.startsWith('fe8') ||
        host.startsWith('fe9') ||
        host.startsWith('fea') ||
        host.startsWith('feb') ||
        host.startsWith('fec') ||
        host.startsWith('fed') ||
        host.startsWith('fee') ||
        host.startsWith('fef') ||
        host.startsWith('ff');
      const blockedName = host === 'localhost' || host.endsWith('.localhost') || host.endsWith('.local') || host.endsWith('.internal') || host.endsWith('.test') || host.endsWith('.invalid') || host.endsWith('.example');
      allowed = !blockedName && !privateIpv4 && !(host.includes(':') && privateIpv6);
    }
  } catch (_) {}
  if (allowed) await route.continue(); else await route.abort();
});
"#;

const SNAPSHOT_SCRIPT: &str = r#"
const collectSnapshot = async () => {
  const body = document.body ? document.body.cloneNode(true) : null;
  if (body) body.querySelectorAll('noscript,script,style,template').forEach(element => element.remove());
  const selectors = 'a[href],button,input,textarea,select,[role="button"],[tabindex]';
  const nodes = Array.from(document.querySelectorAll(selectors));
  const elements = nodes.map((element, index) => {
    const ref = `e${index + 1}`;
    element.dataset.noemaRef = ref;
    const name = element.getAttribute('aria-label') || element.innerText || element.value || element.getAttribute('placeholder') || '';
    return {ref, role: element.getAttribute('role') || element.tagName.toLowerCase(), name: String(name).trim(), href: element.href || null, disabled: Boolean(element.disabled || element.getAttribute('aria-disabled') === 'true')};
  });
  let screenshot = null;
  try { screenshot = (await page.screenshot({type:'png'})).toString('base64'); } catch (_) {}
  return {url: page.url(), title: String(await page.title()), text: String(body ? body.innerText : ''), elements, screenshot, width: Number(await page.evaluate(() => window.innerWidth)) || 0, height: Number(await page.evaluate(() => window.innerHeight)) || 0};
};
return {ok:true,snapshot:await collectSnapshot()};
"#;

const MAX_DEFAULT_SNAPSHOT_CHARS: usize = noema_capabilities::web::browse::DEFAULT_SNAPSHOT_CHARS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::spawn_scripted_server;
    use noema_capabilities::web::browse::{
        BrowseCommand, BrowseNavigationRequest, BrowseWaitUntil,
    };
    use serde_json::json;

    #[test]
    fn scripts_encode_interaction_values() {
        let script = interaction_script(
            "e1",
            BrowseInteractionAction::Fill,
            Some("\"; globalThis.pwned = true; //".to_string()),
        );
        assert!(script.contains("\\\"; globalThis.pwned = true; //"));
        assert!(!script.contains("const value = \"\"; globalThis"));
    }

    #[test]
    fn request_guard_blocks_private_literal_targets() {
        assert!(REQUEST_GUARD.contains("privateIpv4"));
        assert!(REQUEST_GUARD.contains("privateIpv6"));
        assert!(REQUEST_GUARD.contains("blockedName"));
        assert!(REQUEST_GUARD.contains("route.abort"));
    }

    #[test]
    fn session_ids_are_path_safe() {
        assert!(is_safe_session_id("browser-123_abc"));
        assert!(!is_safe_session_id("browser/123"));
        assert!(!is_safe_session_id(""));
    }

    #[tokio::test]
    async fn kernel_wire_flow_reuses_session_and_maps_auth_failures() {
        let snapshot = json!({
            "url": "https://example.com/",
            "title": "Example",
            "text": "Example page",
            "elements": [],
            "screenshot": null,
            "width": 1280,
            "height": 720
        });
        let execute = json!({
            "success": true,
            "result": {"ok": true, "snapshot": snapshot}
        });
        let (base_url, requests) = spawn_scripted_server([
            (200, json!({"session_id": "browser-123"}).to_string()),
            (200, execute.to_string()),
            (204, String::new()),
        ])
        .await;
        let backend = KernelBrowseBackend::with_base_url("kernel-secret".to_string(), 1, base_url);
        let owner = WebBrowseOwner::new("conversation:test");
        let response = backend
            .execute(
                &owner,
                BrowseCommand::Open(BrowseNavigationRequest {
                    url: "https://example.com".to_string(),
                    reason: None,
                    wait_until: BrowseWaitUntil::Load,
                }),
            )
            .await
            .expect("open");
        assert_eq!(response.provider, "kernel");
        assert_eq!(response.snapshot.expect("snapshot").snapshot_revision, 1);
        backend
            .execute(&owner, BrowseCommand::Close)
            .await
            .expect("close");

        let requests = requests.await.expect("requests");
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/browsers");
        let create_body: Value =
            serde_json::from_str(&requests[0].body).expect("browser creation body");
        assert_eq!(create_body["headless"], false);
        assert_eq!(create_body["stealth"], true);
        assert_eq!(
            requests[0].headers.get("authorization"),
            Some(&"Bearer kernel-secret".to_string())
        );
        assert!(!requests[0].body.contains("kernel-secret"));
        assert_eq!(requests[1].path, "/browsers/browser-123/playwright/execute");
        assert!(requests[1].body.contains("page.goto"));
        assert_eq!(requests[2].method, "DELETE");
        assert_eq!(requests[2].path, "/browsers/browser-123");

        let (base_url, _requests) = spawn_scripted_server([(401, "{}")]).await;
        let backend = KernelBrowseBackend::with_base_url("bad-key".to_string(), 1, base_url);
        assert_eq!(
            backend
                .execute(
                    &WebBrowseOwner::new("conversation:auth"),
                    BrowseCommand::Open(BrowseNavigationRequest {
                        url: "https://example.com".to_string(),
                        reason: None,
                        wait_until: BrowseWaitUntil::Load,
                    }),
                )
                .await,
            Err(WebBrowseError::Unauthenticated)
        );
    }
}
