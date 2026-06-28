# Codex Responses API Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Noema's embedded Codex CLI/app-server chat runtime with Noema-owned Codex OAuth and direct Codex Responses API calls.

**Architecture:** Extract the existing OpenAI `/responses` HTTP logic into a shared transport, add a Codex-specific OAuth/token provider on top of it, and rewire the daemon runtime so Noema supplies conversation context from Postgres instead of depending on Codex app-server thread state. Onboarding remains provider-account based, but status checks validate Noema-owned OAuth tokens instead of bare Codex CLI credential files.

**Tech Stack:** Rust 2024, Tokio, Reqwest, Serde, SQLx/Postgres, existing Noema provider and daemon runtime modules.

---

## File Structure

- Create `crates/noema-core/src/providers/responses.rs`
  - Shared OpenAI-compatible Responses transport, response parsing, request body, HTTP status mapping, and unit tests.
- Modify `crates/noema-core/src/providers/openai.rs`
  - Delegate to `responses.rs` while preserving existing `OpenAiProvider` behavior and tests.
- Create `crates/noema-core/src/providers/codex_oauth.rs`
  - Noema-owned Codex OAuth device-code flow, token file read/write, refresh, expiry checks, and safe auth status helpers.
- Create `crates/noema-core/src/providers/codex_responses.rs`
  - `CodexResponsesProvider` implementing `ModelProvider` through the shared Responses transport and Codex OAuth token store.
- Modify `crates/noema-core/src/providers/mod.rs`
  - Export `responses`, `codex_oauth`, and `codex_responses`; retire normal exports for app-server modules once callers are gone.
- Modify `crates/noema-core/src/provider_auth.rs`
  - Replace `codex login --device-auth` request/runtime types with HTTP-backed Codex OAuth auth attempts and token-home helpers.
- Modify `crates/noema-core/src/providers/codex_auth.rs`
  - Either delete after callers move to `codex_oauth.rs`, or reduce it to a compatibility-free wrapper that forwards to the new module during the transition task.
- Modify `crates/noema-core/src/config.rs`
  - Replace CLI-shaped Codex config semantics with direct Responses config: base URL, model, timeout, refresh skew.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Replace `CodexAppServerRuntime` state with a direct `CodexResponsesProvider` runtime. Active conversations keep Noema-local model/cwd metadata only.
- Modify `crates/noema-core/src/daemon/web/mod.rs`
  - Start/poll/cancel auth attempts through `codex_oauth`; reconcile onboarding using token validity/refresh instead of file existence.
- Modify `crates/noema-core/src/daemon/server.rs`
  - Stop passing app-server command fields into web state/runtime.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Keep GraphQL shapes stable unless auth method/status fields need updated descriptions.
- Modify tests in:
  - `crates/noema-core/src/config/tests.rs`
  - `crates/noema-core/src/daemon/tests.rs`
  - `crates/noema-core/src/daemon/web/mod.rs`
  - `crates/noema-core/src/providers/openai.rs`
  - new provider module tests.
- Modify docs:
  - `docs/context/current.md`
  - `docs/superpowers/specs/2026-06-26-provider-auth-onboarding-design.md` or add a short note pointing to the migration spec.
  - Any Docker/dev docs that currently say Codex CLI is required for normal chat.

## Task 1: Extract Shared Responses Transport

**Files:**
- Create: `crates/noema-core/src/providers/responses.rs`
- Modify: `crates/noema-core/src/providers/openai.rs`
- Modify: `crates/noema-core/src/providers/mod.rs`

- [ ] **Step 1: Add the shared module export**

In `crates/noema-core/src/providers/mod.rs`, add:

```rust
/// Shared OpenAI-compatible Responses API transport.
pub mod responses;
```

- [ ] **Step 2: Create failing transport tests**

