//! MCP client runtime for metadata discovery and mediated tool calls.

use serde_json::{Map, Value, json};
use thiserror::Error;

/// MCP tool metadata discovered during setup.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredMcpTool {
    /// MCP tool name.
    pub name: String,
    /// Optional human-readable MCP tool description.
    pub description: Option<String>,
    /// MCP input schema used for argument validation.
    pub input_schema: Value,
    /// Optional MCP output schema used for result validation.
    pub output_schema: Option<Value>,
    /// MCP tool annotations captured as non-authoritative setup hints.
    pub annotations: Value,
}

/// One parsed `tools/list` result page.
pub(crate) struct DiscoveredMcpToolsPage {
    /// Discovered tools in the page.
    pub(crate) tools: Vec<DiscoveredMcpTool>,
    /// Cursor for the next page, when present.
    pub(crate) next_cursor: Option<String>,
}

/// Errors returned by MCP client discovery or mediated tool calls.
#[derive(Debug, Error)]
pub enum McpClientError {
    /// The MCP server requires credentials before metadata can be listed.
    #[error("MCP authentication required: {0}")]
    AuthRequired(String),
    /// The underlying transport failed.
    #[error("MCP transport failed: {0}")]
    Transport(String),
    /// The transport returned malformed metadata.
    #[error("MCP response was malformed: {0}")]
    Malformed(String),
}

/// Transport operations needed for MCP discovery and mediated tool calls.
pub trait McpTransport: Send {
    /// Initialize the MCP connection without invoking tools.
    fn initialize(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), McpClientError>> + Send;

    /// List MCP tool metadata without invoking tools.
    fn list_tools(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Vec<DiscoveredMcpTool>, McpClientError>> + Send;

    /// Call one MCP tool through the initialized transport.
    fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> impl std::future::Future<Output = Result<Value, McpClientError>> + Send;
}

/// Generic MCP client runtime.
pub struct McpClientRuntime<T> {
    transport: T,
}

impl<T> McpClientRuntime<T>
where
    T: McpTransport,
{
    /// Create a metadata-only MCP client over a concrete transport.
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Initialize the transport and list discovered MCP tools.
    ///
    /// This method performs only metadata discovery. It does not call or invoke
    /// any MCP tools.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization or metadata listing fails.
    pub async fn discover_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
        self.transport.initialize().await?;
        self.transport.list_tools().await
    }

    /// Initialize the transport and call one MCP tool.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization or the MCP tool call fails.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, McpClientError> {
        self.transport.initialize().await?;
        self.transport.call_tool(name, arguments).await
    }
}

/// Parse an MCP `tools/list` result object into normalized discovery records.
pub(crate) fn parse_tools_list_result(
    result: Value,
) -> Result<DiscoveredMcpToolsPage, McpClientError> {
    let object = result.as_object().ok_or_else(|| {
        McpClientError::Malformed("tools/list result must be an object".to_string())
    })?;
    let tools = object
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpClientError::Malformed("tools/list result must include tools array".to_string())
        })?
        .iter()
        .map(parse_tool)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = match object.get("nextCursor") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err(McpClientError::Malformed(
                "tools/list nextCursor must be a string".to_string(),
            ));
        }
    };
    Ok(DiscoveredMcpToolsPage { tools, next_cursor })
}

/// Normalize an SDK-discovered MCP tool into Noema's setup metadata shape.
pub(crate) fn discovered_tool_from_rmcp(
    tool: rmcp::model::Tool,
) -> Result<DiscoveredMcpTool, McpClientError> {
    let input_schema = Value::Object((*tool.input_schema).clone());
    let output_schema = tool
        .output_schema
        .map(|schema| Value::Object((*schema).clone()));
    let annotations = match tool.annotations {
        Some(annotations) => serde_json::to_value(annotations).map_err(|error| {
            McpClientError::Malformed(format!("invalid MCP tool annotations: {error}"))
        })?,
        None => json!({}),
    };
    Ok(DiscoveredMcpTool {
        name: tool.name.into_owned(),
        description: tool.description.map(std::borrow::Cow::into_owned),
        input_schema,
        output_schema,
        annotations,
    })
}

