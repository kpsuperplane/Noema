//! Closed adapter-definition vocabulary.

use noema_capabilities::CapabilityToolHint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical provider-neutral adapter manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterManifest {
    /// Exact schema version. Only version 9 is accepted.
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
    pub authentication: AuthenticationSchemeV4,
    /// Closed operation set.
    pub operations: Vec<AdapterOperation>,
}

/// Authentication behavior declared as reviewed data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthenticationSchemeV4 {
    /// No credential is used.
    None,
    /// Human-supplied fields or a document normalized into private fields.
    Credential(CredentialAuthentication),
    /// Standard OAuth 2.0 authorization-code flow with PKCE.
    Oauth2AuthorizationCodePkce(Oauth2AuthorizationCodePkceConfig),
}

impl AuthenticationSchemeV4 {
    /// Return the runtime credential family.
    #[must_use]
    pub const fn mode(&self) -> AuthenticationMode {
        match self {
            Self::None => AuthenticationMode::None,
            Self::Credential(_) => AuthenticationMode::Credential,
            Self::Oauth2AuthorizationCodePkce(_) => AuthenticationMode::Oauth2AuthorizationCodePkce,
        }
    }

    /// Return the standard OAuth configuration when selected.
    #[must_use]
    pub const fn oauth2(&self) -> Option<&Oauth2AuthorizationCodePkceConfig> {
        match self {
            Self::Oauth2AuthorizationCodePkce(config) => Some(config),
            Self::None | Self::Credential(_) => None,
        }
    }

    /// Return the generic credential configuration when selected.
    #[must_use]
    pub const fn credential(&self) -> Option<&CredentialAuthentication> {
        match self {
            Self::Credential(config) => Some(config),
            Self::None | Self::Oauth2AuthorizationCodePkce(_) => None,
        }
    }
}

/// Generic private credential and reviewed request-decoration contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialAuthentication {
    /// Human-facing setup contract and private input shape.
    pub setup: CredentialSetup,
    /// Reviewed transform that emits only request headers and query pairs.
    pub request_auth: LuauTransform,
}

/// Human-facing setup guidance and its write-only input contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSetup {
    /// Exact provider-facing credential type, such as `API key`.
    pub credential_type: String,
    /// Official HTTPS page where the credential is created.
    pub setup_url: String,
    /// Short ordered instructions shown before credential entry.
    pub instructions: Vec<String>,
    /// Write-only setup input and normalized private fields.
    pub input: CredentialInput,
}

/// Write-only credential input accepted by the setup UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialInput {
    /// Values entered directly into labeled secret fields.
    Fields {
        /// Exact write-only fields rendered by the setup UI.
        fields: Vec<CredentialField>,
    },
    /// One transient document normalized by reviewed Luau.
    Document {
        /// Exact media type accepted by this setup.
        media_type: String,
        /// Closed private fields the transform must return.
        fields: Vec<CredentialField>,
        /// Reviewed private normalization transform.
        normalize: LuauTransform,
    },
}

impl CredentialInput {
    /// Return the closed normalized credential fields.
    #[must_use]
    pub fn fields(&self) -> &[CredentialField] {
        match self {
            Self::Fields { fields } | Self::Document { fields, .. } => fields,
        }
    }
}

/// One private string field produced by credential setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialField {
    /// Stable field identifier available to reviewed Luau.
    pub id: String,
    /// Human-facing field label; the value remains write-only.
    pub label: String,
}

/// Reviewed Luau source evaluated in one bounded sandbox profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "language", rename_all = "snake_case", deny_unknown_fields)]
pub enum LuauTransform {
    /// Exact reviewed source text.
    Luau {
        /// Exact reviewed source text.
        source: String,
    },
}

impl LuauTransform {
    /// Return the exact reviewed source.
    #[must_use]
    pub fn source(&self) -> &str {
        match self {
            Self::Luau { source } => source,
        }
    }
}

/// One callback-specific OAuth client setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oauth2CredentialSetup {
    /// Callback authority this provider client must be configured for.
    pub callback_mode: Oauth2CallbackMode,
    /// Exact provider setup guidance and document normalizer.
    pub setup: CredentialSetup,
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

/// Exact reviewed OAuth profile required by one definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oauth2AuthorizationCodePkceConfig {
    /// Content address of the reviewed public protocol profile.
    pub profile_digest: String,
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

/// Authentication modes understood by the definition model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationMode {
    /// No credential is used.
    None,
    /// Reviewed Luau decorates the request from private credential fields.
    Credential,
    /// OAuth 2.0 authorization-code flow with PKCE.
    Oauth2AuthorizationCodePkce,
}

