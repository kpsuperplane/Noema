//! Third-party MCP control-plane domain types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Effective trust classification for an MCP tool policy axis.
pub enum McpTrustClassification {
    /// The tool does not exercise this policy axis.
    None,
    /// The tool operates only on trusted-owner data for this axis.
    Trusted,
    /// The tool operates on untrusted-owner data for this axis.
    Untrusted,
    /// Trust depends on resolved ownership for this axis.
    Mixed,
}

impl McpTrustClassification {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Trusted identity selector type used for deterministic ownership matching.
pub enum TrustedIdentitySelectorKind {
    /// Email address selector.
    Email,
    /// Phone number selector.
    Phone,
    /// DNS domain selector.
    Domain,
}

impl TrustedIdentitySelectorKind {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Phone => "phone",
            Self::Domain => "domain",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Transport used to connect to an MCP server.
pub enum McpTransportKind {
    /// Local stdio MCP transport.
    Stdio,
    /// Remote HTTP/SSE MCP transport.
    HttpSse,
}

impl McpTransportKind {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::HttpSse => "http_sse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Review status for a calibrated MCP tool.
pub enum McpCalibrationStatus {
    /// The tool metadata exists but needs human or admin review.
    NeedsReview,
    /// The tool cannot be enabled because owner resolution is incomplete.
    BlockedUnresolvedOwnership,
    /// The tool is reviewed and ready for gateway use.
    Ready,
    /// The tool is intentionally disabled.
    Disabled,
}

impl McpCalibrationStatus {
    /// Return the persisted snake_case representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedsReview => "needs_review",
            Self::BlockedUnresolvedOwnership => "blocked_unresolved_ownership",
            Self::Ready => "ready",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Deterministic owner extractor used during MCP ownership resolution.
pub struct OwnerExtractor {
    /// Source document or field family to inspect.
    pub source: OwnerExtractorSource,
    /// Type of trusted identity this extractor returns.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// JSON pointer, JSONPath-style path, URI pattern, or adapter key.
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Source for a deterministic MCP owner extractor.
pub enum OwnerExtractorSource {
    /// Extract from MCP tool arguments.
    Arguments,
    /// Extract from MCP structured result content.
    StructuredContent,
    /// Extract from MCP result or resource metadata.
    Metadata,
    /// Extract from an MCP resource URI.
    ResourceUri,
    /// Extract through a built-in Noema adapter rule.
    BuiltInAdapter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// MCP tool schema metadata captured during discovery.
pub struct McpToolSchema {
    /// MCP input schema used for argument validation.
    pub input_schema: Value,
    /// Optional MCP output schema used for result validation.
    pub output_schema: Option<Value>,
    /// MCP tool annotations captured as non-authoritative setup hints.
    pub annotations: Value,
}

/// Normalize a trusted identity selector value before storage or matching.
#[must_use]
pub fn normalize_trusted_identity_value(
    kind: TrustedIdentitySelectorKind,
    value: &str,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match kind {
        TrustedIdentitySelectorKind::Email | TrustedIdentitySelectorKind::Domain => {
            Some(trimmed.to_ascii_lowercase())
        }
        TrustedIdentitySelectorKind::Phone => normalize_phone_value(trimmed),
    }
}

fn normalize_phone_value(trimmed: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut has_digit = false;

    for (index, ch) in trimmed.chars().enumerate() {
        if ch == '+' && index == 0 {
            normalized.push(ch);
        } else if ch.is_ascii_digit() {
            normalized.push(ch);
            has_digit = true;
        }
    }

    has_digit.then_some(normalized)
}
