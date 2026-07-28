//! Closed v3 adapter-definition vocabulary.

use noema_capabilities::CapabilityToolHint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical provider-neutral adapter manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterManifestV3 {
    /// Exact schema version. Only version 3 is accepted.
    pub schema_version: u16,
    /// Stable definition identity.
    pub definition_id: String,
    /// Stable adapter family identity.
    pub adapter_id: String,
    /// Human-facing label; excluded from semantic digests and policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Definition revision chosen by the reviewed source curator.
    pub definition_revision: String,
    /// Whether this exact definition has completed human review.
    pub reviewed: bool,
    /// One fixed request origin. Per-operation hosts are deliberately absent.
    pub origin: String,
    /// Structured authentication requirements; never credential values.
    pub authentication: AuthenticationRequirement,
    /// Structured account/product eligibility gates.
    #[serde(default)]
    pub gates: Vec<AccountGate>,
    /// Quota and economic classification.
    pub quota: QuotaPolicy,
    /// Closed operation set.
    pub operations: Vec<AdapterOperation>,
}

/// Authentication behavior declared as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationRequirement {
    /// Standard credential family.
    pub mode: AuthenticationMode,
    /// Exact reviewed scopes, sorted by the compiler.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Optional official page where the human creates or configures the
    /// provider-side OAuth client. It is display/navigation authority only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_setup_url: Option<String>,
    /// Optional schema for extracting OAuth client metadata from a transient
    /// JSON upload. The schema contains pointers only; it never contains a
    /// credential value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_import: Option<CredentialImportSchema>,
    /// Fixed OAuth 2.0 authorization-code/PKCE endpoints and callback policy.
    /// The setup runtime treats this as reviewed data, never as model input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth2: Option<Oauth2AuthorizationCodePkceConfig>,
    /// Optional read-only operation used once after authentication to obtain a
    /// recognizable account label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_identity: Option<AccountIdentityProbe>,
}

/// Deterministic post-authentication account-label extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountIdentityProbe {
    /// Existing reviewed read operation to execute.
    pub operation_id: String,
    /// Fixed non-secret arguments validated against the operation schema.
    pub arguments: BTreeMap<String, serde_json::Value>,
    /// RFC 6901 pointer to the recognizable string in the operation output.
    pub output_pointer: String,
}

/// Reviewed endpoint and parameter policy for standard OAuth 2.0 setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oauth2AuthorizationCodePkceConfig {
    /// Fixed authorization endpoint. It must use HTTPS at compile time.
    pub authorization_endpoint: String,
    /// Fixed token endpoint. It must use HTTPS at compile time.
    pub token_endpoint: String,
    /// Client authentication method for the later token exchange.
    #[serde(default)]
    pub client_authentication: Oauth2ClientAuthentication,
    /// Explicit callback modes supported by the reviewed application.
    pub callback_modes: Vec<Oauth2CallbackMode>,
    /// Provider-defined authorization parameters, excluding RFC and PKCE keys.
    #[serde(default)]
    pub extra_authorization_parameters: BTreeMap<String, String>,
}

/// OAuth 2.0 token-endpoint client authentication policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Oauth2ClientAuthentication {
    /// Public client or a provider that accepts PKCE without a secret.
    #[default]
    None,
    /// RFC 6749 HTTP Basic client authentication.
    ClientSecretBasic,
    /// RFC 6749 form-body client authentication.
    ClientSecretPost,
}

/// Explicit callback authority selected for one OAuth setup attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Oauth2CallbackMode {
    /// Desktop/native callback on an exact loopback host and port.
    Loopback,
    /// HTTPS callback owned by the configured Noema listener.
    Hosted,
}

/// Definition-declared extraction rules for one OAuth client JSON document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialImportSchema {
    /// Credential document family understood by the generic importer.
    pub kind: CredentialImportKind,
    /// Alternative layouts accepted by this reviewed definition.
    pub alternatives: Vec<CredentialImportLayout>,
}