fn parse_tool(value: &Value) -> Result<DiscoveredMcpTool, McpClientError> {
    let object = value
        .as_object()
        .ok_or_else(|| McpClientError::Malformed("MCP tool must be an object".to_string()))?;
    let name = string_field(object, "name").map_err(McpClientError::Malformed)?;
    let description = match object.get("description") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err(McpClientError::Malformed(
                "MCP tool description must be a string".to_string(),
            ));
        }
    };
    let input_schema = object
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !input_schema.is_object() {
        return Err(McpClientError::Malformed(
            "MCP tool inputSchema must be an object".to_string(),
        ));
    }
    let output_schema = match object.get("outputSchema") {
        None | Some(Value::Null) => None,
        Some(value) if value.is_object() => Some(value.clone()),
        Some(_) => {
            return Err(McpClientError::Malformed(
                "MCP tool outputSchema must be an object".to_string(),
            ));
        }
    };
    let annotations = object
        .get("annotations")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !annotations.is_object() {
        return Err(McpClientError::Malformed(
            "MCP tool annotations must be an object".to_string(),
        ));
    }

    Ok(DiscoveredMcpTool {
        name,
        description,
        input_schema,
        output_schema,
        annotations,
    })
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("MCP config field {field} must be a string"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    #[tokio::test]
    async fn metadata_discovery_lists_tools_without_calling_them() {
        let transport = FakeMcpTransport::new(vec![fake_tool("read_doc")]);
        let mut client = McpClientRuntime::new(transport.clone());

        let tools = client.discover_tools().await.expect("tools");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_doc");
        assert_eq!(tools[0].description.as_deref(), Some("Read a document"));
        assert_eq!(
            tools[0].input_schema,
            json!({
                "type": "object",
                "properties": {
                    "document_id": { "type": "string" }
                },
                "required": ["document_id"]
            })
        );
        assert_eq!(
            tools[0].output_schema,
            Some(json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string" }
                },
                "required": ["content"]
            }))
        );
        assert_eq!(tools[0].annotations, json!({ "readOnlyHint": true }));
        assert_eq!(transport.initialize_count(), 1);
        assert_eq!(transport.list_tools_count(), 1);
        assert_eq!(transport.call_count(), 0);
    }

    #[test]
    fn tools_list_parser_accepts_schema_and_annotations() {
        let page = parse_tools_list_result(json!({
            "tools": [{
                "name": "read_doc",
                "description": "Read a document",
                "inputSchema": { "type": "object" },
                "outputSchema": { "type": "object" },
                "annotations": { "readOnlyHint": true }
            }],
            "nextCursor": "next"
        }))
        .expect("page");

        assert_eq!(page.tools.len(), 1);
        assert_eq!(page.tools[0].name, "read_doc");
        assert_eq!(
            page.tools[0].description.as_deref(),
            Some("Read a document")
        );
        assert_eq!(page.tools[0].input_schema, json!({ "type": "object" }));
        assert_eq!(
            page.tools[0].output_schema,
            Some(json!({ "type": "object" }))
        );
        assert_eq!(page.tools[0].annotations, json!({ "readOnlyHint": true }));
        assert_eq!(page.next_cursor.as_deref(), Some("next"));
    }

    #[tokio::test]
    async fn metadata_discovery_returns_auth_required_from_initialize() {
        let transport = FakeMcpTransport::auth_required("missing token");
        let mut client = McpClientRuntime::new(transport);

        let error = client
            .discover_tools()
            .await
            .expect_err("auth required should propagate");

        assert!(
            matches!(error, McpClientError::AuthRequired(message) if message == "missing token")
        );
    }

    #[derive(Clone)]
    struct FakeMcpTransport {
        state: Arc<FakeMcpTransportState>,
    }

    struct FakeMcpTransportState {
        tools: Vec<DiscoveredMcpTool>,
        initialize_error: Option<String>,
        initialize_count: AtomicUsize,
        list_tools_count: AtomicUsize,
        call_count: AtomicUsize,
    }

    impl FakeMcpTransport {
        fn new(tools: Vec<DiscoveredMcpTool>) -> Self {
            Self {
                state: Arc::new(FakeMcpTransportState {
                    tools,
                    initialize_error: None,
                    initialize_count: AtomicUsize::new(0),
                    list_tools_count: AtomicUsize::new(0),
                    call_count: AtomicUsize::new(0),
                }),
            }
        }

        fn auth_required(message: &str) -> Self {
            Self {
                state: Arc::new(FakeMcpTransportState {
                    tools: Vec::new(),
                    initialize_error: Some(message.to_string()),
                    initialize_count: AtomicUsize::new(0),
                    list_tools_count: AtomicUsize::new(0),
                    call_count: AtomicUsize::new(0),
                }),
            }
        }

        fn initialize_count(&self) -> usize {
            self.state.initialize_count.load(Ordering::SeqCst)
        }

        fn list_tools_count(&self) -> usize {
            self.state.list_tools_count.load(Ordering::SeqCst)
        }

        fn call_count(&self) -> usize {
            self.state.call_count.load(Ordering::SeqCst)
        }

        #[allow(dead_code)]
        async fn call_tool(&self) {
            tokio::time::sleep(Duration::from_millis(1)).await;
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl McpTransport for FakeMcpTransport {
        async fn initialize(&mut self) -> Result<(), McpClientError> {
            self.state.initialize_count.fetch_add(1, Ordering::SeqCst);
            if let Some(message) = &self.state.initialize_error {
                return Err(McpClientError::AuthRequired(message.clone()));
            }
            Ok(())
        }

        async fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>, McpClientError> {
            self.state.list_tools_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.state.tools.clone())
        }

        async fn call_tool(
            &mut self,
            _name: &str,
            _arguments: Value,
        ) -> Result<Value, McpClientError> {
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"content": []}))
        }
    }

    fn fake_tool(name: &str) -> DiscoveredMcpTool {
        DiscoveredMcpTool {
            name: name.to_owned(),
            description: Some("Read a document".to_owned()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "document_id": { "type": "string" }
                },
                "required": ["document_id"]
            }),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string" }
                },
                "required": ["content"]
            })),
            annotations: json!({ "readOnlyHint": true }),
        }
    }
}
