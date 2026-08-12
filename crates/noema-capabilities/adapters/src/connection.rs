//! Filesystem-canonical, provider-neutral adapter connection models.

use noema_capabilities::{CapabilityConnectionPolicy, CapabilityToolPolicyOverride};
use serde::{Deserialize, Serialize};

/// Non-secret desired lifecycle state owned by `connection.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterConnectionStatus {
    /// Reviewed operations may be advertised when credentials and grants match.
    Active,
    /// The human or runtime suspended the connection.
    Suspended,
    /// Interactive authentication or credential replacement is required.
    AuthenticationRequired,
}

impl AdapterConnectionStatus {
    /// Return the stable projection label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::AuthenticationRequired => "authentication_required",
        }
    }
}

/// Non-secret authentication reference owned by an API connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdapterConnectionAuthenticationV1 {
    /// The reviewed definition requires no authentication.
    None,
    /// The connection owns one direct-credential generation.
    Credential {
        /// Current protected direct-credential generation.
        generation_id: String,
        /// Immutable credential revision used by delayed calls.
        revision: u64,
    },
    /// The connection uses one reusable OAuth authorization grant.
    OauthGrant {
        /// Stable grant identity.
        grant_id: String,
    },
}

/// API connection descriptor with reusable OAuth grant ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterConnectionV4 {
    /// Exact descriptor schema. Only version 4 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex identity and directory name.
    pub connection_id: String,
    /// Stable human-chosen tool namespace component.
    pub connection_slug: String,
    /// Exact reviewed definition content address.
    pub semantic_digest: String,
    /// Optional human-visible label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_label: Option<String>,
    /// Desired lifecycle state. Authentication health comes from the grant.
    pub status: AdapterConnectionStatus,
    /// Descriptor revision used by delayed-work fences.
    pub connection_revision: u64,
    /// Policy revision used by delayed-work fences.
    pub policy_revision: u64,
    /// Direct credential or reusable OAuth grant reference.
    pub authentication: AdapterConnectionAuthenticationV1,
    /// Reviewed operation identities enabled for this connection.
    pub allowed_operations: Vec<String>,
    /// Connection-owned sharing and unsafe-action policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<CapabilityConnectionPolicy>,
    /// Human behavior overrides fenced to operation source revisions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_overrides: Vec<CapabilityToolPolicyOverride>,
}

/// Immutable secret-bearing credential generation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCredentialGenerationV2 {
    /// Exact credential schema. Only version 2 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex generation identity and filename.
    pub generation_id: String,
    /// Closed credential material selected by the definition auth mode.
    pub material: AdapterCredentialMaterial,
}

impl std::fmt::Debug for AdapterCredentialGenerationV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterCredentialGenerationV2")
            .field("schema_version", &self.schema_version)
            .field("generation_id", &self.generation_id)
            .field("material", &"[REDACTED]")
            .finish()
    }
}

/// Credential families supported by the v1 definition vocabulary.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdapterCredentialMaterial {
    /// Closed private fields consumed only by reviewed request-auth Luau.
    Credential {
        /// Exact normalized string fields keyed by reviewed field identifiers.
        fields: std::collections::BTreeMap<String, String>,
    },
}

impl std::fmt::Debug for AdapterCredentialMaterial {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AdapterCredentialMaterial([REDACTED])")
    }
}
