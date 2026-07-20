//! MCP client sessions shared by concrete transports.

use std::{
    collections::HashSet,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use rmcp::{
    RoleClient,
    model::{CallToolRequestParams, CallToolResult, PaginatedRequestParams, Tool},
    service::{ClientInitializeError, RunningService, ServiceError},
    transport::{DynamicTransportError, streamable_http_client::StreamableHttpError},
};
use serde_json::{Map, Value, json};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::{
    McpDiagnosticEvent, McpDiagnosticHandle, McpDiagnosticKind, McpDiscoveredTool,
    McpOAuthStoredCredentials, McpSecretMaterial, McpServerRecord, McpTransportKind,
    discovered_tool_fingerprint,
    limits::{
        MAX_ANNOTATIONS_BYTES, MAX_DISCOVERED_TOOLS, MAX_PAGINATION_CURSOR_BYTES, MAX_SCHEMA_BYTES,
        MAX_TOOL_DESCRIPTION_BYTES, MAX_TOOL_NAME_BYTES, MAX_TOOL_RESULT_BYTES,
        bounded_diagnostic_text, json_limit_violation, json_object_limit_violation,
        json_within_limits,
    },
};

const SESSION_CLOSE_OPERATION: &str = "session/close";
const SESSION_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Boxed future returned by object-safe MCP transport ports.
pub type McpClientFuture<'a, T> = Pin<Box<dyn Future<Output = McpClientResult<T>> + Send + 'a>>;

/// Result returned by MCP client and transport operations.
pub type McpClientResult<T> = Result<T, McpClientError>;

/// Typed internal failure from MCP transport preparation or protocol work.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpClientError {
    /// The remote server requires authentication or additional OAuth scope.
    #[error("MCP authentication is required: {0}")]
    AuthenticationRequired(String),
    /// The server, process, or underlying transport is unavailable.
    #[error("MCP transport is unavailable: {0}")]
    Unavailable(String),
    /// The peer returned data that cannot be represented by the MCP contract.
    #[error("MCP response is malformed: {0}")]
    Malformed(String),
    /// The peer returned valid metadata beyond Noema's bounded support.
    #[error("MCP metadata is unsupported: {0}")]
    UnsupportedMetadata(String),
    /// The peer returned a valid protocol-level failure.
    #[error("MCP protocol operation failed: {0}")]
    Protocol(String),
    /// The operation exceeded its caller-provided deadline.
    #[error("MCP operation timed out: {operation}")]
    Timeout {
        /// Stable operation name that exceeded its deadline.
        operation: &'static str,
    },
    /// The caller or service shutdown cancelled the operation.
    #[error("MCP operation was cancelled: {operation}")]
    Cancelled {
        /// Stable operation name that was cancelled.
        operation: &'static str,
    },
}

/// One absolute deadline and cancellation signal shared across transport work.
#[derive(Clone)]
pub struct McpRequestContext {
    deadline: Instant,
    cancellation: CancellationToken,
}

impl McpRequestContext {
    /// Construct a request context with an absolute deadline.
    #[must_use]
    pub const fn new(deadline: Instant, cancellation: CancellationToken) -> Self {
        Self {
            deadline,
            cancellation,
        }
    }

    /// Construct a request context whose deadline is relative to now.
    #[must_use]
    pub fn with_timeout(timeout: Duration, cancellation: CancellationToken) -> Self {
        let now = Instant::now();
        Self::new(now.checked_add(timeout).unwrap_or(now), cancellation)
    }

    /// Return the absolute request deadline.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Return a clone of the caller cancellation signal.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    /// Fail before starting remote work when the context is already terminal.
    pub(crate) fn check(&self, operation: &'static str) -> McpClientResult<()> {
        if self.cancellation.is_cancelled() {
            return Err(McpClientError::Cancelled { operation });
        }
        if Instant::now() >= self.deadline {
            return Err(McpClientError::Timeout { operation });
        }
        Ok(())
    }
}