Create `crates/noema-core/src/providers/responses.rs` with these test-first definitions at the bottom. The module will not compile yet because `ResponsesTransport`, `ResponsesRequest`, and `ResponsesProviderIds` are not implemented.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderError;
    use serde_json::json;

    #[test]
    fn responses_url_appends_responses_to_base_url() {
        let transport = ResponsesTransport::new(
            reqwest::Client::new(),
            ResponsesTransportConfig {
                base_url: "https://chatgpt.com/backend-api/codex/".to_string(),
                timeout_seconds: 30,
                provider: "codex".to_string(),
            },
        )
        .expect("transport");

        assert_eq!(
            transport.responses_url(),
            "https://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn request_body_omits_absent_optional_fields() {
        let body = ResponsesRequest {
            model: "gpt-5.3-codex".to_string(),
            input: "hello".to_string(),
            instructions: None,
            max_output_tokens: None,
            temperature: None,
            store: false,
        };

        let value = serde_json::to_value(body).expect("request json");
        assert_eq!(
            value,
            json!({
                "model": "gpt-5.3-codex",
                "input": "hello",
                "store": false
            })
        );
    }

    #[test]
    fn collect_output_text_reads_message_output_text() {
        let response: ResponsesResponse = serde_json::from_value(json!({
            "id": "resp_123",
            "model": "gpt-5.3-codex",
            "output": [
                {
                    "type": "message",
                    "content": [
                        { "type": "output_text", "text": "hello" },
                        { "type": "output_text", "text": " world" }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 2,
                "output_tokens": 3,
                "total_tokens": 5
            }
        }))
        .expect("response");

        assert_eq!(collect_output_text(&response).expect("text"), "hello world");
        let usage = response.usage.expect("usage");
        assert_eq!(usage.input, 2);
        assert_eq!(usage.output, 3);
        assert_eq!(usage.total, 5);
    }

    #[test]
    fn status_mapping_classifies_auth_and_rate_limit_errors() {
        let auth = error_from_status(
            reqwest::StatusCode::UNAUTHORIZED,
            Some("req_1".to_string()),
            r#"{"error":{"message":"expired token"}}"#,
        );
        assert!(matches!(auth, ProviderError::AuthenticationFailure { .. }));

        let rate_limit = error_from_status(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            Some("req_2".to_string()),
            r#"{"error":{"message":"quota exhausted"}}"#,
        );
        assert!(matches!(rate_limit, ProviderError::RateLimit { .. }));
    }
}
```

- [ ] **Step 3: Run the focused failing tests**

Run:

```bash
cargo test -p noema-core providers::responses --no-fail-fast
```

Expected: compile failure naming missing transport/request/response items.

- [ ] **Step 4: Implement the shared transport**

Add the implementation above the tests in `crates/noema-core/src/providers/responses.rs`:

```rust
//! Shared OpenAI-compatible Responses API transport.

use crate::provider::{GenerateResponse, ProviderError, TokenUsage};
use reqwest::{StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponsesTransportConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct ResponsesTransport {
    client: reqwest::Client,
    config: ResponsesTransportConfig,
}

impl ResponsesTransport {
    pub fn new(
        client: reqwest::Client,
        mut config: ResponsesTransportConfig,
    ) -> Result<Self, ProviderError> {
        config.base_url = config.base_url.trim().trim_end_matches('/').to_string();
        config.provider = config.provider.trim().to_string();
        if config.base_url.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "responses base URL cannot be empty".to_string(),
            });
        }
        if reqwest::Url::parse(&config.base_url).is_err() {
            return Err(ProviderError::InvalidRequest {
                message: "responses base URL must be an absolute URL".to_string(),
            });
        }
        if config.provider.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "responses provider cannot be empty".to_string(),
            });
        }
        if config.timeout_seconds == 0 {
            return Err(ProviderError::InvalidRequest {
                message: "responses timeout must be greater than zero seconds".to_string(),
            });
        }

        Ok(Self { client, config })
    }

    pub fn with_default_client(config: ResponsesTransportConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;
        Self::new(client, config)
    }

    pub fn responses_url(&self) -> String {
        format!("{}/responses", self.config.base_url)
    }

    pub async fn send(
        &self,
        bearer_token: &str,
        request: ResponsesRequest,
        extra_headers: HeaderMap,
    ) -> Result<GenerateResponse, ProviderError> {
        if bearer_token.trim().is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: self.config.provider.clone(),
                credential: "access_token".to_string(),
            });
        }

        let mut builder = self
            .client
            .post(self.responses_url())
            .bearer_auth(bearer_token)
            .json(&request);

        for (name, value) in &extra_headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let request_id = request_id(response.headers());
        let body_text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;

        if !status.is_success() {
            return Err(error_from_status(status, request_id, &body_text));
        }

        let response: ResponsesResponse =
            serde_json::from_str(&body_text).map_err(|source| {
                ProviderError::MalformedResponse {
                    message: format!("failed to parse JSON: {source}"),
                }
            })?;
        let text = collect_output_text(&response)?;
        Ok(GenerateResponse {
            output: crate::provider::output_items_from_text(text)?,
            provider: self.config.provider.clone(),
            model: response.model.unwrap_or(request.model),
            response_id: response.id,
            usage: response.usage.map(Into::into),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub store: bool,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub output: Vec<ResponsesOutputItem>,
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ResponsesContent> },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(rename = "refusal")]
    Refusal { refusal: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesUsage {
    #[serde(default, rename = "input_tokens")]
    pub input: u64,
    #[serde(default, rename = "output_tokens")]
    pub output: u64,
    #[serde(default, rename = "total_tokens")]
    pub total: u64,
}

impl From<ResponsesUsage> for TokenUsage {
    fn from(value: ResponsesUsage) -> Self {
        Self {
            input_tokens: value.input,
            output_tokens: value.output,
            total_tokens: value.total,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorResponse {
    error: Option<ResponsesErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ResponsesErrorBody {
    message: Option<String>,
}

pub fn collect_output_text(response: &ResponsesResponse) -> Result<String, ProviderError> {
    let mut output = String::new();
    let mut refusals = Vec::new();

    for item in &response.output {
        let ResponsesOutputItem::Message { content } = item else {
            continue;
        };
        for content_item in content {
            match content_item {
                ResponsesContent::OutputText { text } => output.push_str(text),
                ResponsesContent::Refusal { refusal } => refusals.push(refusal.as_str()),
                ResponsesContent::Other => {}
            }
        }
    }

    if !output.is_empty() {
        return Ok(output);
    }

    if !refusals.is_empty() {
        return Err(ProviderError::ApiError {
            status: 200,
            message: refusals.join("\n"),
            request_id: response.id.clone(),
        });
    }

    Err(ProviderError::MalformedResponse {
        message: "response did not contain output_text".to_string(),
    })
}

pub fn error_from_status(
    status: StatusCode,
    request_id: Option<String>,
    body_text: &str,
) -> ProviderError {
    let message = serde_json::from_str::<ResponsesErrorResponse>(body_text)
        .ok()
        .and_then(|body| body.error)
        .and_then(|error| error.message)
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| body_text.trim().to_string())
        .if_empty_then(|| status.canonical_reason().unwrap_or("API error").to_string());

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthenticationFailure {
            message,
            request_id,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimit {
            message,
            request_id,
        },
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id,
        },
    }
}

fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

trait EmptyStringExt {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.is_empty() { fallback() } else { self }
    }
}
```

- [ ] **Step 5: Update `OpenAiProvider` to use the transport**

In `crates/noema-core/src/providers/openai.rs`, replace duplicated request/response structs and parsing with `ResponsesTransport`. Keep `OpenAiProviderConfig` and `OpenAiProvider` public API stable.

Core shape:

```rust
use crate::{
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, ModelProvider, ProviderError,
        output_items_from_text, required_output_items_from_text,
    },
    providers::responses::{ResponsesRequest, ResponsesTransport, ResponsesTransportConfig},
};
use reqwest::header::HeaderMap;
use std::time::Duration;

#[derive(Debug)]
pub struct OpenAiProvider {
    transport: ResponsesTransport,
    config: OpenAiProviderConfig,
}

impl OpenAiProvider {
    pub fn new(config: OpenAiProviderConfig) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;
        Self::with_client(client, config)
    }

    pub fn with_client(
        client: reqwest::Client,
        config: OpenAiProviderConfig,
    ) -> Result<Self, ProviderError> {
        let config = normalize_config(config)?;
        let transport = ResponsesTransport::new(
            client,
            ResponsesTransportConfig {
                base_url: config.base_url.clone(),
                timeout_seconds: config.timeout_seconds,
                provider: "openai".to_string(),
            },
        )?;
        Ok(Self { transport, config })
    }
}
```

In `generate`, call `self.transport.send(...)`, then re-parse output according to `require_noema_response`:

```rust
let response = self
    .transport
    .send(&self.config.api_key, body, headers)
    .await?;