/// Credential document families supported by the credential importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialImportKind {
    /// OAuth client metadata represented as a JSON object.
    OauthClientJson,
}

/// One exact JSON Pointer layout for OAuth client metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialImportLayout {
    /// RFC 6901 pointer to the nonempty client identifier.
    pub client_id_pointer: String,
    /// Optional RFC 6901 pointer to the confidential-client secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_pointer: Option<String>,
}

/// Authentication modes understood by the definition model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationMode {
    /// No credential is used.
    None,
    /// Static bearer-style credential, supplied only by connection authority.
    StaticBearer,
    /// OAuth 2.0 authorization-code flow with PKCE.
    Oauth2AuthorizationCodePkce,
}

/// Structured account or product eligibility gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AccountGate {
    /// Personal, workspace, bot, merchant, or other account surface.
    AccountKind(String),
    /// Region or marketplace restriction.
    Region(String),
    /// Exact stable API version.
    ApiVersion(String),
    /// Delegated, application, tenant, or audience eligibility.
    AuthEligibility(String),
    /// Product, subscription, or access tier.
    ProductTier(String),
    /// Review, allowlist, partner, or administrator approval requirement.
    AccessReview(String),
    /// Notification endpoint requirement.
    NotificationEndpoint(String),
}

/// Definition-level quota and economic metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaPolicy {
    /// Whether ordinary calls are free, metered, or not yet established.
    pub cost_class: CostClass,
    /// Optional stable quota bucket identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    /// Relative request cost within the bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_units: Option<u32>,
}

/// Economic classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostClass {
    /// No per-call charge is documented.
    Free,
    /// Calls consume a metered or paid allowance.
    Metered,
    /// Economics require later qualification.
    Unknown,
}

/// One declarative HTTP operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterOperation {
    /// Stable operation identity within the definition.
    pub operation_id: String,
    /// Untrusted source prose. It never enters the compiled tool description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
    /// Fixed HTTP method.
    pub method: HttpMethod,
    /// Fixed-origin relative path with named argument placeholders.
    pub path: String,
    /// Fixed non-secret request headers.
    #[serde(default)]
    pub fixed_headers: BTreeMap<String, String>,
    /// User/model arguments. Credential-derived arguments cannot be expressed.
    #[serde(default)]
    pub arguments: Vec<ArgumentDefinition>,
    /// Proposed four-field tool behavior and provenance.
    pub behavior: AdapterOperationBehavior,
    /// Explicit retry behavior.
    pub retry: RetryPolicy,
    /// Explicit pagination behavior.
    pub pagination: PaginationPolicy,
    /// Optional event workflow. M1 parses but does not activate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventMetadata>,
    /// Operation-specific eligibility gates.
    #[serde(default)]
    pub gates: Vec<AccountGate>,
}

/// HTTP methods supported by the declarative JSON transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// Retrieve a resource.
    Get,
    /// Create or invoke a resource.
    Post,
    /// Replace a resource.
    Put,
    /// Partially update a resource.
    Patch,
    /// Delete a resource.
    Delete,
}

/// User/model argument definition. No credential source variant exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgumentDefinition {
    /// Stable argument name.
    pub name: String,
    /// Closed value authority. Credential/runtime sources are not representable.
    pub source: ArgumentSource,
    /// Wire location controlled by the operation plan.
    pub location: ArgumentLocation,
    /// Closed schema type.
    #[serde(rename = "type")]
    pub argument_type: ArgumentType,
    /// Whether the model must provide the value.
    #[serde(default)]
    pub required: bool,
    /// Optional closed string vocabulary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
}

/// Authority permitted to supply one request argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentSource {
    /// Explicit model/user input governed by the provider-visible schema.
    ModelInput,
}

/// Supported argument locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentLocation {
    /// Percent-encoded named path segment.
    Path,
    /// Percent-encoded query parameter.
    Query,
    /// Top-level JSON request member.
    JsonBody,
}