impl fmt::Debug for McpRequestContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpRequestContext")
            .field("is_cancelled", &self.cancellation.is_cancelled())
            .field("deadline_elapsed", &(Instant::now() >= self.deadline))
            .finish()
    }
}

/// Tool-call response before policy maps a tool-declared failure.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolCallOutput {
    /// Complete MCP `tools/call` result encoded as JSON.
    pub result: Value,
    /// The server-declared `isError` bit. This is not a transport error.
    pub is_error: bool,
}

/// A connected and initialized MCP session.
pub trait McpPreparedSession: Send {
    /// Discover every tool advertised by the initialized server.
    fn discover_tools<'a>(
        &'a mut self,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, Vec<McpDiscoveredTool>>;

    /// Call one tool without interpreting the server-declared `isError` bit.
    fn call_tool<'a>(
        &'a mut self,
        tool_name: &'a str,
        arguments: Value,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpToolCallOutput>;

    /// Close the protocol service and its underlying transport.
    fn close(self: Box<Self>) -> McpClientFuture<'static, ()>;
}

/// Prepared session plus refreshed credentials that have not been persisted.
pub struct McpSessionPreparation {
    session: Box<dyn McpPreparedSession>,
    refreshed_oauth_credentials: Option<McpOAuthStoredCredentials>,
}

impl McpSessionPreparation {
    /// Construct a preparation result owned by a transport factory.
    #[must_use]
    pub fn new(
        session: Box<dyn McpPreparedSession>,
        refreshed_oauth_credentials: Option<McpOAuthStoredCredentials>,
    ) -> Self {
        Self {
            session,
            refreshed_oauth_credentials,
        }
    }

    /// Split the preparation into its session and unpersisted credentials.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Box<dyn McpPreparedSession>,
        Option<McpOAuthStoredCredentials>,
    ) {
        (self.session, self.refreshed_oauth_credentials)
    }
}

impl fmt::Debug for McpSessionPreparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpSessionPreparation")
            .field("session", &"[PREPARED]")
            .field(
                "refreshed_oauth_credentials",
                &self
                    .refreshed_oauth_credentials
                    .as_ref()
                    .map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// Factory seam for preparing one initialized transport session.
pub trait McpSessionFactory: Send + Sync + fmt::Debug {
    /// Prepare a session without persisting refreshed credentials or invoking a tool.
    fn prepare<'a>(
        &'a self,
        server: &'a McpServerRecord,
        secrets: &'a McpSecretMaterial,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpSessionPreparation>;
}

/// Shared object-safe MCP transport factory.
pub type McpSessionFactoryHandle = Arc<dyn McpSessionFactory>;

/// Routes preparation to the configured factory for each supported transport.
#[derive(Clone)]
pub struct McpSessionFactoryRouter {
    stdio: McpSessionFactoryHandle,
    streamable_http: McpSessionFactoryHandle,
}

impl McpSessionFactoryRouter {
    /// Construct a router over the two supported MCP transport factories.
    #[must_use]
    pub const fn new(
        stdio: McpSessionFactoryHandle,
        streamable_http: McpSessionFactoryHandle,
    ) -> Self {
        Self {
            stdio,
            streamable_http,
        }
    }
}

impl fmt::Debug for McpSessionFactoryRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpSessionFactoryRouter")
            .field("stdio", &"[CONFIGURED]")
            .field("streamable_http", &"[CONFIGURED]")
            .finish()
    }
}

impl McpSessionFactory for McpSessionFactoryRouter {
    fn prepare<'a>(
        &'a self,
        server: &'a McpServerRecord,
        secrets: &'a McpSecretMaterial,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpSessionPreparation> {
        match server.transport_kind {
            McpTransportKind::Stdio => self.stdio.prepare(server, secrets, context),
            McpTransportKind::StreamableHttp => {
                self.streamable_http.prepare(server, secrets, context)
            }
        }
    }
}