let text = response.assistant_text();
let output = if request.options.require_noema_response {
    required_output_items_from_text(text)?
} else {
    output_items_from_text(text)?
};
Ok(GenerateResponse { output, ..response })
```

- [ ] **Step 6: Run tests for OpenAI and shared transport**

Run:

```bash
cargo test -p noema-core providers::responses providers::openai --no-fail-fast
```

Expected: PASS.

## Task 2: Add Noema-Owned Codex Token Storage

**Files:**
- Create: `crates/noema-core/src/providers/codex_oauth.rs`
- Modify: `crates/noema-core/src/providers/mod.rs`
- Test: `crates/noema-core/src/providers/codex_oauth.rs`

- [ ] **Step 1: Export the module**

In `crates/noema-core/src/providers/mod.rs`, add:

```rust
/// Noema-owned Codex OAuth and token storage.
pub mod codex_oauth;
```

- [ ] **Step 2: Write token storage tests**

Create `crates/noema-core/src/providers/codex_oauth.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn token_file_round_trip_preserves_tokens() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("providers/codex/default/auth.json");
        let store = CodexTokenStore::new(path.clone());
        let tokens = CodexOAuthTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            last_refresh: "2026-06-27T00:00:00Z".to_string(),
            auth_mode: "chatgpt".to_string(),
            source: "device_code".to_string(),
        };

        store.write_tokens(&tokens).expect("write tokens");
        let loaded = store.read_tokens().expect("read tokens").expect("tokens");

        assert_eq!(loaded, tokens);
    }

    #[test]
    fn missing_token_file_returns_none() {
        let dir = TempDir::new().expect("temp dir");
        let store = CodexTokenStore::new(dir.path().join("missing/auth.json"));

        assert_eq!(store.read_tokens().expect("read tokens"), None);
    }

    #[cfg(unix)]
    #[test]
    fn token_file_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("providers/codex/default/auth.json");
        let store = CodexTokenStore::new(path.clone());
        store
            .write_tokens(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: "2026-06-27T00:00:00Z".to_string(),
                auth_mode: "chatgpt".to_string(),
                source: "device_code".to_string(),
            })
            .expect("write tokens");

        let dir_mode = std::fs::metadata(path.parent().expect("parent"))
            .expect("dir metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = std::fs::metadata(&path)
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }

    #[test]
    fn malformed_token_file_is_an_auth_error() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("auth.json");
        std::fs::write(&path, "not json").expect("write bad file");
        let store = CodexTokenStore::new(path);

        let error = store.read_tokens().unwrap_err();
        assert!(matches!(error, CodexOAuthError::MalformedTokenFile { .. }));
    }
}
```

- [ ] **Step 3: Run tests and verify failure**

Run:

```bash
cargo test -p noema-core providers::codex_oauth --no-fail-fast
```

Expected: compile failure for missing token store types.

- [ ] **Step 4: Implement token storage**

Add above tests:

```rust
//! Noema-owned Codex OAuth and token storage.

use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CODEX_OAUTH_ISSUER: &str = "https://auth.openai.com";
pub const CODEX_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

#[derive(Debug, Error)]
pub enum CodexOAuthError {
    #[error("codex token file IO failed: {source}")]
    Io { source: io::Error },
    #[error("codex token file was malformed: {source}")]
    MalformedTokenFile { source: serde_json::Error },
    #[error("codex auth is missing {field}")]
    MissingField { field: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexOAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub last_refresh: String,
    pub auth_mode: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct CodexTokenStore {
    path: PathBuf,
}

impl CodexTokenStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_tokens(&self) -> Result<Option<CodexOAuthTokens>, CodexOAuthError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.path).map_err(|source| CodexOAuthError::Io { source })?;
        let file: CodexTokenFile =
            serde_json::from_str(&text).map_err(|source| CodexOAuthError::MalformedTokenFile { source })?;
        file.into_tokens().map(Some)
    }

    pub fn write_tokens(&self, tokens: &CodexOAuthTokens) -> Result<(), CodexOAuthError> {
        let parent = self.path.parent().ok_or(CodexOAuthError::MissingField {
            field: "token_file_parent",
        })?;
        create_private_dir_all(parent)?;
        let file = CodexTokenFile::from_tokens(tokens);
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|source| CodexOAuthError::MalformedTokenFile { source })?;
        let tmp_path = self.path.with_extension("json.tmp");
        write_private_file(&tmp_path, &bytes)?;
        fs::rename(&tmp_path, &self.path).map_err(|source| CodexOAuthError::Io { source })?;
        set_private_file_permissions(&self.path)?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CodexTokenFile {
    auth_mode: String,
    source: String,
    last_refresh: String,
    tokens: CodexTokenFileTokens,
}

#[derive(Debug, Serialize, Deserialize)]
struct CodexTokenFileTokens {
    access_token: String,
    refresh_token: String,
}

impl CodexTokenFile {
    fn from_tokens(tokens: &CodexOAuthTokens) -> Self {
        Self {
            auth_mode: tokens.auth_mode.clone(),
            source: tokens.source.clone(),
            last_refresh: tokens.last_refresh.clone(),
            tokens: CodexTokenFileTokens {
                access_token: tokens.access_token.clone(),
                refresh_token: tokens.refresh_token.clone(),
            },
        }
    }

    fn into_tokens(self) -> Result<CodexOAuthTokens, CodexOAuthError> {
        if self.tokens.access_token.trim().is_empty() {
            return Err(CodexOAuthError::MissingField { field: "access_token" });
        }
        if self.tokens.refresh_token.trim().is_empty() {
            return Err(CodexOAuthError::MissingField { field: "refresh_token" });
        }
        Ok(CodexOAuthTokens {
            access_token: self.tokens.access_token,
            refresh_token: self.tokens.refresh_token,
            last_refresh: self.last_refresh,
            auth_mode: self.auth_mode,
            source: self.source,
        })
    }
}

fn create_private_dir_all(path: &Path) -> Result<(), CodexOAuthError> {
    fs::create_dir_all(path).map_err(|source| CodexOAuthError::Io { source })?;
    set_private_dir_permissions(path)
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<(), CodexOAuthError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|source| CodexOAuthError::Io { source })
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<(), CodexOAuthError> {
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), CodexOAuthError> {
    fs::write(path, bytes).map_err(|source| CodexOAuthError::Io { source })?;
    set_private_file_permissions(path)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), CodexOAuthError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|source| CodexOAuthError::Io { source })
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), CodexOAuthError> {
    Ok(())
}
```

- [ ] **Step 5: Run token storage tests**

Run:

```bash
cargo test -p noema-core providers::codex_oauth --no-fail-fast
```

Expected: PASS for token storage tests.

## Task 3: Implement Codex OAuth Device-Code Flow And Refresh

**Files:**
- Modify: `crates/noema-core/src/providers/codex_oauth.rs`
- Modify: `crates/noema-core/src/provider_auth.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Write OAuth flow tests with a local HTTP server helper**

In `crates/noema-core/src/providers/codex_oauth.rs`, add test helper and tests:

