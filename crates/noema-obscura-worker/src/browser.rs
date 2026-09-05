use crate::WebBrowseError;
use crate::protocol::{
    BrowseCommand, BrowseHistoryAction, BrowseInteractionAction, BrowseInteractiveElement,
    BrowseResponse, BrowseScreenshot, BrowseSnapshot, BrowseWaitUntil, MAX_INTERACTIVE_ELEMENTS,
    MAX_SNAPSHOT_CHARS,
};
use crate::public_url::validate_public_url as validate_public_url_with_dns;
use crate::snapshot::{
    RawSubmissionContext, map_resulting_url_error, map_url_error, public_display_url,
    submission_context, truncate_chars,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use obscura_browser::{BrowserContext, Page, WaitUntil};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
const POST_NAVIGATION_SETTLE_MS: u64 = 250;
const SCREENSHOT_RESOURCE_TIMEOUT_MS: u64 = 1_000;
const MAX_SCREENSHOT_BYTES: usize = 900_000;
#[cfg(not(test))]
const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(test)]
const NAVIGATION_TIMEOUT: Duration = Duration::from_millis(150);

pub(crate) struct WorkerState {
    page: Page,
    revision: u64,
}

impl WorkerState {
    pub(crate) fn new(generation: u64) -> Self {
        let context = Arc::new(BrowserContext::with_storage_and_network(
            format!("noema-{generation}"),
            None,
            true,
            None,
            None,
            false,
        ));
        let mut page = Page::new(format!("page-{generation}"), context);
        page.set_navigation_timeout(NAVIGATION_TIMEOUT);
        Self { page, revision: 0 }
    }

    pub(crate) async fn execute(
        &mut self,
        command: BrowseCommand,
    ) -> Result<BrowseResponse, WebBrowseError> {
        if !matches!(&command, BrowseCommand::Open(_) | BrowseCommand::Close) {
            self.validate_resulting_url().await?;
        }
        match command {
            BrowseCommand::Open(request) => {
                self.navigate(&request.url, request.wait_until).await?;
                self.snapshot(crate::protocol::DEFAULT_SNAPSHOT_CHARS).await
            }
            BrowseCommand::Snapshot { max_chars } => self.snapshot(max_chars).await,
            BrowseCommand::Interact(request) => {
                self.require_revision(request.snapshot_revision)?;
                let main_document_status = self
                    .interact(request.reference, request.action, request.value)
                    .await?;
                self.validate_resulting_url().await?;
                let mut response = self
                    .snapshot(crate::protocol::DEFAULT_SNAPSHOT_CHARS)
                    .await?;
                if main_document_status.is_some_and(|status| status >= 500) {
                    response.state = "outcome_uncertain".to_string();
                }
                Ok(response)
            }
            BrowseCommand::Wait(request) => {
                self.wait(request.text, request.reference, request.timeout_ms)
                    .await?;
                self.validate_resulting_url().await?;
                self.snapshot(crate::protocol::DEFAULT_SNAPSHOT_CHARS).await
            }
            BrowseCommand::History(request) => {
                self.require_revision(request.snapshot_revision)?;
                self.history(request.action).await?;
                self.snapshot(crate::protocol::DEFAULT_SNAPSHOT_CHARS).await
            }
            BrowseCommand::Close => Ok(BrowseResponse {
                provider: crate::OBSCURA_BROWSER_PROVIDER_ID.to_string(),
                state: "closed".to_string(),
                snapshot: None,
                screenshot: None,
            }),
        }
    }

    async fn navigate(
        &mut self,
        raw_url: &str,
        wait: BrowseWaitUntil,
    ) -> Result<(), WebBrowseError> {
        let checked = validate_public_url_with_dns(raw_url)
            .await
            .map_err(map_url_error)?;
        self.page
            .navigate_with_wait(checked.as_str(), map_wait(wait))
            .await
            .map_err(|error| {
                WebBrowseError::NavigationFailed.with_provider_detail(
                    crate::OBSCURA_BROWSER_PROVIDER_ID,
                    "navigation",
                    error.to_string(),
                )
            })?;
        self.page.settle(POST_NAVIGATION_SETTLE_MS).await;
        self.validate_resulting_url().await
    }

