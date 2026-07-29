use serde::{Deserialize, Serialize};

/// Bridge protocol version supported by this Noema build.
pub const BRIDGE_PROTOCOL_VERSION: u32 = 3;

/// Request sent from Rust to the Swift bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequest {
    /// Stable request id.
    pub id: String,
    /// Request payload.
    pub payload: BridgeRequestPayload,
}

/// Request payload sent from Rust to the Swift bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeRequestPayload {
    /// Version/capability handshake.
    Handshake {
        /// Protocol version requested by Rust.
        protocol_version: u32,
    },
    /// Health/capability check.
    Health,
    /// Create a session for a Noema conversation.
    CreateSession {
        /// Noema conversation id.
        conversation_id: String,
        /// Model/profile id.
        model_profile: String,
        /// Optional instructions.
        instructions: Option<String>,
        /// Model-visible native tool definitions for this session.
        #[serde(default)]
        tools: Vec<BridgeToolDefinition>,
        /// Stable fingerprint of the session's native tool catalog.
        #[serde(default)]
        tool_catalog_fingerprint: String,
    },
    /// Replay prior turns into a session.
    ReplayTurns {
        /// Bridge session id.
        session_id: String,
        /// Prior turns.
        turns: Vec<BridgeReplayTurn>,
    },
    /// Generate the next assistant response.
    Generate {
        /// Bridge session id.
        session_id: String,
        /// User input text.
        input: String,
        /// Optional maximum response tokens.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_output_tokens: Option<u32>,
    },
    /// Deliver the result of a native tool call back to the suspended
    /// Foundation Models generation.
    ToolResult {
        /// Bridge session that owns the suspended generation.
        session_id: String,
        /// Correlation id emitted with the tool-call event.
        call_id: String,
        /// JSON-compatible tool result body.
        output: String,
        /// Whether the tool execution failed.
        #[serde(default)]
        is_error: bool,
    },
    /// Count tokens for instructions and input.
    CountTokens {
        /// Optional instructions.
        instructions: Option<String>,
        /// User input text.
        input: String,
    },
    /// Cancel an in-flight request.
    Cancel {
        /// Request id to cancel.
        request_id: String,
    },
    /// Close a bridge session.
    CloseSession {
        /// Bridge session id.
        session_id: String,
    },
    /// Ask the bridge to shut down.
    Shutdown,
}

/// Replayable turn sent to the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeReplayTurn {
    /// Role visible to the model.
    pub role: BridgeRole,
    /// Text content for ordinary prompt/assistant turns.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Native assistant tool call to replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<BridgeReplayToolCall>,
    /// Native tool result to replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<BridgeReplayToolResult>,
}

/// Model-visible native tool definition sent when a session is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeToolDefinition {
    /// Tool name visible to Foundation Models.
    pub name: String,
    /// Human/model-readable tool description.
    pub description: String,
    /// JSON schema encoded as a string for the Swift bridge.
    pub parameters: String,
}

/// Native tool call in durable Foundation transcript replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeReplayToolCall {
    /// Provider-native correlation id.
    pub call_id: String,
    /// Tool name visible to Foundation Models.
    pub tool_name: String,
    /// JSON arguments encoded as a string.
    pub arguments: String,
}

/// Native tool result in durable Foundation transcript replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeReplayToolResult {
    /// Provider-native correlation id.
    pub call_id: String,
    /// Tool name visible to Foundation Models.
    pub tool_name: String,
    /// Result body encoded as a string.
    pub output: String,
}

/// Native tool call surfaced by the bridge process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeToolCall {
    /// Provider-native correlation id.
    pub call_id: String,
    /// Tool name visible to Foundation Models.
    pub tool_name: String,
    /// JSON arguments.
    pub arguments: String,
}

/// Native tool result sent back to a suspended bridge generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeToolResult {
    /// Provider-native correlation id.
    pub call_id: String,
    /// JSON-compatible result body.
    pub output: String,
    /// Whether the tool execution failed.
    pub is_error: bool,
}

/// Bridge-visible turn role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRole {
    /// Trusted application/runtime context appended to the model transcript.
    ApplicationContext,
    /// User turn.
    User,
    /// Assistant turn.
    Assistant,
}

/// Response sent from the Swift bridge to Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeResponse {
    /// Matching request id.
    pub id: String,
    /// Response payload.
    pub payload: BridgeResponsePayload,
}

/// Response payload sent from the Swift bridge to Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeResponsePayload {
    /// Handshake succeeded.
    HandshakeOk {
        /// Bridge protocol version.
        protocol_version: u32,
    },
    /// Health/capability state.
    Health {
        /// Whether generation is available.
        available: bool,
        /// Safe unavailable reason.
        unavailable_reason: Option<String>,
    },
    /// Session creation succeeded.
    SessionCreated {
        /// Bridge session id.
        session_id: String,
    },
    /// Replay completed.
    ReplayComplete,
    /// Cancellation completed.
    CancelComplete,
    /// Assistant text delta.
    AssistantTextDelta {
        /// Delta text.
        delta: String,
    },
    /// Native tool call requested by Foundation Models.
    ToolCall {
        /// Provider-native correlation id.
        call_id: String,
        /// Tool name visible to Foundation Models.
        tool_name: String,
        /// JSON arguments encoded as a string.
        arguments: String,
    },
    /// Final assistant text.
    GenerateComplete {
        /// Complete text.
        text: String,
    },
    /// Native tool result was accepted by the bridge.
    ToolResultAccepted,
    /// Token count completed.
    TokenCount {
        /// Total tokens counted by the bridge.
        tokens: u32,
    },
    /// Request failed.
    Error {
        /// Stable safe error code.
        code: String,
        /// Safe error message.
        message: String,
    },
}