/// Deliberately small, non-ambiguous JSON schema subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentType {
    /// UTF-8 text.
    String,
    /// Signed integer representable by JSON.
    Integer,
    /// Finite JSON number.
    Number,
    /// Boolean value.
    Boolean,
    /// Array of UTF-8 strings.
    StringArray,
}

/// Proposed API behavior for one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AdapterOperationBehavior {
    /// Whether the operation leaves external state unchanged.
    pub read_only: CapabilityToolHint,
    /// Whether repeating identical arguments adds no further change.
    pub idempotent: CapabilityToolHint,
    /// Whether the operation may overwrite or delete state.
    pub destructive: CapabilityToolHint,
    /// Whether the operation may interact with external entities.
    pub open_world: CapabilityToolHint,
}

impl AdapterOperationBehavior {
    /// Construct one complete model-proposed behavior.
    #[must_use]
    pub fn model(read_only: bool, idempotent: bool, destructive: bool, open_world: bool) -> Self {
        let hint = |value| CapabilityToolHint {
            value: Some(value),
            source: Some(noema_capabilities::CapabilityToolHintSource::Model),
        };
        Self {
            read_only: hint(read_only),
            idempotent: hint(idempotent),
            destructive: hint(destructive),
            open_world: hint(open_world),
        }
    }
}

/// Explicit retry semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryPolicy {
    /// Never automatically retry.
    Never,
    /// Retry a transport failure only for an idempotent read.
    TransportSafeRead,
}

/// How a provider-issued continuation URL may carry credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationCredentialMode {
    /// Do not attach the connection bearer to the URL request.
    Omit,
    /// Attach the reviewed connection bearer to the URL request.
    ProviderToken,
}

/// Provider-issued URL use within a bounded operation workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLinkKind {
    /// A bounded next-page or delta link.
    NextPage,
    /// A bounded artifact/download link.
    Download,
}

/// Pagination and continuation semantics known to the definition model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaginationPolicy {
    /// The operation returns one bounded page.
    None,
    /// A response field supplies the next request token. Activation is M2.
    ResponseToken {
        /// Response JSON pointer containing the opaque token.
        response_pointer: String,
        /// Request query argument receiving the token.
        request_argument: String,
    },
    /// A typed absolute provider-issued URL constrained to reviewed origins.
    ProviderLink {
        /// Response JSON pointer containing the absolute link.
        response_pointer: String,
        /// Runtime-only query argument receiving the next page token, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_argument: Option<String>,
        /// Exact HTTPS origins that may receive the link.
        allowed_origins: Vec<String>,
        /// Whether the connection bearer is attached to the link request.
        credential_mode: ContinuationCredentialMode,
        /// Whether this is a next page or download link.
        link_kind: ProviderLinkKind,
        /// Maximum link response body size.
        max_bytes: u32,
        /// Maximum age of the issued link in seconds.
        ttl_seconds: u32,
    },
    /// An opaque provider cursor with an explicit bounded baseline operation.
    DeltaCursor {
        /// Response JSON pointer containing the cursor value.
        response_pointer: String,
        /// Runtime-only query argument receiving the opaque cursor.
        request_argument: String,
        /// Reviewed operation used for a bounded full resynchronization.
        baseline_operation: String,
        /// Maximum cursor age in seconds.
        max_age_seconds: u32,
    },
}

/// Event workflow metadata retained as definition data until M6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventMetadata {
    /// Delivery transport.
    pub transport: EventTransport,
    /// Authenticity mechanism.
    pub authenticity: EventAuthenticity,
}

/// Event delivery transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTransport {
    /// Poll a bounded change feed.
    Poll,
    /// Receive a callback notification.
    Webhook,
}

/// Event authenticity mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAuthenticity {
    /// Shared-secret message authentication.
    Hmac,
    /// Signed JSON web token.
    Jwt,
    /// Provider challenge/response handshake.
    Challenge,
}