    async fn validate_resulting_url(&self) -> Result<(), WebBrowseError> {
        validate_public_url_with_dns(&self.page.url_string())
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

    async fn snapshot(&mut self, max_chars: usize) -> Result<BrowseResponse, WebBrowseError> {
        let raw = self
            .page
            .evaluate_with_timeout(SNAPSHOT_SCRIPT, Duration::from_millis(500));
        let raw = parse_snapshot(raw)?;
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
                submission: submission_context(element.submission),
            })
            .collect();
        let screenshot = self.screenshot().await;
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
            screenshot,
        })
    }

    async fn screenshot(&mut self) -> Option<BrowseScreenshot> {
        let viewport = self.page.viewport;
        let _ = self
            .page
            .prepare_screenshot_resources(SCREENSHOT_RESOURCE_TIMEOUT_MS)
            .await;
        let png = self.page.screenshot(viewport)?;
        if png.len() > MAX_SCREENSHOT_BYTES {
            return None;
        }
        Some(BrowseScreenshot {
            media_type: "image/png".to_string(),
            data: STANDARD.encode(png),
            width: viewport.0 as u32,
            height: viewport.1 as u32,
        })
    }

    async fn interact(
        &mut self,
        reference: String,
        action: BrowseInteractionAction,
        value: Option<String>,
    ) -> Result<Option<u16>, WebBrowseError> {
        if action == BrowseInteractionAction::UploadFile {
            return Err(WebBrowseError::Unavailable.with_provider_detail(
                crate::OBSCURA_BROWSER_PROVIDER_ID,
                "interact",
                "file upload requires a later browser provider",
            ));
        }
        let main_document_request = self
            .page
            .network_events
            .iter()
            .rev()
            .find(|event| event.resource_type == "Document")
            .map(|event| event.request_id.clone());
        let script = interaction_script(&reference, action, value.as_deref());
        let result = self
            .page
            .evaluate_with_timeout(&script, Duration::from_millis(500));
        if result != Value::Bool(true) {
            return Err(WebBrowseError::ElementNotFound);
        }
        self.page.settle(250).await;
        if self
            .page
            .process_pending_navigation()
            .await
            .map_err(|_| WebBrowseError::OutcomeUncertain)?
        {
            self.page.settle(POST_NAVIGATION_SETTLE_MS).await;
        }
        Ok(self
            .page
            .network_events
            .iter()
            .rev()
            .find(|event| {
                event.resource_type == "Document"
                    && Some(&event.request_id) != main_document_request.as_ref()
            })
            .map(|event| event.status))
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

fn parse_snapshot(raw: Value) -> Result<RawSnapshot, WebBrowseError> {
    serde_json::from_value(raw).map_err(|_| WebBrowseError::NavigationFailed)
}

#[derive(Deserialize)]
struct RawElement {
    #[serde(rename = "ref")]
    reference: String,
    role: String,
    name: String,
    href: Option<String>,
    disabled: bool,
    #[serde(default)]
    submission: Option<RawSubmissionContext>,
}

const SNAPSHOT_SCRIPT: &str = r#"(() => {
  const body = document.body ? document.body.cloneNode(true) : null;
  if (body) body.querySelectorAll('noscript,script,style,template').forEach(element => element.remove());
  const selectors = 'a[href],button,input,textarea,select,[role="button"],[tabindex]';
  const nodes = Array.from(document.querySelectorAll(selectors));
  const elements = nodes.map((element, index) => {
    const ref = `e${index + 1}`;
    element.dataset.noemaRef = ref;
    const name = element.getAttribute('aria-label') || element.innerText || element.value || element.getAttribute('placeholder') || '';
    const form = element.form || (element.closest && element.closest('form'));
    let submission = null;
    if (form && !['button', 'reset'].includes(String(element.type || '').toLowerCase())) {
      const fields = [];
      let omittedControlCount = 0;
      for (const control of Array.from(form.elements)) {
        const fieldName = String(control.name || '');
        if (!fieldName || control.disabled) continue;
        const type = String(control.type || '').toLowerCase();
        if (['hidden', 'password', 'file'].includes(type)) { omittedControlCount += 1; continue; }
        if (['button', 'reset'].includes(type)) continue;
        if ((type === 'checkbox' || type === 'radio') && !control.checked) continue;
        if ((control.tagName === 'BUTTON' || type === 'submit' || type === 'image') && control !== element) continue;
        const values = control.tagName === 'SELECT' && control.multiple
          ? Array.from(control.selectedOptions).map(option => option.value)
          : [control.value];
        for (const value of values) fields.push({name:fieldName, value:String(value || '')});
      }
      submission = {
        destination: String(element.formAction || form.action || window.location.href),
        method: String(element.formMethod || form.method || 'get'),
        fields: fields.slice(0, 64),
        omitted_control_count: omittedControlCount,
        truncated: fields.length > 64,
      };
    }
    return {ref, role: element.getAttribute('role') || element.tagName.toLowerCase(), name: String(name).trim(), href: element.href || null, disabled: Boolean(element.disabled || element.getAttribute('aria-disabled') === 'true'), submission};
  });
  return {text: String(body ? body.innerText : ''), elements};
})()"#;

