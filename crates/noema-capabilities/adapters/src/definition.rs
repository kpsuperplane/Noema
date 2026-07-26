//! Closed v1 adapter-definition vocabulary.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical provider-neutral adapter manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterManifestV1 {
    /// Exact schema version. Only version 1 is accepted.
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
    /// Provider-side retention and deletion contract.
    pub provider_data_policy: ProviderDataPolicy,
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

/// Provider-side data-use contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDataPolicy {
    /// Provider may retain ordinary requests according to its reviewed terms.
    pub retention_allowed: bool,
    /// Provider exposes a deletion mechanism for retained user data.
    pub deletion_supported: bool,
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
    /// Explicit side-effect class.
    pub effect: OperationEffect,
    /// Explicit reviewed admission route.
    pub admission: AdmissionMode,
    /// Result delivery and persistence contract.
    pub result: ResultDefinition,
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

/// Side-effect classes supported by adapter definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationEffect {
    /// Authenticated or public read.
    ReadOnly,
    /// External state change.
    ExternalWrite,
    /// External data egress.
    ExternalExport,
    /// External state change and data egress.
    ExternalWriteAndExport,
}

/// Admission routes that a reviewed definition may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionMode {
    /// No governed action. Valid only for read-only operations.
    Direct,
    /// Deterministic policy and reviewer may authorize the action.
    ReviewerMayApprove,
    /// Human approval is always required.
    AlwaysAsk,
}

/// Result projection contract supported by the central runtime boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultDefinition {
    /// Whether result bytes contain private account data.
    pub classification: ResultClassification,
    /// Model route permitted to receive the result.
    pub model_route: ModelRoute,
    /// Model payload view.
    pub model_payload: ModelPayload,
    /// Provider-side response storage/cache behavior.
    pub provider_retention: ProviderRetention,
    /// Durable local persistence view.
    pub persistence: PersistenceMode,
}

/// Result sensitivity class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultClassification {
    /// Public/non-account data.
    Public,
    /// Authenticated or private account data.
    Private,
}

/// Model route constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRoute {
    /// Any exact configured route may receive the view.
    AnyKnownRoute,
    /// Only local inference may receive the view.
    LocalOnly,
}

/// Model-visible result view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPayload {
    /// Bounded invoker output.
    Full,
    /// Fixed operation metadata only.
    MetadataOnly,
    /// No model continuation payload.
    Omit,
}

/// Provider-side retention contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRetention {
    /// Reviewed provider retention is permitted.
    Allow,
    /// Routes using storage, cache, or previous-response state are denied.
    Deny,
}

/// Durable local payload policy. Field allowlists remain unsupported in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceMode {
    /// Persist recursively redacted payloads.
    Redacted,
    /// Persist fixed metadata only.
    MetadataOnly,
    /// Persist no arguments or result payload.
    Omit,
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

/// Pagination semantics known to the definition model.
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