pub(crate) struct RmcpPreparedSession {
    service: RunningService<RoleClient, ()>,
    diagnostics: Option<McpDiagnosticHandle>,
    mcp_server_id: String,
    transport_kind: &'static str,
}

impl RmcpPreparedSession {
    pub(crate) fn new(
        service: RunningService<RoleClient, ()>,
        diagnostics: Option<McpDiagnosticHandle>,
        mcp_server_id: String,
        transport_kind: &'static str,
    ) -> Self {
        Self {
            service,
            diagnostics,
            mcp_server_id,
            transport_kind,
        }
    }

    fn record_malformed(
        &self,
        operation: &'static str,
        error: &McpClientError,
        raw: Option<Value>,
    ) {
        let Some(diagnostics) = &self.diagnostics else {
            return;
        };
        diagnostics.record(McpDiagnosticEvent {
            kind: McpDiagnosticKind::MalformedResponse,
            message: "MCP returned a malformed protocol response".to_string(),
            mcp_server_id: Some(self.mcp_server_id.clone()),
            tool_name: None,
            operation,
            error_chain: vec![bounded_diagnostic_text(error.to_string())],
            raw: raw.map(|payload| {
                json!({
                    "transport_kind": self.transport_kind,
                    "payload": payload,
                })
            }),
        });
    }
}

impl Drop for RmcpPreparedSession {
    fn drop(&mut self) {
        self.service.cancellation_token().cancel();
    }
}

impl McpPreparedSession for RmcpPreparedSession {
    fn discover_tools<'a>(
        &'a mut self,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, Vec<McpDiscoveredTool>> {
        Box::pin(async move {
            let tools = run_with_context(context, "tools/list", async {
                let mut tools = Vec::new();
                let mut cursor = None;
                let mut seen_cursors = HashSet::new();
                for _ in 0..MAX_DISCOVERED_TOOLS {
                    let result = self
                        .service
                        .peer()
                        .list_tools(Some(PaginatedRequestParams::default().with_cursor(cursor)))
                        .await
                        .map_err(|error| client_service_error("tools/list", error))?;
                    for tool in &result.tools {
                        validate_rmcp_tool_metadata(tool)?;
                    }
                    if tools.len().saturating_add(result.tools.len()) > MAX_DISCOVERED_TOOLS {
                        return Err(McpClientError::Malformed(
                            "MCP tools/list exceeded the supported tool count".to_string(),
                        ));
                    }
                    tools.extend(result.tools);
                    let Some(next_cursor) = result.next_cursor else {
                        return Ok(tools);
                    };
                    if next_cursor.is_empty()
                        || next_cursor.len() > MAX_PAGINATION_CURSOR_BYTES
                        || !seen_cursors.insert(next_cursor.clone())
                    {
                        return Err(McpClientError::Malformed(
                            "MCP tools/list returned an invalid pagination cursor".to_string(),
                        ));
                    }
                    cursor = Some(next_cursor);
                }
                Err(McpClientError::Malformed(
                    "MCP tools/list exceeded the supported page count".to_string(),
                ))
            })
            .await;
            let tools = match tools {
                Ok(tools) => tools,
                Err(error) => {
                    if matches!(error, McpClientError::Malformed(_)) {
                        self.record_malformed("tools/list", &error, None);
                    }
                    return Err(error);
                }
            };

            tools
                .into_iter()
                .map(|tool| {
                    let raw = Some(json!({
                        "tool_name": bounded_diagnostic_text(tool.name.as_ref()),
                        "payload_omitted": true,
                    }));
                    discovered_tool_from_rmcp(tool).inspect_err(|error| {
                        self.record_malformed("tools/list", error, raw);
                    })
                })
                .collect()
        })
    }

    fn call_tool<'a>(
        &'a mut self,
        tool_name: &'a str,
        arguments: Value,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpToolCallOutput> {
        Box::pin(async move {
            context.check("tools/call")?;
            let params = call_tool_params(tool_name, arguments)?;
            let result = run_with_context(context, "tools/call", async {
                self.service
                    .peer()
                    .call_tool(params)
                    .await
                    .map_err(|error| client_service_error("tools/call", error))
            })
            .await?;
            let raw = Some(json!({"payload_omitted": true}));
            tool_call_output_from_rmcp(result).inspect_err(|error| {
                self.record_malformed("tools/call", error, raw);
            })
        })
    }

    fn close(mut self: Box<Self>) -> McpClientFuture<'static, ()> {
        Box::pin(async move {
            let cancellation = self.service.cancellation_token();
            match tokio::time::timeout(SESSION_CLOSE_TIMEOUT, self.service.close()).await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(error)) => Err(McpClientError::Unavailable(format!(
                    "MCP session cleanup failed: {error}"
                ))),
                Err(_) => {
                    cancellation.cancel();
                    Err(McpClientError::Timeout {
                        operation: SESSION_CLOSE_OPERATION,
                    })
                }
            }
        })
    }
}

