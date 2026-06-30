//! Third-party MCP control-plane domain types.

pub mod client;
/// Disk-backed MCP secret storage.
pub mod secrets;

pub use client::{DiscoveredMcpTool, McpClientError, McpClientRuntime, McpTransport};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Effective trust classification for an MCP tool policy axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Trusted identity selector type used for deterministic ownership matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Transport used to connect to an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Review status for a calibrated MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Deterministic owner extractor used during MCP ownership resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerExtractor {
    /// Source document or field family to inspect.
    pub source: OwnerExtractorSource,
    /// Type of trusted identity this extractor returns.
    pub selector_kind: TrustedIdentitySelectorKind,
    /// JSON pointer, JSONPath-style path, URI pattern, or adapter key.
    pub path: String,
}

/// Source for a deterministic MCP owner extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// MCP tool schema metadata captured during discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        TrustedIdentitySelectorKind::Email => normalize_email_value(trimmed),
        TrustedIdentitySelectorKind::Domain => normalize_domain_value(trimmed),
        TrustedIdentitySelectorKind::Phone => normalize_phone_value(trimmed),
    }
}

fn normalize_email_value(trimmed: &str) -> Option<String> {
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }

    let (local, domain) = trimmed.split_once('@')?;
    if !is_valid_email_local_part(local) || domain.contains('@') {
        return None;
    }

    let normalized_domain = normalize_domain_value(domain)?;
    Some(format!(
        "{}@{}",
        local.to_ascii_lowercase(),
        normalized_domain
    ))
}

fn is_valid_email_local_part(local: &str) -> bool {
    !local.is_empty()
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-'))
}

fn normalize_domain_value(trimmed: &str) -> Option<String> {
    if trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.chars().any(char::is_whitespace)
        || !trimmed.contains('.')
    {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    let labels_are_valid = normalized.split('.').all(is_valid_domain_label);

    labels_are_valid.then_some(normalized)
}

fn is_valid_domain_label(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn normalize_phone_value(trimmed: &str) -> Option<String> {
    if !trimmed.starts_with('+') {
        return None;
    }

    let mut chars = trimmed.chars();
    chars.next();
    if !chars.all(is_allowed_phone_format_char) || trimmed[1..].contains('+') {
        return None;
    }

    let digits: String = trimmed[1..].chars().filter(char::is_ascii_digit).collect();
    let digit_count = digits.len();

    if (8..=15).contains(&digit_count) && !digits.starts_with('0') {
        Some(format!("+{digits}"))
    } else {
        None
    }
}

fn is_allowed_phone_format_char(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, ' ' | '-' | '.' | '(' | ')')
}