/// Authorization requirement for one reviewed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationAuthorization {
    /// The operation needs no OAuth scope.
    None,
    /// Any one complete scope set authorizes the operation.
    OauthScopes {
        /// Alternative complete scope sets in canonical order.
        accepted_scope_sets: Vec<Vec<String>>,
    },
}

impl OperationAuthorization {
    /// Return whether the granted scope set covers this operation.
    #[must_use]
    pub fn is_satisfied_by(&self, granted_scopes: &[String]) -> bool {
        match self {
            Self::None => true,
            Self::OauthScopes {
                accepted_scope_sets,
            } => accepted_scope_sets.iter().any(|required| {
                required
                    .iter()
                    .all(|scope| granted_scopes.binary_search(scope).is_ok())
            }),
        }
    }

    /// Return all accepted complete scope sets.
    #[must_use]
    pub fn accepted_scope_sets(&self) -> &[Vec<String>] {
        match self {
            Self::None => &[],
            Self::OauthScopes {
                accepted_scope_sets,
            } => accepted_scope_sets,
        }
    }
}

/// One declarative HTTP operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterOperation {
    /// Stable operation identity within the definition.
    pub operation_id: String,
    /// Reviewed model-facing guidance for selecting and using this operation.
    pub description: String,
    /// Untrusted source prose. It never enters the compiled tool description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
    /// Fixed HTTP method.
    pub method: HttpMethod,
    /// Fixed-origin relative path with named argument placeholders.
    pub path: String,
    /// Exact operation-specific authorization requirement.
    pub authorization: OperationAuthorization,
    /// Fixed non-secret request headers.
    #[serde(default)]
    pub fixed_headers: BTreeMap<String, String>,
    /// Fixed non-secret query parameters required by reviewed provider semantics.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fixed_query: BTreeMap<String, String>,
    /// User/model arguments. Credential-derived arguments cannot be expressed.
    #[serde(default)]
    pub arguments: Vec<ArgumentDefinition>,
    /// Optional reviewed JSON body shape with exact argument placeholders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_body_template: Option<serde_json::Value>,
    /// Proposed four-field tool behavior and provenance.
    pub behavior: AdapterOperationBehavior,
    /// Explicit retry behavior.
    pub retry: RetryPolicy,
    /// Explicit pagination behavior.
    pub pagination: PaginationPolicy,
    /// Reviewed bounded successful-response contract.
    pub response: ResponseContract,
}

/// Closed transformation contract for one successful HTTP response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseContract {
    /// Exact normalized media types accepted for body-bearing responses.
    pub accepted_content_types: Vec<String>,
    /// Optional deterministic reviewed transform source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<ResponseTransform>,
    /// Closed schema required of the canonical JSON value.
    pub output_schema: OutputSchema,
}

/// Supported response-transform languages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "language", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResponseTransform {
    /// Luau source that evaluates to one response-transform function.
    Luau {
        /// Exact reviewed source text.
        source: String,
    },
}

/// Closed JSON-compatible output schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSchema {
    /// Exact JSON value kind.
    #[serde(rename = "type")]
    pub value_type: OutputType,
    /// Object properties; valid only for object schemas.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, OutputSchema>,
    /// Required object properties; valid only for object schemas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    /// Must be explicitly false for object schemas.
    #[serde(
        default,
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<bool>,
    /// Array item schema; valid only for array schemas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<OutputSchema>>,
    /// Maximum UTF-8 bytes; required only for string schemas.
    #[serde(default, rename = "maxBytes", skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
    /// Maximum elements; required only for array schemas.
    #[serde(default, rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
}

/// JSON kinds accepted by response output schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    /// JSON object.
    Object,
    /// JSON array.
    Array,
    /// JSON string.
    String,
    /// JSON integer.
    Integer,
    /// JSON number.
    Number,
    /// JSON boolean.
    Boolean,
    /// JSON null.
    Null,
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
    /// Reviewed model-facing guidance for supplying this argument.
    pub description: String,
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

/// Pagination and continuation semantics known to the definition model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PaginationPolicy {
    /// The operation returns one bounded page.
    None,
    /// A response field supplies the next request token.
    ResponseToken {
        /// Response JSON pointer containing the opaque token.
        response_pointer: String,
        /// Request query argument receiving the token.
        request_argument: String,
        /// Optional reviewed fixed provider page size.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page_size: Option<PageSizePolicy>,
    },
}

/// Reviewed fixed provider page-size shaping, never exposed as model input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSizePolicy {
    /// Provider query argument receiving the fixed value.
    pub request_argument: String,
    /// Exact reviewed positive page size.
    pub value: u32,
}
