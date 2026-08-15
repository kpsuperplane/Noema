use crate::{WebBrowseError, WebBrowseOwner};
pub(super) mod process;

use noema_capabilities::web::{
    browse::{
        BrowseCommand, BrowseHistoryAction, BrowseInteractionAction, BrowseInteractiveElement,
        BrowseResponse, BrowseSnapshot, BrowseWaitUntil, MAX_INTERACTIVE_ELEMENTS,
        MAX_SNAPSHOT_CHARS,
    },
    url_policy::validate_public_url,
};
use obscura_browser::{BrowserContext, Page, WaitUntil};
use process::{WorkerHandle, WorkerLaunch, WorkerRequest, spawn_worker};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore, oneshot};
use tokio::time::Instant;

use crate::adapters::web::fetch::url_policy::validate_public_web_fetch_url;

const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub(crate) struct ObscuraBrowseBackend {
    inner: Arc<BackendInner>,
}

struct BackendInner {
    sessions: Mutex<HashMap<String, Session>>,
    changed: Notify,
    next_generation: AtomicU64,
    shutdown: AtomicBool,
    capacity: Arc<Semaphore>,
    launch: WorkerLaunch,
}

struct Session {
    generation: u64,
    deadline: Instant,
    worker: WorkerHandle,
    _capacity: OwnedSemaphorePermit,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.worker.cancel();
    }
}

impl ObscuraBrowseBackend {
    pub(crate) fn new(max_sessions: usize, max_old_space_mb: usize) -> Self {
        Self::with_launch(
            max_sessions.clamp(1, 8),
            WorkerLaunch::current_executable(max_old_space_mb.clamp(256, 4_096)),
        )
    }