fn interaction_script(
    reference: &str,
    action: BrowseInteractionAction,
    value: Option<&str>,
) -> String {
    let operation = match action {
        BrowseInteractionAction::Click => {
            r#"
            element.focus();
            emit(new MouseEvent('mousedown', {bubbles:true,cancelable:true,button:0,buttons:1}));
            emit(new MouseEvent('mouseup', {bubbles:true,cancelable:true,button:0,buttons:0}));
            const tag = element.tagName;
            const type = String(element.getAttribute('type') || '').toLowerCase();
            const checkable = tag === 'INPUT' && (type === 'checkbox' || type === 'radio');
            const oldChecked = checkable ? Boolean(element.checked) : false;
            if (checkable) element.checked = type === 'radio' ? true : !oldChecked;
            const accepted = emit(new MouseEvent('click', {bubbles:true,cancelable:true,button:0,buttons:0}));
            if (!accepted && checkable) element.checked = oldChecked;
            if (accepted && checkable && element.checked !== oldChecked) {
              emit(new Event('input', {bubbles:true}));
              emit(new Event('change', {bubbles:true}));
            } else if (accepted) {
              const link = element.closest ? element.closest('a[href]') : null;
              if (link) {
                const href = link.getAttribute('href');
                if (href && !href.startsWith('#') && !href.startsWith('javascript:')) location.assign(href);
              } else if ((tag === 'BUTTON' && type !== 'button' && type !== 'reset') ||
                         (tag === 'INPUT' && (type === 'submit' || type === 'image'))) {
                const form = element.form || (element.closest && element.closest('form'));
                if (form) form.requestSubmit ? form.requestSubmit(element) : form.submit();
              }
            }
            "#
        }
        BrowseInteractionAction::Fill => {
            r#"
            element.focus();
            setValue(value);
            emit(new Event('input', {bubbles:true}));
            emit(new Event('change', {bubbles:true}));
            "#
        }
        BrowseInteractionAction::Type => {
            r#"
            element.focus();
            for (const character of value) {
              const accepted = emit(new KeyboardEvent('keydown', {key:character,bubbles:true,cancelable:true}));
              if (accepted) {
                setValue(String(element.value || '') + character);
                emit(new Event('input', {bubbles:true}));
              }
              emit(new KeyboardEvent('keyup', {key:character,bubbles:true}));
            }
            "#
        }
        BrowseInteractionAction::PressKey => {
            r#"
            element.focus();
            const namedKeys = {enter:'Enter',backspace:'Backspace',arrowup:'ArrowUp',arrowdown:'ArrowDown',arrowleft:'ArrowLeft',arrowright:'ArrowRight',escape:'Escape',tab:'Tab',delete:'Delete',home:'Home',end:'End',pageup:'PageUp',pagedown:'PageDown'};
            const key = namedKeys[value.toLowerCase()] || value;
            const accepted = emit(new KeyboardEvent('keydown', {key,bubbles:true,cancelable:true}));
            if (accepted && key === 'Backspace') {
              setValue(String(element.value || '').slice(0, -1));
              emit(new Event('input', {bubbles:true}));
            } else if (accepted && key === 'Enter') {
              if (element.tagName === 'TEXTAREA') {
                setValue(String(element.value || '') + '\n');
                emit(new Event('input', {bubbles:true}));
              } else {
                const form = element.form || (element.closest && element.closest('form'));
                if (form) form.requestSubmit ? form.requestSubmit() : form.submit();
              }
            }
            emit(new KeyboardEvent('keyup', {key,bubbles:true}));
            "#
        }
        BrowseInteractionAction::SelectOption => {
            r#"
            element.focus();
            setValue(value);
            emit(new Event('input', {bubbles:true}));
            emit(new Event('change', {bubbles:true}));
            "#
        }
        BrowseInteractionAction::UploadFile => unreachable!("file upload uses its own script"),
    };
    format!(
        "(() => {{ const ref = {}; const value = {}; const element = Array.from(document.querySelectorAll('[data-noema-ref]')).find(item => item.dataset.noemaRef === ref); if (!element) return false; const emit = event => element.dispatchEvent(globalThis.__obscura_markTrusted(event)); const setValue = next => {{ globalThis.__obscura_setFieldValue(element, 'value', next); if (element.setSelectionRange) element.setSelectionRange(String(next).length, String(next).length); }}; {operation} return true; }})()",
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "browser-fixtures")]
    use crate::protocol::BrowseInteractionRequest;
    #[cfg(feature = "browser-fixtures")]
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn invalid_document_snapshot_is_not_a_worker_failure() {
        assert!(matches!(
            parse_snapshot(Value::Null),
            Err(WebBrowseError::NavigationFailed)
        ));
    }

    #[cfg(feature = "browser-fixtures")]
    #[tokio::test(flavor = "current_thread")]
    async fn embedded_obscura_snapshots_and_emits_trusted_interactions() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(b"HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Fixture</title><body><noscript>JavaScript is disabled</noscript><label>Name<input aria-label='Name'></label><label>File<input type='file' aria-label='File'></label><label>Role<select aria-label='Role'><option value='engineer'>Engineer</option><option value='manager'>Manager</option></select></label><button type='button' disabled>Save</button><div role='button' aria-label='Activate' tabindex='0'>Activate</div><form action='https://example.com/confirmed' method='post'><input name='amount' value='125.00'><input type='hidden' name='csrf' value='hidden-secret'><input type='password' name='pin' value='password-secret'><button name='confirm' value='yes'>Submit form</button></form><script>const input=document.querySelector('input');const select=document.querySelector('select');const button=document.querySelector('button');const activate=document.querySelector('[role=button]');setTimeout(()=>button.disabled=false,1);input.addEventListener('input',event=>input.setAttribute('data-input-trusted',String(event.isTrusted)));input.addEventListener('change',event=>input.setAttribute('data-change-trusted',String(event.isTrusted)));input.addEventListener('keydown',event=>input.setAttribute('data-keydown-trusted',String(event.isTrusted)));input.addEventListener('keyup',event=>input.setAttribute('data-keyup-trusted',String(event.isTrusted)));select.addEventListener('input',event=>select.setAttribute('data-input-trusted',String(event.isTrusted)));select.addEventListener('change',event=>select.setAttribute('data-change-trusted',String(event.isTrusted)));button.addEventListener('click',event=>button.setAttribute('data-click-trusted',String(event.isTrusted)));activate.addEventListener('keydown',event=>{activate.setAttribute('data-key',event.key);if(event.key==='Enter')activate.setAttribute('data-enter','true')});</script></body>")
                .await
                .expect("write fixture");
            drop(stream);
            let (mut stream, _) = listener.accept().await.expect("accept navigation");
            let _ = stream.read(&mut request).await;
            stream
                .write_all(b"HTTP/1.0 502 Bad Gateway\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!doctype html><title>Confirmed</title>")
                .await
                .expect("write navigation");
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
        state.page.settle(POST_NAVIGATION_SETTLE_MS).await;

        let response = state.snapshot(2_000).await.expect("snapshot");
        let screenshot = response.screenshot.as_ref().expect("rendered screenshot");
        assert_eq!(screenshot.media_type, "image/png");
        assert_eq!((screenshot.width, screenshot.height), (1280, 720));
        assert!(
            STANDARD
                .decode(&screenshot.data)
                .is_ok_and(|png| png.starts_with(b"\x89PNG\r\n\x1a\n"))
        );
        let snapshot = response.snapshot.expect("open snapshot");
        assert!(!snapshot.text.contains("JavaScript is disabled"));
        let input = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Name")
            .expect("input reference");
        let button = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Save")
            .expect("button reference");
        let file = snapshot
            .elements
            .iter()
            .find(|element| element.name == "File")
            .expect("file reference");
        assert!(!button.disabled);
        let select = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Role")
            .expect("select reference");
        let activate = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Activate")
            .expect("role button reference");
        let submission = snapshot
            .elements
            .iter()
            .find(|element| element.name == "Submit form")
            .and_then(|element| element.submission.as_ref())
            .expect("submission context");
        assert_eq!(submission.method, "POST");
        assert_eq!(submission.omitted_control_count, 2);
        assert_eq!(submission.fields[0].value, "125.00");
        assert!(
            !serde_json::to_string(submission)
                .expect("serialize submission")
                .contains("secret")
        );
        let request = BrowseInteractionRequest {
            snapshot_revision: snapshot.snapshot_revision,
            reference: input.reference.clone(),
            action: BrowseInteractionAction::Fill,
            value: Some("Ada".to_string()),
            artifact_id: None,
            artifact_version_id: None,
            upload: None,
        };
        state
            .require_revision(request.snapshot_revision)
            .expect("current revision");
        state
            .interact(request.reference, request.action, request.value)
            .await
            .expect("fill input");
        assert_eq!(
            state.page.evaluate_with_timeout(
                "(() => { const input = document.querySelector('input'); return {value:input.value,inputTrusted:input.getAttribute('data-input-trusted'),changeTrusted:input.getAttribute('data-change-trusted')}; })()",
                Duration::from_millis(500),
            ),
            json!({"value":"Ada","inputTrusted":"true","changeTrusted":"true"})
        );
        assert!(matches!(
            state
                .interact(file.reference.clone(), BrowseInteractionAction::UploadFile, None)
                .await,
            Err(WebBrowseError::ProviderFailure { kind, .. })
                if *kind == WebBrowseError::Unavailable
        ));
        state
            .interact(
                input.reference.clone(),
                BrowseInteractionAction::Type,
                Some("!".to_string()),
            )
            .await
            .expect("type input");
        state
            .interact(
                input.reference.clone(),
                BrowseInteractionAction::PressKey,
                Some("Backspace".to_string()),
            )
            .await
            .expect("press input key");
        assert_eq!(
            state.page.evaluate_with_timeout(
                "(() => { const input = document.querySelector('input'); return {value:input.value,keydownTrusted:input.getAttribute('data-keydown-trusted'),keyupTrusted:input.getAttribute('data-keyup-trusted')}; })()",
                Duration::from_millis(500),
            ),
            json!({"value":"Ada","keydownTrusted":"true","keyupTrusted":"true"})
        );
        state
            .interact(
                select.reference.clone(),
                BrowseInteractionAction::SelectOption,
                Some("manager".to_string()),
            )
            .await
            .expect("select option");
        assert_eq!(
            state.page.evaluate_with_timeout(
                "(() => { const select = document.querySelector('select'); return {value:select.value,inputTrusted:select.getAttribute('data-input-trusted'),changeTrusted:select.getAttribute('data-change-trusted')}; })()",
                Duration::from_millis(500),
            ),
            json!({"value":"manager","inputTrusted":"true","changeTrusted":"true"})
        );
        state
            .interact(
                button.reference.clone(),
                BrowseInteractionAction::Click,
                None,
            )
            .await
            .expect("click button");
        assert_eq!(
            state.page.evaluate_with_timeout(
                "document.querySelector('button').getAttribute('data-click-trusted')",
                Duration::from_millis(500),
            ),
            json!("true")
        );
        state
            .interact(
                activate.reference.clone(),
                BrowseInteractionAction::PressKey,
                Some("ARROWRIGHT".to_string()),
            )
            .await
            .expect("press named key");
        assert_eq!(
            state.page.evaluate_with_timeout(
                "document.querySelector('[role=button]').getAttribute('data-key')",
                Duration::from_millis(500),
            ),
            json!("ArrowRight")
        );
        state
            .page
            .evaluate_with_timeout(
                "document.querySelector('[role=button]').addEventListener('keydown',event=>{if(event.key==='Enter')location.assign('/confirmed')})",
                Duration::from_millis(500),
            );
        let status = state
            .interact(
                activate.reference.clone(),
                BrowseInteractionAction::PressKey,
                Some("ENTER".to_string()),
            )
            .await
            .expect("activate navigation");
        assert_eq!(status, Some(502));
        assert_eq!(state.page.title, "Confirmed");
        state.snapshot(2_000).await.expect("updated snapshot");
        assert_eq!(
            state.require_revision(snapshot.snapshot_revision),
            Err(WebBrowseError::StaleSnapshot)
        );
        assert!(crate::url_policy::validate_public_url("https://user@example.com").is_err());
        assert!(crate::url_policy::validate_public_url("http://127.0.0.1").is_err());
        state.page.url =
            Some(url::Url::parse("http://127.0.0.1").expect("invalid policy fixture URL"));
        assert_eq!(
            state.validate_resulting_url().await,
            Err(WebBrowseError::BlockedTarget)
        );
    }

    #[test]
    fn browser_workers_enable_obscura_stealth() {
        let state = WorkerState::new(1);
        assert!(state.page.context.stealth);
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
}