fn validate_rmcp_tool_metadata(tool: &Tool) -> McpClientResult<()> {
    if tool.name.len() > MAX_TOOL_NAME_BYTES {
        return Err(McpClientError::Malformed(
            "MCP tool name exceeded the supported size".to_string(),
        ));
    }
    if tool
        .description
        .as_deref()
        .is_some_and(|description| description.len() > MAX_TOOL_DESCRIPTION_BYTES)
    {
        return Err(McpClientError::Malformed(
            "MCP tool description exceeded the supported size".to_string(),
        ));
    }
    if let Some(violation) =
        json_object_limit_violation(tool.input_schema.as_ref(), MAX_SCHEMA_BYTES)
    {
        return Err(McpClientError::UnsupportedMetadata(format!(
            "MCP tool `{}` input schema exceeded the supported {}",
            tool.name,
            violation.description()
        )));
    }
    if let Some(violation) = tool
        .output_schema
        .as_deref()
        .and_then(|schema| json_object_limit_violation(schema, MAX_SCHEMA_BYTES))
    {
        return Err(McpClientError::UnsupportedMetadata(format!(
            "MCP tool `{}` output schema exceeded the supported {}",
            tool.name,
            violation.description()
        )));
    }
    if let Some(annotations) = &tool.annotations {
        let annotations = serde_json::to_value(annotations).map_err(|error| {
            McpClientError::Malformed(format!("MCP tool annotations were invalid: {error}"))
        })?;
        if let Some(violation) = json_limit_violation(&annotations, MAX_ANNOTATIONS_BYTES) {
            return Err(McpClientError::UnsupportedMetadata(format!(
                "MCP tool `{}` annotations exceeded the supported {}",
                tool.name,
                violation.description()
            )));
        }
    }
    Ok(())
}

pub(crate) async fn run_with_context<T>(
    context: &McpRequestContext,
    operation: &'static str,
    future: impl Future<Output = McpClientResult<T>>,
) -> McpClientResult<T> {
    context.check(operation)?;
    let deadline = tokio::time::Instant::from_std(context.deadline());
    tokio::select! {
        biased;
        () = context.cancellation.cancelled() => Err(McpClientError::Cancelled { operation }),
        result = tokio::time::timeout_at(deadline, future) => {
            result.unwrap_or(Err(McpClientError::Timeout { operation }))
        }
    }
}

pub(crate) fn initialize_error(
    operation: &'static str,
    error: ClientInitializeError,
) -> McpClientError {
    match error {
        ClientInitializeError::Cancelled => McpClientError::Cancelled { operation },
        ClientInitializeError::ExpectedInitResponse(_)
        | ClientInitializeError::ExpectedInitResult(_) => {
            McpClientError::Malformed(bounded_diagnostic_text(error.to_string()))
        }
        ClientInitializeError::ConflictInitResponseId(_, _)
        | ClientInitializeError::JsonRpcError(_) => {
            McpClientError::Protocol(bounded_diagnostic_text(error.to_string()))
        }
        ClientInitializeError::ConnectionClosed(_) => {
            McpClientError::Unavailable(bounded_diagnostic_text(error.to_string()))
        }
        ClientInitializeError::TransportError {
            error: transport_error,
            ..
        } => dynamic_transport_error(transport_error),
        _ => McpClientError::Unavailable(bounded_diagnostic_text(error.to_string())),
    }
}

