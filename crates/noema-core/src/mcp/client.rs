//! Metadata-only MCP client runtime.

use serde_json::Value;
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

/// Errors returned by metadata-only MCP client discovery.
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

/// Transport operations needed for metadata-only MCP discovery.
pub trait McpTransport: Send {
    /// Initialize the MCP connection without invoking tools.
    fn initialize(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), McpClientError>> + Send;

    /// List MCP tool metadata without invoking tools.
    fn list_tools(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Vec<DiscoveredMcpTool>, McpClientError>> + Send;
}

/// Generic metadata-only MCP client runtime.
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