```rust
#[cfg(test)]
mod oauth_flow_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[tokio::test]
    async fn device_code_login_persists_tokens() {
        let server = TestAuthServer::start(vec![
            TestResponse::json(200, r#"{"user_code":"ABCD-EFGH","device_auth_id":"dev_1","interval":1}"#),
            TestResponse::json(200, r#"{"authorization_code":"auth_code","code_verifier":"verifier"}"#),
            TestResponse::json(200, r#"{"access_token":"access","refresh_token":"refresh"}"#),
        ])
        .await;
        let dir = tempfile::TempDir::new().expect("temp dir");
        let store = CodexTokenStore::new(dir.path().join("auth.json"));
        let client = reqwest::Client::new();
        let config = CodexOAuthClientConfig {
            issuer: server.url(),
            token_url: format!("{}/oauth/token", server.url()),
            client_id: "client".to_string(),
            poll_interval_seconds: 1,
            attempt_timeout_seconds: 5,
        };

        let login = CodexOAuthClient::new(client, config)
            .login_with_device_code(&store)
            .await
            .expect("login");

        assert_eq!(login.verification_url, format!("{}/codex/device", server.url()));
        assert_eq!(login.user_code, "ABCD-EFGH");
        let tokens = store.read_tokens().expect("read tokens").expect("tokens");
        assert_eq!(tokens.access_token, "access");
        assert_eq!(tokens.refresh_token, "refresh");
    }

    #[tokio::test]
    async fn refresh_replaces_access_and_refresh_tokens() {
        let server = TestAuthServer::start(vec![
            TestResponse::json(200, r#"{"access_token":"new_access","refresh_token":"new_refresh"}"#),
        ])
        .await;
        let client = CodexOAuthClient::new(
            reqwest::Client::new(),
            CodexOAuthClientConfig {
                issuer: server.url(),
                token_url: format!("{}/oauth/token", server.url()),
                client_id: "client".to_string(),
                poll_interval_seconds: 1,
                attempt_timeout_seconds: 5,
            },
        );
        let refreshed = client
            .refresh_tokens(&CodexOAuthTokens {
                access_token: "old_access".to_string(),
                refresh_token: "old_refresh".to_string(),
                last_refresh: "2026-06-27T00:00:00Z".to_string(),
                auth_mode: "chatgpt".to_string(),
                source: "device_code".to_string(),
            })
            .await
            .expect("refresh");

        assert_eq!(refreshed.access_token, "new_access");
        assert_eq!(refreshed.refresh_token, "new_refresh");
    }

    struct TestResponse {
        status: u16,
        body: String,
    }

    impl TestResponse {
        fn json(status: u16, body: &str) -> Self {
            Self { status, body: body.to_string() }
        }
    }

    struct TestAuthServer {
        addr: std::net::SocketAddr,
        _responses: Arc<Mutex<Vec<TestResponse>>>,
    }

    impl TestAuthServer {
        async fn start(responses: Vec<TestResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("addr");
            let responses = Arc::new(Mutex::new(responses));
            let task_responses = responses.clone();
            tokio::spawn(async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else { break };
                    let responses = task_responses.clone();
                    tokio::spawn(async move {
                        let mut buf = [0_u8; 4096];
                        let _ = stream.read(&mut buf).await;
                        let response = responses.lock().expect("responses").remove(0);
                        let body = response.body;
                        let wire = format!(
                            "HTTP/1.1 {} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                            response.status,
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(wire.as_bytes()).await;
                    });
                }
            });
            Self { addr, _responses: responses }
        }

        fn url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }
}
```

- [ ] **Step 2: Run OAuth tests and verify failure**

Run:

```bash
cargo test -p noema-core providers::codex_oauth::oauth_flow_tests --no-fail-fast
```

Expected: compile failure for missing `CodexOAuthClient` and config types.

- [ ] **Step 3: Implement OAuth client**

Add to `codex_oauth.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexOAuthClientConfig {
    pub issuer: String,
    pub token_url: String,
    pub client_id: String,
    pub poll_interval_seconds: u64,
    pub attempt_timeout_seconds: u64,
}

impl Default for CodexOAuthClientConfig {
    fn default() -> Self {
        Self {
            issuer: CODEX_OAUTH_ISSUER.to_string(),
            token_url: CODEX_OAUTH_TOKEN_URL.to_string(),
            client_id: CODEX_OAUTH_CLIENT_ID.to_string(),
            poll_interval_seconds: 3,
            attempt_timeout_seconds: 15 * 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexOAuthClient {
    client: reqwest::Client,
    config: CodexOAuthClientConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDeviceLoginResult {
    pub verification_url: String,
    pub user_code: String,
    pub tokens: CodexOAuthTokens,
}

impl CodexOAuthClient {
    pub fn new(client: reqwest::Client, config: CodexOAuthClientConfig) -> Self {
        Self { client, config }
    }

    pub async fn login_with_device_code(
        &self,
        store: &CodexTokenStore,
    ) -> Result<CodexDeviceLoginResult, ProviderError> {
        let device = self.request_device_code().await?;
        let auth_code = self.poll_device_code(&device).await?;
        let tokens = self.exchange_authorization_code(&auth_code).await?;
        store
            .write_tokens(&tokens)
            .map_err(|source| ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: source.to_string(),
            })?;
        Ok(CodexDeviceLoginResult {
            verification_url: format!("{}/codex/device", self.config.issuer.trim_end_matches('/')),
            user_code: device.user_code,
            tokens,
        })
    }

    pub async fn refresh_tokens(
        &self,
        tokens: &CodexOAuthTokens,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        let response = self
            .client
            .post(&self.config.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", tokens.refresh_token.as_str()),
                ("client_id", self.config.client_id.as_str()),
            ])
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        if !status.is_success() {
            return Err(codex_oauth_status_error(status, &text));
        }
        let payload: TokenResponse =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse codex token response: {source}"),
            })?;
        let access_token = non_empty(payload.access_token, "access_token")?;
        let refresh_token = payload.refresh_token.unwrap_or_else(|| tokens.refresh_token.clone());
        Ok(CodexOAuthTokens {
            access_token,
            refresh_token,
            last_refresh: current_utc_timestamp(),
            auth_mode: "chatgpt".to_string(),
            source: "device_code".to_string(),
        })
    }

    async fn request_device_code(&self) -> Result<DeviceCodeResponse, ProviderError> {
        let url = format!(
            "{}/api/accounts/deviceauth/usercode",
            self.config.issuer.trim_end_matches('/')
        );
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({ "client_id": self.config.client_id }))
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        if !status.is_success() {
            return Err(codex_oauth_status_error(status, &text));
        }
        serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
            message: format!("failed to parse codex device-code response: {source}"),
        })
    }

    async fn poll_device_code(
        &self,
        device: &DeviceCodeResponse,
    ) -> Result<DeviceTokenResponse, ProviderError> {
        let url = format!(
            "{}/api/accounts/deviceauth/token",
            self.config.issuer.trim_end_matches('/')
        );
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(self.config.attempt_timeout_seconds);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(ProviderError::Timeout {
                    provider: "codex".to_string(),
                    operation: "codex device-code login".to_string(),
                    seconds: self.config.attempt_timeout_seconds,
                });
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                device.interval.unwrap_or(self.config.poll_interval_seconds).max(1),
            ))
            .await;
            let response = self
                .client
                .post(&url)
                .json(&serde_json::json!({
                    "device_auth_id": device.device_auth_id,
                    "user_code": device.user_code,
                }))
                .send()
                .await
                .map_err(|source| ProviderError::HttpFailure { source })?;
            let status = response.status();
            let text = response
                .text()
                .await
                .map_err(|source| ProviderError::HttpFailure { source })?;
            if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
                continue;
            }
            if !status.is_success() {
                return Err(codex_oauth_status_error(status, &text));
            }
            return serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse codex device polling response: {source}"),
            });
        }
    }

    async fn exchange_authorization_code(
        &self,
        auth_code: &DeviceTokenResponse,
    ) -> Result<CodexOAuthTokens, ProviderError> {
        let response = self
            .client
            .post(&self.config.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", auth_code.authorization_code.as_str()),
                ("redirect_uri", format!("{}/deviceauth/callback", self.config.issuer).as_str()),
                ("client_id", self.config.client_id.as_str()),
                ("code_verifier", auth_code.code_verifier.as_str()),
            ])
            .send()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| ProviderError::HttpFailure { source })?;
        if !status.is_success() {
            return Err(codex_oauth_status_error(status, &text));
        }
        let payload: TokenResponse =
            serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
                message: format!("failed to parse codex token exchange response: {source}"),
            })?;
        Ok(CodexOAuthTokens {
            access_token: non_empty(payload.access_token, "access_token")?,
            refresh_token: non_empty(payload.refresh_token.unwrap_or_default(), "refresh_token")?,
            last_refresh: current_utc_timestamp(),
            auth_mode: "chatgpt".to_string(),
            source: "device_code".to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_auth_id: String,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

fn non_empty(value: String, field: &'static str) -> Result<String, ProviderError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: format!("codex auth response missing {field}"),
        });
    }
    Ok(value)
}

fn codex_oauth_status_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.pointer("/error_description"))
                .or_else(|| value.pointer("/message"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| status.canonical_reason().unwrap_or("codex auth failed").to_string());
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            ProviderError::AuthenticationFailure { message, request_id: None }
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            ProviderError::RateLimit { message, request_id: None }
        }
        _ => ProviderError::ApiError {
            status: status.as_u16(),
            message,
            request_id: None,
        },
    }
}

fn current_utc_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{seconds}")
}
```