fn client_service_error(operation: &'static str, error: ServiceError) -> McpClientError {
    match error {
        ServiceError::McpError(_) => {
            McpClientError::Protocol(bounded_diagnostic_text(error.to_string()))
        }
        ServiceError::TransportSend(error) => dynamic_transport_error(error),
        ServiceError::TransportClosed => {
            McpClientError::Unavailable(bounded_diagnostic_text(error.to_string()))
        }
        ServiceError::UnexpectedResponse => {
            McpClientError::Malformed(bounded_diagnostic_text(error.to_string()))
        }
        ServiceError::Cancelled { .. } => McpClientError::Cancelled { operation },
        ServiceError::Timeout { .. } => McpClientError::Timeout { operation },
        _ => McpClientError::Protocol(bounded_diagnostic_text(error.to_string())),
    }
}

fn dynamic_transport_error(error: DynamicTransportError) -> McpClientError {
    if error_chain_requires_authentication(error.error.as_ref()) {
        McpClientError::AuthenticationRequired(
            "the MCP server rejected its credentials".to_string(),
        )
    } else {
        McpClientError::Unavailable(bounded_diagnostic_text(error.to_string()))
    }
}

fn error_chain_requires_authentication(mut error: &(dyn std::error::Error + 'static)) -> bool {
    loop {
        if error
            .downcast_ref::<StreamableHttpError<reqwest::Error>>()
            .is_some_and(|error| {
                matches!(
                    error,
                    StreamableHttpError::AuthRequired(_)
                        | StreamableHttpError::InsufficientScope(_)
                )
            })
        {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

fn discovered_tool_from_rmcp(tool: Tool) -> McpClientResult<McpDiscoveredTool> {
    let annotations = match tool.annotations {
        Some(annotations) => serde_json::to_value(annotations).map_err(|error| {
            McpClientError::Malformed(format!("invalid MCP tool annotations: {error}"))
        })?,
        None => json!({}),
    };
    let mut tool = McpDiscoveredTool {
        name: tool.name.into_owned(),
        description: tool.description.map(std::borrow::Cow::into_owned),
        input_schema: Value::Object((*tool.input_schema).clone()),
        output_schema: tool
            .output_schema
            .map(|schema| Value::Object((*schema).clone())),
        annotations,
        metadata_fingerprint: String::new(),
    };
    tool.metadata_fingerprint = discovered_tool_fingerprint(&tool);
    Ok(tool)
}

fn call_tool_params(tool_name: &str, arguments: Value) -> McpClientResult<CallToolRequestParams> {
    let arguments = match arguments {
        Value::Object(arguments) => arguments,
        Value::Null => Map::new(),
        _ => {
            return Err(McpClientError::Malformed(
                "MCP tool arguments must be an object".to_string(),
            ));
        }
    };
    Ok(CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments))
}

fn tool_call_output_from_rmcp(result: CallToolResult) -> McpClientResult<McpToolCallOutput> {
    let is_error = result.is_error.unwrap_or(false);
    let result = serde_json::to_value(result).map_err(|error| {
        McpClientError::Malformed(format!("invalid MCP tools/call result: {error}"))
    })?;
    if !json_within_limits(&result, MAX_TOOL_RESULT_BYTES) {
        return Err(McpClientError::Malformed(
            "MCP tools/call result exceeded supported limits".to_string(),
        ));
    }
    Ok(McpToolCallOutput { result, is_error })
}

#[cfg(test)]
mod tests;
