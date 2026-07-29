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

/// Exact non-secret revision fence for one connection binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterConnectionRevisions {
    /// Descriptor/lifecycle revision.
    pub connection: u64,
    /// Immutable credential generation revision.
    pub credential: u64,
    /// Reconciled provider grant revision.
    pub grant: u64,
    /// Reviewed operation/policy revision.
    pub policy: u64,
}

/// Canonical non-secret descriptor stored as `connection.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterConnectionV3 {
    /// Exact descriptor schema. Only version 3 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex identity and directory name.
    pub connection_id: String,
    /// Stable human-chosen tool namespace component.
    pub connection_slug: String,
    /// Exact reviewed definition content address.
    pub semantic_digest: String,
    /// Optional stable external account identity, never a display label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Optional human-visible connection label, never used as stable authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_label: Option<String>,
    /// Definition-compatible account surface such as personal or workspace.
    pub account_kind: String,
    /// Desired lifecycle state.
    pub status: AdapterConnectionStatus,
    /// Exact revisions captured by bindings and delayed actions.
    pub revisions: AdapterConnectionRevisions,
    /// Current immutable credential generation, absent for no-auth or
    /// pre-authorization connections awaiting client metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_generation: Option<String>,
    /// Exact provider-returned or locally qualified scope subset.
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    /// Reviewed operation identities enabled for this connection.
    pub allowed_operations: Vec<String>,
    /// Connection-owned sharing and unsafe-action policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<CapabilityConnectionPolicy>,
    /// Human behavior overrides fenced to exact operation source revisions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_overrides: Vec<CapabilityToolPolicyOverride>,
}

/// Immutable secret-bearing credential generation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCredentialGenerationV1 {
    /// Exact credential schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex generation identity and filename.
    pub generation_id: String,
    /// Closed credential material selected by the definition auth mode.
    pub material: AdapterCredentialMaterial,
}

impl std::fmt::Debug for AdapterCredentialGenerationV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterCredentialGenerationV1")
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
    /// Static bearer credential supplied by the human.
    StaticBearer {
        /// Exact secret inserted only by the invoker.
        token: String,
    },
    /// OAuth client metadata retained before interactive authorization.
    Oauth2ClientMetadata {
        /// Client identifier extracted from transient setup input.
        client_id: String,
        /// Optional confidential-client secret.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
    },
    /// OAuth 2.0 client metadata plus a current token generation.
    Oauth2AuthorizationCodePkce {
        /// Client identifier extracted from transient setup input.
        client_id: String,
        /// Optional confidential-client secret.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
        /// Current bearer access token.
        access_token: String,
        /// Optional refresh token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refresh_token: Option<String>,
        /// Optional Unix expiry timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expires_at_epoch_seconds: Option<u64>,
    },
}

impl std::fmt::Debug for AdapterCredentialMaterial {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AdapterCredentialMaterial([REDACTED])")
    }
}