- [ ] **Step 4: Connect provider auth manager to HTTP OAuth**

In `crates/noema-core/src/provider_auth.rs`, change `CodexDeviceAuthRequest` fields:

```rust
pub struct CodexDeviceAuthRequest {
    pub provider_account_id: String,
    pub account_home: PathBuf,
    pub attempt_timeout: Option<std::time::Duration>,
}
```

Update `ProviderAuthManager::start_codex_device_code` to call a new `start_codex_device_auth` function in `codex_oauth.rs` or move the manager-facing function from `codex_auth.rs` to `codex_oauth.rs`.

Manager-facing behavior:

```rust
let token_store = CodexTokenStore::new(request.account_home.join("auth.json"));
let client = CodexOAuthClient::new(reqwest::Client::new(), CodexOAuthClientConfig::default());
```

The attempt should transition:

- `Starting` immediately after insertion.
- `WaitingForUser` after device code is received.
- `Completed` after token exchange and token file write.
- `Failed` on `ProviderError`.
- `Expired` on timeout.
- `Cancelled` when manager cancellation fires.

- [ ] **Step 5: Update web auth startup call**

In `crates/noema-core/src/daemon/web/mod.rs`, update `start_codex_provider_auth_attempt` to stop passing `codex_command`:

```rust
.start_codex_device_code(CodexDeviceAuthRequest {
    provider_account_id: account.provider_account_id.clone(),
    account_home: paths.provider_account_home(&account.provider_kind, &account.account_key),
    attempt_timeout: None,
})
```

Remove `codex_command` from `WebState` only after server construction is updated in Task 6.

- [ ] **Step 6: Run auth tests**

Run:

```bash
cargo test -p noema-core provider_auth providers::codex_oauth daemon::web::tests::provider_auth --no-fail-fast
```

Expected: PASS after updating tests that previously faked a `codex` binary to instead fake HTTP responses or exercise manager state directly.

## Task 4: Add `CodexResponsesProvider`

**Files:**
- Create: `crates/noema-core/src/providers/codex_responses.rs`
- Modify: `crates/noema-core/src/providers/mod.rs`
- Test: `crates/noema-core/src/providers/codex_responses.rs`

- [ ] **Step 1: Export the provider**

In `crates/noema-core/src/providers/mod.rs`, add:

```rust
/// Provider adapter for Codex OAuth over the Codex Responses API.
pub mod codex_responses;
```

- [ ] **Step 2: Write provider tests**

Create `codex_responses.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        provider::{GenerateInput, GenerateOptions, GenerateRequest, GenerateOutputItem, ModelProvider},
        providers::codex_oauth::{CodexOAuthTokens, CodexTokenStore},
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn missing_token_file_is_missing_credentials() {
        let dir = TempDir::new().expect("temp dir");
        let provider = CodexResponsesProvider::with_client(
            reqwest::Client::new(),
            CodexResponsesProviderConfig {
                base_url: "http://127.0.0.1:9".to_string(),
                default_model: Some("gpt-5.3-codex".to_string()),
                timeout_seconds: 30,
                refresh_skew_seconds: 120,
                token_store: CodexTokenStore::new(dir.path().join("auth.json")),
            },
        )
        .expect("provider");

        let error = provider
            .generate(GenerateRequest {
                model: None,
                input: GenerateInput::Text("hello".to_string()),
                instructions: None,
                options: GenerateOptions::default(),
            })
            .await
            .unwrap_err();

        assert!(matches!(error, crate::provider::ProviderError::MissingCredentials { .. }));
    }

    #[test]
    fn config_requires_model_from_request_or_default() {
        let dir = TempDir::new().expect("temp dir");
        let provider = CodexResponsesProvider::with_client(
            reqwest::Client::new(),
            CodexResponsesProviderConfig {
                base_url: "https://chatgpt.com/backend-api/codex".to_string(),
                default_model: None,
                timeout_seconds: 30,
                refresh_skew_seconds: 120,
                token_store: CodexTokenStore::new(dir.path().join("auth.json")),
            },
        )
        .expect("provider");

        let error = provider.model_for_request(None).unwrap_err();
        assert!(matches!(error, crate::provider::ProviderError::InvalidRequest { .. }));
    }
}
```

- [ ] **Step 3: Run provider tests and verify failure**

Run:

```bash
cargo test -p noema-core providers::codex_responses --no-fail-fast
```

Expected: compile failure for missing provider types.

- [ ] **Step 4: Implement the provider**

Add:

```rust
//! Codex OAuth provider backed by the Codex Responses API.

use crate::{
    provider::{
        GenerateInput, GenerateRequest, GenerateResponse, ModelProvider, ProviderError,
        output_items_from_text, required_output_items_from_text,
    },
    providers::{
        codex_oauth::{
            CodexOAuthClient, CodexOAuthClientConfig, CodexTokenStore, DEFAULT_CODEX_BASE_URL,
        },
        responses::{ResponsesRequest, ResponsesTransport, ResponsesTransportConfig},
    },
};
use reqwest::header::HeaderMap;

#[derive(Debug, Clone)]
pub struct CodexResponsesProviderConfig {
    pub base_url: String,
    pub default_model: Option<String>,
    pub timeout_seconds: u64,
    pub refresh_skew_seconds: u64,
    pub token_store: CodexTokenStore,
}

#[derive(Debug)]
pub struct CodexResponsesProvider {
    transport: ResponsesTransport,
    oauth: CodexOAuthClient,
    config: CodexResponsesProviderConfig,
}

impl CodexResponsesProvider {
    pub fn new(config: CodexResponsesProviderConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|source| ProviderError::HttpFailure { source })?;
        Self::with_client(client, config)
    }

    pub fn with_client(
        client: reqwest::Client,
        mut config: CodexResponsesProviderConfig,
    ) -> Result<Self, ProviderError> {
        if config.base_url.trim().is_empty() {
            config.base_url = DEFAULT_CODEX_BASE_URL.to_string();
        }
        let transport = ResponsesTransport::new(
            client.clone(),
            ResponsesTransportConfig {
                base_url: config.base_url.clone(),
                timeout_seconds: config.timeout_seconds,
                provider: "codex".to_string(),
            },
        )?;
        let oauth = CodexOAuthClient::new(client, CodexOAuthClientConfig::default());
        Ok(Self { transport, oauth, config })
    }

    fn model_for_request(&self, model: Option<String>) -> Result<String, ProviderError> {
        let model = model.or_else(|| self.config.default_model.clone()).unwrap_or_default();
        let model = model.trim().to_string();
        if model.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "codex model cannot be empty".to_string(),
            });
        }
        Ok(model)
    }

    async fn access_token(&self) -> Result<String, ProviderError> {
        let Some(tokens) = self.config.token_store.read_tokens().map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: "codex".to_string(),
                message: source.to_string(),
            }
        })? else {
            return Err(ProviderError::MissingCredentials {
                provider: "codex".to_string(),
                credential: "access_token".to_string(),
            });
        };

        if token_needs_refresh(&tokens.access_token, self.config.refresh_skew_seconds) {
            let refreshed = self.oauth.refresh_tokens(&tokens).await?;
            self.config.token_store.write_tokens(&refreshed).map_err(|source| {
                ProviderError::ProviderUnavailable {
                    provider: "codex".to_string(),
                    message: source.to_string(),
                }
            })?;
            return Ok(refreshed.access_token);
        }

        Ok(tokens.access_token)
    }
}

impl ModelProvider for CodexResponsesProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let GenerateInput::Text(input) = request.input;
        if input.trim().is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "input cannot be empty".to_string(),
            });
        }
        let model = self.model_for_request(request.model)?;
        let body = ResponsesRequest {
            model,
            input,
            instructions: request
                .instructions
                .filter(|instructions| !instructions.trim().is_empty()),
            max_output_tokens: request.options.max_output_tokens,
            temperature: request.options.temperature,
            store: false,
        };
        let response = self
            .transport
            .send(&self.access_token().await?, body, HeaderMap::new())
            .await?;
        let text = response.assistant_text();
        let output = if request.options.require_noema_response {
            required_output_items_from_text(text)?
        } else {
            output_items_from_text(text)?
        };
        Ok(GenerateResponse { output, ..response })
    }
}

fn token_needs_refresh(access_token: &str, _refresh_skew_seconds: u64) -> bool {
    access_token.trim().is_empty()
}
```

This `token_needs_refresh` implementation is intentionally conservative for the first migration: empty tokens refresh or fail immediately, and non-empty tokens rely on a retry-after-401 refresh path implemented in `access_token`. Do not decode JWT expiry in this migration unless tests expose a concrete stale-token failure.

- [ ] **Step 5: Run provider tests**

Run:

```bash
cargo test -p noema-core providers::codex_responses --no-fail-fast
```

Expected: PASS.

## Task 5: Update Codex Configuration

**Files:**
- Modify: `crates/noema-core/src/config.rs`
- Modify: `crates/noema-core/src/config/tests.rs`
- Modify: `crates/noema-core/src/home.rs`

- [ ] **Step 1: Write config tests for direct Codex fields**

In `crates/noema-core/src/config/tests.rs`, update or add:

```rust
#[test]
fn codex_provider_defaults_to_direct_responses_config() {
    let resolved = Config::load_from_source(
        None,
        CliOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("config");

    let ProviderConfig::Codex(codex) = resolved.provider else {
        panic!("expected codex config");
    };
    assert_eq!(codex.base_url, "https://chatgpt.com/backend-api/codex");
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);
    assert_eq!(codex.refresh_skew_seconds, 120);
}
```

Remove assertions that Codex defaults include `command`, `sandbox`, `ephemeral`,
`ignore_rules`, `ignore_user_config`, or `codex_home`.

- [ ] **Step 2: Run config tests and verify failure**

Run:

```bash
cargo test -p noema-core config::tests::codex_provider_defaults_to_direct_responses_config --no-fail-fast
```

Expected: compile failure or assertion failure until config structs are updated.

- [ ] **Step 3: Replace `CodexProviderConfig` fields**

Move `CodexProviderConfig` out of `providers/codex.rs` if that file is deleted, or redefine it in a config/provider module:

```rust
pub const DEFAULT_CODEX_TIMEOUT_SECONDS: u64 = 300;
pub const DEFAULT_CODEX_REFRESH_SKEW_SECONDS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderConfig {
    pub base_url: String,
    pub default_model: Option<String>,
    pub timeout_seconds: u64,
    pub refresh_skew_seconds: u64,
}
```

In `RawCodexConfig`, keep:

```rust
struct RawCodexConfig {
    model: Option<String>,
    base_url: String,
    timeout_seconds: u64,
    refresh_skew_seconds: u64,
}
```

In `resolve_codex_config`, return:

```rust
Ok(CodexProviderConfig {
    base_url: non_empty_option(Some(self.codex.base_url.as_str()))
        .unwrap_or("https://chatgpt.com/backend-api/codex")
        .to_string(),
    default_model: non_empty_option(self.model.as_deref())
        .or_else(|| non_empty_option(self.codex.model.as_deref()))
        .map(ToString::to_string),
    timeout_seconds: require_positive(
        self.codex.timeout_seconds,
        "NOEMA_CODEX__TIMEOUT_SECONDS",
    )?,
    refresh_skew_seconds: require_positive(
        self.codex.refresh_skew_seconds,
        "NOEMA_CODEX__REFRESH_SKEW_SECONDS",
    )?,
})
```

- [ ] **Step 4: Update default config file creation**

In `crates/noema-core/src/home.rs`, remove `codex.command` from generated config and include:

```yaml
codex:
  base_url: https://chatgpt.com/backend-api/codex
```

- [ ] **Step 5: Run config tests**

Run:

```bash
cargo test -p noema-core config::tests --no-fail-fast
```

