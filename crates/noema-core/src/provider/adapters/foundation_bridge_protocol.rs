use serde::{Deserialize, Serialize};

/// Bridge protocol version supported by this Noema build.
pub const BRIDGE_PROTOCOL_VERSION: u32 = 1;

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
    /// Text content.
    pub text: String,
}

/// Bridge-visible turn role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRole {
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
        /// Profile options.
        profiles: Vec<BridgeProfile>,
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
    /// Assistant text delta.
    AssistantTextDelta {
        /// Delta text.
        delta: String,
    },
    /// Final assistant text.
    GenerateComplete {
        /// Complete text.
        text: String,
    },
    /// Request failed.
    Error {
        /// Stable safe error code.
        code: String,
        /// Safe error message.
        message: String,
    },
}

/// Bridge-reported profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeProfile {
    /// Stable profile id.
    pub id: String,
    /// User-facing label.
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handshake_request_serializes_as_json_line_payload() {
        let message = BridgeRequest {
            id: "request-1".to_string(),
            payload: BridgeRequestPayload::Handshake {
                protocol_version: BRIDGE_PROTOCOL_VERSION,
            },
        };

        let value = serde_json::to_value(&message).expect("json");

        assert_eq!(value["id"], "request-1");
        assert_eq!(value["payload"]["type"], "handshake");
        assert_eq!(
            value["payload"]["protocol_version"],
            BRIDGE_PROTOCOL_VERSION
        );
    }

    #[test]
    fn generate_delta_response_parses() {
        let value = json!({
            "id": "request-2",
            "payload": {
                "type": "assistant_text_delta",
                "delta": "hello"
            }
        });

        let response: BridgeResponse = serde_json::from_value(value).expect("response");

        assert_eq!(response.id, "request-2");
        assert!(matches!(
            response.payload,
            BridgeResponsePayload::AssistantTextDelta { ref delta } if delta == "hello"
        ));
    }
}