    fn with_launch(max_sessions: usize, launch: WorkerLaunch) -> Self {
        let inner = Arc::new(BackendInner {
            sessions: Mutex::new(HashMap::new()),
            changed: Notify::new(),
            next_generation: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            capacity: Arc::new(Semaphore::new(max_sessions)),
            launch,
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
            BrowseCommand::Open(_) => self.open(owner, command).await,
            BrowseCommand::Close => self.close(owner).await,
            _ => self.execute_active(owner, command).await,
        }
    }

    async fn open(
        &self,
        owner: &WebBrowseOwner,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        let owner_key = owner.as_str().to_string();
        let (generation, worker, created) = {
            let mut sessions = self.inner.sessions.lock().await;
            remove_expired(&mut sessions, Instant::now());
            if let Some(session) = sessions.get(&owner_key) {
                (session.generation, session.worker.clone(), false)
            } else {
                let capacity = self
                    .inner
                    .capacity
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| WebBrowseError::Capacity)?;
                let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
                let worker = spawn_worker(self.inner.launch.clone(), generation)?;
                sessions.insert(
                    owner_key.clone(),
                    Session {
                        generation,
                        deadline: Instant::now() + IDLE_TIMEOUT,
                        worker: worker.clone(),
                        _capacity: capacity,
                    },
                );
                (generation, worker, true)
            }
        };
        self.inner.changed.notify_one();
        match dispatch(&worker, command, !created).await {
            Ok(response) => {
                self.refresh(&owner_key, generation).await;
                Ok(response)
            }
            Err(error) => {
                if created
                    || matches!(
                        error,
                        WebBrowseError::BlockedTarget
                            | WebBrowseError::Unavailable
                            | WebBrowseError::OutcomeUncertain
                    )
                    || !worker.is_alive()
                {
                    self.remove(&owner_key, generation).await;
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
        let (generation, worker) = {
            let mut sessions = self.inner.sessions.lock().await;
            remove_expired(&mut sessions, Instant::now());
            let session = sessions
                .get(&owner_key)
                .ok_or(WebBrowseError::SessionNotFound)?;
            (session.generation, session.worker.clone())
        };
        let outcome_uncertain = matches!(
            command,
            BrowseCommand::Interact(_) | BrowseCommand::History(_)
        );
        let result = dispatch(&worker, command, outcome_uncertain).await;
        if result.is_ok() {
            self.refresh(&owner_key, generation).await;
        } else if matches!(
            result,
            Err(WebBrowseError::BlockedTarget
                | WebBrowseError::Unavailable
                | WebBrowseError::OutcomeUncertain)
        ) || !worker.is_alive()
        {
            self.remove(&owner_key, generation).await;
        }
        result
    }

    pub(crate) async fn has_session(&self, owner: &WebBrowseOwner) -> bool {
        let mut sessions = self.inner.sessions.lock().await;
        remove_expired(&mut sessions, Instant::now());
        sessions.contains_key(owner.as_str())
    }

    async fn close(&self, owner: &WebBrowseOwner) -> Result<BrowseResponse, WebBrowseError> {
        if self
            .inner
            .sessions
            .lock()
            .await
            .remove(owner.as_str())
            .is_some()
        {
            self.inner.changed.notify_one();
        }
        Ok(BrowseResponse {
            provider: crate::OBSCURA_BROWSER_PROVIDER_ID.to_string(),
            state: "closed".to_string(),
            snapshot: None,
        })
    }

    async fn refresh(&self, owner: &str, generation: u64) {
        let mut sessions = self.inner.sessions.lock().await;
        if let Some(session) = sessions.get_mut(owner)
            && session.generation == generation
        {
            session.deadline = Instant::now() + IDLE_TIMEOUT;
            self.inner.changed.notify_one();
        }
    }

    async fn remove(&self, owner: &str, generation: u64) {
        let mut sessions = self.inner.sessions.lock().await;
        if sessions
            .get(owner)
            .is_some_and(|session| session.generation == generation)
        {
            sessions.remove(owner);
            self.inner.changed.notify_one();
        }
    }
}

impl Drop for ObscuraBrowseBackend {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        self.inner.changed.notify_waiters();
    }
}

async fn expire_sessions(inner: Weak<BackendInner>) {
    loop {
        let Some(inner) = inner.upgrade() else { return };
        if inner.shutdown.load(Ordering::Acquire) {
            inner.sessions.lock().await.clear();
            return;
        }
        let deadline = {
            let mut sessions = inner.sessions.lock().await;
            remove_expired(&mut sessions, Instant::now());
            sessions.values().map(|session| session.deadline).min()
        };
        match deadline {
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

fn remove_expired(sessions: &mut HashMap<String, Session>, now: Instant) {
    sessions.retain(|_, session| session.deadline > now && session.worker.is_alive());
}

async fn dispatch(
    worker: &WorkerHandle,
    command: BrowseCommand,
    outcome_uncertain: bool,
) -> Result<BrowseResponse, WebBrowseError> {
    let (response, receiver) = oneshot::channel();
    worker
        .sender()
        .send(WorkerRequest { command, response })
        .await
        .map_err(|_| WebBrowseError::Unavailable)?;
    receiver.await.unwrap_or({
        Err(if outcome_uncertain {
            WebBrowseError::OutcomeUncertain
        } else {
            WebBrowseError::Unavailable
        })
    })
}

struct WorkerState {
    page: Page,
    revision: u64,
}

impl WorkerState {
    fn new(generation: u64) -> Self {
        let context = Arc::new(BrowserContext::with_storage_and_network(
            format!("noema-{generation}"),
            None,
            false,
            None,
            None,
            false,
        ));
        Self {
            page: Page::new(format!("page-{generation}"), context),
            revision: 0,
        }
    }

    async fn execute(&mut self, command: BrowseCommand) -> Result<BrowseResponse, WebBrowseError> {
        if !matches!(&command, BrowseCommand::Open(_) | BrowseCommand::Close) {
            self.validate_resulting_url().await?;
        }
        match command {
            BrowseCommand::Open(request) => {
                self.navigate(&request.url, request.wait_until).await?;
                self.snapshot(noema_capabilities::web::browse::DEFAULT_SNAPSHOT_CHARS)
            }
            BrowseCommand::Snapshot { max_chars } => self.snapshot(max_chars),
            BrowseCommand::Interact(request) => {
                self.require_revision(request.snapshot_revision)?;
                self.interact(request.reference, request.action, request.value)?;
                self.page.settle(250).await;
                self.validate_resulting_url().await?;
                self.snapshot(noema_capabilities::web::browse::DEFAULT_SNAPSHOT_CHARS)
            }
            BrowseCommand::Wait(request) => {
                self.wait(request.text, request.reference, request.timeout_ms)
                    .await?;
                self.validate_resulting_url().await?;
                self.snapshot(noema_capabilities::web::browse::DEFAULT_SNAPSHOT_CHARS)
            }
            BrowseCommand::History(request) => {
                self.require_revision(request.snapshot_revision)?;
                self.history(request.action).await?;
                self.snapshot(noema_capabilities::web::browse::DEFAULT_SNAPSHOT_CHARS)
            }
            BrowseCommand::Close => Ok(BrowseResponse {
                provider: crate::OBSCURA_BROWSER_PROVIDER_ID.to_string(),
                state: "closed".to_string(),
                snapshot: None,
            }),
        }
    }

    async fn navigate(
        &mut self,
        raw_url: &str,
        wait: BrowseWaitUntil,
    ) -> Result<(), WebBrowseError> {
        let checked = validate_public_web_fetch_url(raw_url)
            .await
            .map_err(map_url_error)?;
        self.page
            .navigate_with_wait(checked.url.as_str(), map_wait(wait))
            .await
            .map_err(|_| WebBrowseError::Unavailable)?;
        self.validate_resulting_url().await
    }

    async fn validate_resulting_url(&self) -> Result<(), WebBrowseError> {
        validate_public_web_fetch_url(&self.page.url_string())
            .await
            .map(|_| ())
            .map_err(map_resulting_url_error)
    }

    fn require_revision(&self, revision: u64) -> Result<(), WebBrowseError> {
        if revision == self.revision {
            Ok(())
        } else {
            Err(WebBrowseError::StaleSnapshot)
        }
    }

    fn snapshot(&mut self, max_chars: usize) -> Result<BrowseResponse, WebBrowseError> {
        let raw = self
            .page
            .evaluate_with_timeout(SNAPSHOT_SCRIPT, Duration::from_millis(500));
        let raw: RawSnapshot =
            serde_json::from_value(raw).map_err(|_| WebBrowseError::Unavailable)?;
        self.revision = self.revision.saturating_add(1);
        let max_chars = max_chars.clamp(1_000, MAX_SNAPSHOT_CHARS);
        let (text, text_truncated) = truncate_chars(raw.text, max_chars);
        let element_truncated = raw.elements.len() > MAX_INTERACTIVE_ELEMENTS;
        let elements = raw
            .elements
            .into_iter()
            .take(MAX_INTERACTIVE_ELEMENTS)
            .map(|element| BrowseInteractiveElement {
                reference: element.reference,
                role: element.role,
                name: truncate_chars(element.name, 500).0,
                href: element.href.and_then(public_display_url),
                disabled: element.disabled,
            })
            .collect();
        Ok(BrowseResponse {
            provider: crate::OBSCURA_BROWSER_PROVIDER_ID.to_string(),
            state: "open".to_string(),
            snapshot: Some(BrowseSnapshot {
                url: self.page.url_string(),
                title: truncate_chars(self.page.title.clone(), 500).0,
                text,
                snapshot_revision: self.revision,
                elements,
                truncated: text_truncated || element_truncated,
            }),
        })
    }

    fn interact(
        &mut self,
        reference: String,
        action: BrowseInteractionAction,
        value: Option<String>,
    ) -> Result<(), WebBrowseError> {
        let script = interaction_script(&reference, action, value.as_deref());
        let result = self
            .page
            .evaluate_with_timeout(&script, Duration::from_millis(500));
        if result == Value::Bool(true) {
            Ok(())
        } else {
            Err(WebBrowseError::ElementNotFound)
        }
    }

    async fn wait(
        &mut self,
        text: Option<String>,
        reference: Option<String>,
        timeout_ms: u64,
    ) -> Result<(), WebBrowseError> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let script = if let Some(text) = &text {
                format!(
                    "document.body && document.body.innerText.includes({})",
                    json!(text)
                )
            } else {
                format!(
                    "Array.from(document.querySelectorAll('[data-noema-ref]')).some(e => e.dataset.noemaRef === {})",
                    json!(reference.as_deref().unwrap_or_default())
                )
            };
            if self
                .page
                .evaluate_with_timeout(&script, Duration::from_millis(100))
                == Value::Bool(true)
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(WebBrowseError::Timeout);
            }
            self.page.settle(25).await;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn history(&mut self, action: BrowseHistoryAction) -> Result<(), WebBrowseError> {
        let target = match action {
            BrowseHistoryAction::Back if self.page.history_index > 0 => {
                Some(self.page.history_index - 1)
            }
            BrowseHistoryAction::Forward
                if self.page.history_index + 1 < self.page.history.len() =>
            {
                Some(self.page.history_index + 1)
            }
            BrowseHistoryAction::Reload => Some(self.page.history_index),
            _ => None,
        }
        .ok_or(WebBrowseError::HistoryUnavailable)?;
        let url = self
            .page
            .history
            .get(target)
            .cloned()
            .ok_or(WebBrowseError::HistoryUnavailable)?;
        let history = self.page.history.clone();
        self.navigate(&url, BrowseWaitUntil::Load).await?;
        self.page.history = history;
        self.page.history_index = target;
        Ok(())
    }
}

#[derive(Deserialize)]
struct RawSnapshot {
    text: String,
    elements: Vec<RawElement>,
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

const SNAPSHOT_SCRIPT: &str = r#"(() => {
  const selectors = 'a[href],button,input,textarea,select,[role="button"],[tabindex]';
  const nodes = Array.from(document.querySelectorAll(selectors));
  const elements = nodes.map((element, index) => {
    const ref = `e${index + 1}`;
    element.dataset.noemaRef = ref;
    const name = element.getAttribute('aria-label') || element.innerText || element.value || element.getAttribute('placeholder') || '';
    return {ref, role: element.getAttribute('role') || element.tagName.toLowerCase(), name: String(name).trim(), href: element.href || null, disabled: Boolean(element.disabled || element.getAttribute('aria-disabled') === 'true')};
  });
  return {text: String(document.body ? document.body.innerText : ''), elements};
})()"#;

fn interaction_script(
    reference: &str,
    action: BrowseInteractionAction,
    value: Option<&str>,
) -> String {
    let operation = match action {
        BrowseInteractionAction::Click => "element.click();",
        BrowseInteractionAction::Fill => {
            "element.value = value; element.dispatchEvent(new Event('input', {bubbles:true})); element.dispatchEvent(new Event('change', {bubbles:true}));"
        }
        BrowseInteractionAction::Type => {
            "element.value = String(element.value || '') + value; element.dispatchEvent(new Event('input', {bubbles:true}));"
        }
        BrowseInteractionAction::PressKey => {
            "element.dispatchEvent(new KeyboardEvent('keydown', {key:value,bubbles:true})); element.dispatchEvent(new KeyboardEvent('keyup', {key:value,bubbles:true}));"
        }
        BrowseInteractionAction::SelectOption => {
            "element.value = value; element.dispatchEvent(new Event('input', {bubbles:true})); element.dispatchEvent(new Event('change', {bubbles:true}));"
        }
    };
    format!(
        "(() => {{ const ref = {}; const value = {}; const element = Array.from(document.querySelectorAll('[data-noema-ref]')).find(item => item.dataset.noemaRef === ref); if (!element) return false; {operation} return true; }})()",
        json!(reference),
        json!(value.unwrap_or_default()),
    )
}

fn map_wait(wait: BrowseWaitUntil) -> WaitUntil {
    match wait {
        BrowseWaitUntil::Load => WaitUntil::Load,
        BrowseWaitUntil::Domcontentloaded => WaitUntil::DomContentLoaded,
        BrowseWaitUntil::Networkidle0 => WaitUntil::NetworkIdle0,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capabilities::web::browse::{BrowseInteractionRequest, BrowseNavigationRequest};
    #[cfg(unix)]
    use std::path::PathBuf;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test(flavor = "current_thread")]
    async fn embedded_obscura_snapshots_and_interacts_with_local_fixture() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(b"HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Fixture</title><body><label>Name<input aria-label='Name'></label><button>Save</button></body>")
                .await
                .expect("write fixture");
        });
        let context = Arc::new(BrowserContext::with_storage_and_network(
            "test".to_string(),
            None,
            false,
            None,
            None,
            true,
        ));
        let mut state = WorkerState {
            page: Page::new("test-page".to_string(), context),
            revision: 0,
        };
        state
            .page
            .navigate_with_wait(&format!("http://{address}"), WaitUntil::Load)
            .await
            .expect("load fixture");

        let snapshot = state.snapshot(2_000).expect("snapshot");
        let snapshot = snapshot.snapshot.expect("open snapshot");
        let input = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Name")
            .expect("input reference");
        let request = BrowseInteractionRequest {
            snapshot_revision: snapshot.snapshot_revision,
            reference: input.reference.clone(),
            action: BrowseInteractionAction::Fill,
            value: Some("Ada".to_string()),
        };
        state
            .require_revision(request.snapshot_revision)
            .expect("current revision");
        state
            .interact(request.reference, request.action, request.value)
            .expect("fill input");
        state.snapshot(2_000).expect("updated snapshot");
        assert_eq!(
            state.require_revision(snapshot.snapshot_revision),
            Err(WebBrowseError::StaleSnapshot)
        );
        assert!(validate_public_url("https://user@example.com").is_err());
        assert!(validate_public_url("https://example.com/#private").is_err());
        assert!(validate_public_url("http://127.0.0.1").is_err());
        state.page.url = Some(
            url::Url::parse("https://example.com/#private").expect("invalid policy fixture URL"),
        );
        assert_eq!(
            state.validate_resulting_url().await,
            Err(WebBrowseError::BlockedTarget)
        );
    }