Expected: PASS after test expectations are updated.

## Task 6: Rewire Daemon Runtime To Direct Responses

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write a runtime unit test for no provider thread state**

In `crates/noema-core/src/daemon/runtime.rs` tests, replace `codex_config_for_provider_account_uses_account_home` with:

```rust
#[test]
fn codex_token_store_path_uses_provider_account_home() {
    let account_home = std::path::PathBuf::from("/noema/providers/codex/default");
    let token_path = codex_token_store_path(&account_home);

    assert_eq!(
        token_path,
        std::path::PathBuf::from("/noema/providers/codex/default/auth.json")
    );
}
```

- [ ] **Step 2: Run the test and verify failure**

Run:

```bash
cargo test -p noema-core daemon::runtime::tests::codex_token_store_path_uses_provider_account_home --no-fail-fast
```

Expected: compile failure for missing helper.

- [ ] **Step 3: Replace app-server runtime fields**

In `runtime.rs`, replace imports:

```rust
providers::{
    codex::CodexProviderConfig,
    codex_responses::{CodexResponsesProvider, CodexResponsesProviderConfig},
    codex_oauth::CodexTokenStore,
},
provider::{GenerateRequest, GenerateOptions, ModelProvider},
```

Add:

```rust
fn codex_token_store_path(account_home: &std::path::Path) -> std::path::PathBuf {
    account_home.join("auth.json")
}
```

Change `ActiveConversation` provider field from `CodexAppServerConversation` to:

```rust
#[derive(Debug, Clone)]
struct ActiveProviderConversation {
    model: Option<String>,
}
```

Change runtime actor fields:

```rust
struct CodexRuntimeActor {
    provider: CodexResponsesProvider,
    memory_extraction_worker: MemoryExtractionWorkerHandle,
    memory_repository: PostgresMemoryRepository,
    conversations: HashMap<String, ActiveConversation>,
}
```

- [ ] **Step 4: Build direct provider in `CodexRuntimeHandle::spawn`**

Replace account-home application with token store setup:

```rust
let paths = crate::NoemaPaths::from_process_env()?;
let account_home = paths.provider_account_home("codex", "default");
crate::provider_auth::ensure_provider_account_home(&account_home)?;
let token_store = CodexTokenStore::new(codex_token_store_path(&account_home));
let provider_config = CodexResponsesProviderConfig {
    base_url: codex_config.base_url.clone(),
    default_model: codex_config.default_model.clone(),
    timeout_seconds: codex_config.timeout_seconds,
    refresh_skew_seconds: codex_config.refresh_skew_seconds,
    token_store,
};
```

Pass `provider_config` to actor and memory worker.

- [ ] **Step 5: Replace `start_conversation` provider start**

In `start_conversation`, remove `self.runtime.start_conversation(...)`. Insert:

```rust
ActiveConversation {
    provider: ActiveProviderConversation { model },
    cwd,
    next_turn_index: 1,
}
```

In `start_primary_conversation`, do the same and compute `next_turn_index` from the repository as today.

- [ ] **Step 6: Replace provider turn call with `GenerateRequest`**

In `turn`, replace:

```rust
self.runtime.turn_structured(&conversation.provider, input.clone(), structured_instructions).await
```

with:

```rust
self.provider
    .generate(GenerateRequest {
        model: conversation.provider.model.clone(),
        input: crate::provider::GenerateInput::Text(input.clone()),
        instructions: Some(structured_instructions),
        options: GenerateOptions {
            max_output_tokens: None,
            temperature: None,
            require_noema_response: true,
        },
    })
    .await
```

- [ ] **Step 7: Replace continuation and memory extraction provider calls**

For local tool continuation, build the same `GenerateRequest` with:

```rust
input: crate::provider::GenerateInput::Text(continuation_input.to_string()),
instructions: Some(continuation_instructions),
options: GenerateOptions {
    max_output_tokens: None,
    temperature: None,
    require_noema_response: true,
},
```

For memory extraction worker, replace its `CodexAppServerRuntime` with `CodexResponsesProvider`. The extraction call should use:

```rust
GenerateRequest {
    model: None,
    input: crate::provider::GenerateInput::Text(prompt),
    instructions: None,
    options: GenerateOptions::default(),
}
```

- [ ] **Step 8: Update server state**

In `crates/noema-core/src/daemon/server.rs`, remove `codex_command` extraction and pass only runtime, repository, auth manager, and paths into `WebState::new`.

- [ ] **Step 9: Run daemon runtime tests**

Run:

```bash
cargo test -p noema-core daemon::runtime daemon::tests --no-fail-fast
```

Expected: existing fake app-server tests fail until rewritten to fake `ModelProvider` or to use mocked Responses provider injection. Rewrite tests so they validate Noema persistence and transcript behavior without JSON-RPC app-server scripts.

## Task 7: Update Onboarding Reconciliation

**Files:**
- Modify: `crates/noema-core/src/provider_auth.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Rename account home helper**

In `provider_auth.rs`, replace `ensure_codex_account_home` with provider-neutral:

```rust
pub fn ensure_provider_account_home(account_home: &Path) -> io::Result<()> {
    create_private_account_dir_all(account_home)
}
```

Remove writing `cli_auth_credentials_store = "file"` because the directory is no longer `CODEX_HOME`.

- [ ] **Step 2: Update reconciliation test**

Replace `onboarding_reconciles_existing_codex_file_credentials` with:

```rust
#[tokio::test]
async fn onboarding_does_not_trust_bare_auth_json_file() {
    let store = RecordingProviderAccountStatusStore::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let paths = crate::NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut account = test_provider_account();
    account.status = crate::ProviderAccountStatus::Unauthenticated;
    let account_home = paths.provider_account_home(&account.provider_kind, &account.account_key);
    std::fs::create_dir_all(&account_home).expect("account home");
    std::fs::write(account_home.join("auth.json"), "{}").expect("empty auth");

    let reconciled = reconcile_onboarding_provider_account(&store, &paths, Some(account))
        .await
        .expect("reconcile")
        .expect("account");

    assert_eq!(reconciled.status, crate::ProviderAccountStatus::Unauthenticated);
}
```

- [ ] **Step 3: Add valid token reconciliation test**

Add:

```rust
#[tokio::test]
async fn onboarding_reconciles_valid_noema_codex_tokens() {
    let store = RecordingProviderAccountStatusStore::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let paths = crate::NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut account = test_provider_account();
    account.status = crate::ProviderAccountStatus::Unauthenticated;
    let account_home = paths.provider_account_home(&account.provider_kind, &account.account_key);
    let token_store = crate::providers::codex_oauth::CodexTokenStore::new(account_home.join("auth.json"));
    token_store
        .write_tokens(&crate::providers::codex_oauth::CodexOAuthTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            last_refresh: "2026-06-27T00:00:00Z".to_string(),
            auth_mode: "chatgpt".to_string(),
            source: "device_code".to_string(),
        })
        .expect("tokens");

    let reconciled = reconcile_onboarding_provider_account(&store, &paths, Some(account))
        .await
        .expect("reconcile")
        .expect("account");

    assert_eq!(reconciled.status, crate::ProviderAccountStatus::Authenticated);
}
```

- [ ] **Step 4: Implement token-aware reconciliation**

In `daemon/web/mod.rs`, replace `codex_account_home_has_file_credentials` with:

```rust
fn codex_account_home_has_noema_tokens(
    paths: &crate::NoemaPaths,
    account: &crate::ProviderAccountRecord,
) -> bool {
    if account.provider_kind != "codex" {
        return false;
    }
    let token_store = crate::providers::codex_oauth::CodexTokenStore::new(
        paths
            .provider_account_home(&account.provider_kind, &account.account_key)
            .join("auth.json"),
    );
    matches!(token_store.read_tokens(), Ok(Some(_)))
}
```

Update `reconcile_onboarding_provider_account` to call this helper.

- [ ] **Step 5: Run web onboarding tests**

Run:

```bash
cargo test -p noema-core daemon::web::tests::onboarding --no-fail-fast
```

Expected: PASS.

## Task 8: Remove App-Server Primary Runtime Code And Stale Tests

**Files:**
- Modify: `crates/noema-core/src/providers/mod.rs`
- Delete or stop using: `crates/noema-core/src/providers/codex_app_server.rs`
- Delete or stop using: `crates/noema-core/src/providers/codex.rs`
- Delete or stop using: `crates/noema-core/src/providers/codex_auth.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Remove app-server module exports**

After all references are gone, update `providers/mod.rs` so it exports only active provider modules:

```rust
pub mod codex_oauth;
pub mod codex_responses;
pub mod openai;
pub mod responses;
```

- [ ] **Step 2: Search for stale app-server references**

Run:

```bash
rg -n "codex_app_server|CodexAppServer|codex exec|codex login|codex.command|CODEX_HOME|ignore_rules|ephemeral|sandbox" crates docs -S
```

Expected: only historical docs/specs mention old app-server behavior. Runtime source should have no active app-server references.

- [ ] **Step 3: Delete obsolete provider files if no references remain**

Delete these files with `apply_patch` delete hunks:

```text
crates/noema-core/src/providers/codex_app_server.rs
crates/noema-core/src/providers/codex.rs
crates/noema-core/src/providers/codex_auth.rs
```

Keep a file only if another active module still needs a type from it; move that type first so no obsolete runtime module remains.

- [ ] **Step 4: Rewrite daemon tests away from fake shell scripts**

In `daemon/tests.rs`, remove fake `codex app-server` shell scripts. Replace them with a fake provider seam in `runtime.rs`:

```rust
#[cfg(test)]
#[derive(Debug, Clone)]
struct FakeGenerateProvider {
    response_text: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}
```

The fake should implement a local trait introduced for runtime orchestration:

```rust
#[allow(async_fn_in_trait)]
trait GenerateProvider: Send {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError>;
}
```

Implement this trait for `CodexResponsesProvider` and for `FakeGenerateProvider` in tests.

- [ ] **Step 5: Run stale reference search again**

Run:

```bash
rg -n "codex_app_server|CodexAppServer|codex exec|codex login|codex.command|CODEX_HOME|ignore_rules|ephemeral|sandbox" crates/noema-core/src -S
```

Expected: no active runtime matches.

## Task 9: Update Docs And Current Context

**Files:**
- Modify: `docs/context/current.md`
- Modify: `docs/superpowers/specs/2026-06-26-provider-auth-onboarding-design.md`
- Modify other docs found by search.

- [ ] **Step 1: Find stale docs**

Run:

```bash
rg -n "Codex CLI|codex app-server|CODEX_HOME|codex login|providers/codex/default|codex command not found" docs crates -S
```

Expected: list of docs/source comments that still describe the old runtime.

- [ ] **Step 2: Update current context**

In `docs/context/current.md`, replace:

```markdown
- Codex-backed chat through the daemon.
```

with:

```markdown
- Codex-backed chat through direct Noema-owned Codex OAuth and Responses API calls.
```

Add a settled decision:

```markdown
- Codex provider account homes store Noema-owned OAuth token state for direct
  Codex Responses API calls; they are not used as `CODEX_HOME` directories for
  embedded Codex CLI runtimes.
```

- [ ] **Step 3: Update onboarding design note**

At the top of `docs/superpowers/specs/2026-06-26-provider-auth-onboarding-design.md`, add:

```markdown
> Superseded for Codex runtime/auth details by
> `docs/superpowers/specs/2026-06-27-codex-responses-api-migration-design.md`.
> The provider-account/onboarding shape remains useful, but Codex no longer
> authenticates through `codex login --device-auth` or runs with `CODEX_HOME`.
```

- [ ] **Step 4: Update Docker/dev docs**

For any docs saying Docker must include Codex CLI for chat, replace with:

```markdown
Normal Noema chat no longer requires the Codex CLI. Codex authentication is
performed by the daemon through Noema-owned OAuth device-code flow.
```

- [ ] **Step 5: Run docs search**

Run:

```bash
rg -n "codex app-server|CODEX_HOME|codex login --device-auth|codex command not found" docs crates/noema-core/src -S
```

Expected: only historical migration/spec references remain.

## Task 10: Full Validation

**Files:**
- All modified files.

- [ ] **Step 1: Format check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 2: Workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 3: Clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. If tests that bind local sockets fail with sandbox `PermissionDenied`, rerun the same command with socket permissions and record the distinction in the implementation summary.

- [ ] **Step 5: Final status and diff check**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: `git diff --check` PASS. Status should show only intended files plus any pre-existing unrelated dirty worktree files.

- [ ] **Step 6: Commit checkpoint only when explicitly requested**

The repo instructions say commit and push only when explicitly requested. If the user has explicitly requested a commit for this implementation, run:

```bash
git add <approved-files>
git diff --cached --stat
git diff --cached --name-status
git commit -m "Replace Codex app-server with direct Responses API"
```

Expected: commit succeeds and includes only the approved migration scope.

## Self-Review Checklist

- Spec coverage:
  - Direct Codex OAuth: Task 3.
  - Noema-owned token files: Task 2.
  - Shared Responses transport: Task 1.
  - Codex-specific provider: Task 4.
  - App-server primary runtime replacement: Task 6 and Task 8.
  - Onboarding token validation: Task 7.
  - Docs and current context: Task 9.
  - Validation: Task 10.
- Placeholder scan:
  - The plan contains no unfinished markers or intentionally unspecified implementation steps.
- Type consistency:
  - `CodexOAuthTokens`, `CodexTokenStore`, `CodexOAuthClient`, `ResponsesTransport`, and `CodexResponsesProvider` are introduced before subsequent tasks reference them.