    #[test]
    fn interaction_values_are_json_encoded_into_fixed_scripts() {
        let script = interaction_script(
            "e1",
            BrowseInteractionAction::Fill,
            Some("\"; globalThis.pwned = true; //"),
        );
        assert!(script.contains("\\\"; globalThis.pwned = true; //"));
        assert!(!script.contains("const value = \"\"; globalThis"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn concurrent_same_owner_opens_reuse_one_worker_at_capacity_one() {
        let script = r#"printf '{"version":1,"ready":true}\n'; while read -r request; do case "$request" in *one.example*) url='https://one.example/' ;; *two.example*) url='https://two.example/' ;; *invalid.example*) printf '{"version":1,"error":"invalid_url"}\n'; continue ;; *) exit 2 ;; esac; printf '{"version":1,"response":{"provider":"obscura","state":"open","snapshot":{"url":"%s","title":"","text":"","snapshot_revision":1,"elements":[],"truncated":false}}}\n' "$url"; done"#;
        let backend = ObscuraBrowseBackend::with_launch(
            1,
            WorkerLaunch::command(
                PathBuf::from("/bin/sh"),
                vec!["-c".to_string(), script.to_string()],
            ),
        );
        let owner = WebBrowseOwner::new("task:shared:1");
        let open = |url: &str| {
            backend.execute(
                &owner,
                BrowseCommand::Open(BrowseNavigationRequest {
                    url: url.to_string(),
                    reason: None,
                    wait_until: BrowseWaitUntil::Load,
                }),
            )
        };

        let (first, second) =
            tokio::join!(open("https://one.example"), open("https://two.example"));

        assert_eq!(
            first
                .expect("first open")
                .snapshot
                .expect("first snapshot")
                .url,
            "https://one.example/"
        );
        assert_eq!(
            second
                .expect("second open")
                .snapshot
                .expect("second snapshot")
                .url,
            "https://two.example/"
        );
        assert_eq!(
            open("https://invalid.example").await,
            Err(WebBrowseError::InvalidUrl)
        );
        assert!(backend.has_session(&owner).await);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_crash_is_contained_and_removes_the_session() {
        let script = r#"printf '{"version":1,"ready":true}\n'; read -r _request; printf '{"version":1,"response":{"provider":"obscura","state":"open"}}\n'; read -r _request; exit 133"#;
        let backend = ObscuraBrowseBackend::with_launch(
            1,
            WorkerLaunch::command(
                PathBuf::from("/bin/sh"),
                vec!["-c".to_string(), script.to_string()],
            ),
        );
        let owner = WebBrowseOwner::new("turn:owner");
        let request = BrowseNavigationRequest {
            url: "https://example.com".to_string(),
            reason: None,
            wait_until: BrowseWaitUntil::Load,
        };
        backend
            .execute(&owner, BrowseCommand::Open(request.clone()))
            .await
            .expect("fake worker opens");
        assert_eq!(
            backend.execute(&owner, BrowseCommand::Open(request)).await,
            Err(WebBrowseError::OutcomeUncertain)
        );
        assert!(!backend.has_session(&owner).await);
        for _ in 0..2 {
            assert_eq!(
                backend.execute(&owner, BrowseCommand::Close).await,
                Ok(BrowseResponse {
                    provider: crate::OBSCURA_BROWSER_PROVIDER_ID.to_string(),
                    state: "closed".to_string(),
                    snapshot: None,
                })
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn dropping_backend_stops_and_reaps_worker() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let pid_path = directory.path().join("worker.pid");
        let script = r#"printf '%s' "$$" > "$1"; printf '{"version":1,"ready":true}\n'; read -r _request; printf '{"version":1,"response":{"provider":"obscura","state":"open"}}\n'; while :; do sleep 1; done"#;
        let backend = ObscuraBrowseBackend::with_launch(
            1,
            WorkerLaunch::command(
                PathBuf::from("/bin/sh"),
                vec![
                    "-c".to_string(),
                    script.to_string(),
                    "noema-browser-test".to_string(),
                    pid_path.display().to_string(),
                ],
            ),
        );
        let owner = WebBrowseOwner::new("turn:cleanup");
        backend
            .execute(
                &owner,
                BrowseCommand::Open(BrowseNavigationRequest {
                    url: "https://example.com".to_string(),
                    reason: None,
                    wait_until: BrowseWaitUntil::Load,
                }),
            )
            .await
            .expect("fake worker opens");
        let pid = std::fs::read_to_string(&pid_path).expect("worker pid");
        let process_path = PathBuf::from(format!("/proc/{pid}"));
        assert!(process_path.exists());
        drop(backend);
        for _ in 0..100 {
            if !process_path.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("browser worker was not reaped");
    }
}
