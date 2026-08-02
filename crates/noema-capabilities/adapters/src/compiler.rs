//! Deterministic manifest validation and compilation.

use crate::{
    AdapterManifestV5, AdapterOperation, ArgumentLocation, ArgumentType, CredentialInput,
    HttpMethod, PaginationPolicy, RetryPolicy,
    digest::{
        OperationDigest, SemanticDigest, canonical_json_bytes, semantic_manifest_value,
        semantic_operation_value,
    },
    oauth::validate_oauth_config,
};
use noema_capabilities::{
    CapabilityToolBehavior, CapabilityToolHintSource, CapabilityToolPolicy,
    CapabilityToolPolicyStatus, apply_tool_safe_defaults,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use url::Url;

const COMPILER_VERSION: &str = "adapter-compiler-v5";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_OPERATIONS: usize = 256;
const MAX_ARGUMENTS: usize = 128;
const MAX_ID_BYTES: usize = 96;
const MAX_SCOPE_BYTES: usize = 256;
const MAX_HEADER_BYTES: usize = 4_096;
const MAX_TOKEN_BYTES: usize = 160;
const MAX_SETUP_URL_BYTES: usize = 2_048;
const MAX_JSON_BODY_TEMPLATE_BYTES: usize = 256 * 1024;
const MAX_JSON_BODY_TEMPLATE_DEPTH: usize = 12;
const MAX_JSON_BODY_TEMPLATE_NODES: usize = 256;

/// Deterministic provider-neutral definition compiler.
#[derive(Debug, Default)]
pub struct AdapterCompiler;

/// Immutable, connection-independent compiled definition.
#[derive(Debug, Clone)]
pub struct CompiledAdapterDefinition {
    /// Stable manifest identity.
    pub definition_id: String,
    /// Stable adapter family identity.
    pub adapter_id: String,
    /// Optional human-facing service name, excluded from semantic authority.
    pub display_name: Option<String>,
    /// Exact definition revision.
    pub definition_revision: String,
    /// Whether this exact manifest was reviewed.
    pub reviewed: bool,
    /// Fixed request origin shared by every operation.
    pub origin: String,
    /// Credential family/scopes retained as immutable connection input.
    pub authentication: crate::AuthenticationSchemeV4,
    /// Definition-level account/product gates.
    pub gates: Vec<crate::AccountGate>,
    /// Definition-level economics and quota metadata.
    pub quota: crate::QuotaPolicy,
    /// Content address of execution/security semantics.
    pub semantic_digest: SemanticDigest,
    /// Stable operations sorted by operation identity.
    pub operations: Vec<CompiledOperation>,
}

/// One immutable connection-independent operation plan.
#[derive(Debug, Clone)]
pub struct CompiledOperation {
    /// Stable operation identity.
    pub operation_id: String,
    /// Fixed request method.
    pub method: HttpMethod,
    /// Fixed-origin relative request path.
    pub path: String,
    /// Fixed reviewed non-secret headers.
    pub fixed_headers: BTreeMap<String, String>,
    /// Fixed reviewed non-secret query parameters.
    pub fixed_query: BTreeMap<String, String>,
    /// Exact validated wire argument mappings.
    pub arguments: Vec<crate::ArgumentDefinition>,
    /// Optional reviewed nested JSON body shape.
    pub json_body_template: Option<Value>,
    /// Provider-visible schema with no auth/runtime fields.
    pub input_schema: Value,
    /// Complete effective behavior for the definition's proposed policy.
    pub behavior: CapabilityToolBehavior,
    /// Provenance-bearing proposed policy for this exact source revision.
    pub tool_policy: CapabilityToolPolicy,
    /// Exact safe retry contract.
    pub retry: RetryPolicy,
    /// Exact reviewed pagination contract.
    pub pagination: PaginationPolicy,
    /// Reviewed bounded successful-response contract.
    pub response: crate::ResponseContract,
    /// Operation-specific account/product gates.
    pub gates: Vec<crate::AccountGate>,
    /// Digest of this operation's execution/security semantics.
    pub operation_digest: OperationDigest,
    /// Bounded versioned definition authority token.
    pub token: DefinitionOperationToken,
}

impl CompiledAdapterDefinition {
    /// Classify whether another compiled definition can retain review.
    #[must_use]
    pub fn semantic_change_from(&self, previous: &Self) -> SemanticChange {
        if self.semantic_digest == previous.semantic_digest {
            SemanticChange::DocumentationOnly
        } else {
            SemanticChange::RequiresReview
        }
    }
}

/// Review consequence of comparing two canonical definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticChange {
    /// Only excluded labels, prose, or provenance changed.
    DocumentationOnly,
    /// Execution/security semantics changed and require fresh review.
    RequiresReview,
}

/// Stable bounded per-connection tool-name slug.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionSlug(String);

impl ConnectionSlug {
    /// Validate one stable connection slug.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterCompileError`] unless the slug is a bounded stable ID.
    pub fn new(value: impl Into<String>) -> Result<Self, AdapterCompileError> {
        let value = value.into();
        validate_id("connection_slug", &value)?;
        Ok(Self(value))
    }

    /// Return the stable slug.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Bounded opaque token identifying one definition operation plan.
///
/// This is not invocation authority. Invocation wraps it with the exact connection,
/// credential, grant, and policy revisions before constructing a binding.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DefinitionOperationToken(String);

impl DefinitionOperationToken {
    /// Return the non-secret versioned token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for DefinitionOperationToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DefinitionOperationToken([OPAQUE])")
    }
}

/// Safe, bounded definition compiler failure.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AdapterCompileError {
    /// Manifest JSON is malformed, oversized, or contains unknown fields.
    #[error("adapter manifest is invalid")]
    Manifest,
    /// A closed manifest invariant failed.
    #[error("adapter manifest field is invalid: {0}")]
    Invalid(&'static str),
    /// A parsed feature is intentionally unsupported by the current runtime.
    #[error("adapter feature is unsupported: {0}")]
    Unsupported(&'static str),
}

impl AdapterCompiler {
    /// Parse and compile bounded canonical manifest JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterCompileError`] for malformed input or a rejected plan.
    pub fn compile_json(bytes: &[u8]) -> Result<CompiledAdapterDefinition, AdapterCompileError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(AdapterCompileError::Manifest);
        }
        let manifest: AdapterManifestV5 =
            serde_json::from_slice(bytes).map_err(|_| AdapterCompileError::Manifest)?;
        Self::compile(&manifest)
    }

    /// Validate and deterministically compile one v5 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterCompileError`] when any authority, schema, policy, or
    /// currently unsupported workflow is unsafe or ambiguous.
    pub fn compile(
        manifest: &AdapterManifestV5,
    ) -> Result<CompiledAdapterDefinition, AdapterCompileError> {
        validate_manifest(manifest)?;
        let semantic_value =
            semantic_manifest_value(manifest).map_err(|_| AdapterCompileError::Manifest)?;
        let semantic_digest = SemanticDigest::compute(
            &canonical_json_bytes(&semantic_value).map_err(|_| AdapterCompileError::Manifest)?,
        );
        let mut operations = manifest.operations.iter().collect::<Vec<_>>();
        operations.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        let operations = operations
            .into_iter()
            .map(|operation| compile_operation(operation, &semantic_digest))
            .collect::<Result<Vec<_>, _>>()?;
        let compiled = CompiledAdapterDefinition {
            definition_id: manifest.definition_id.clone(),
            adapter_id: manifest.adapter_id.clone(),
            display_name: manifest.display_name.clone(),
            definition_revision: manifest.definition_revision.clone(),
            reviewed: manifest.reviewed,
            origin: manifest.origin.clone(),
            authentication: manifest.authentication.clone(),
            gates: manifest.gates.clone(),
            quota: manifest.quota.clone(),
            semantic_digest,
            operations,
        };
        validate_account_identity(manifest, &compiled)?;
        Ok(compiled)
    }

    /// Return the compiler identity stored in rebuildable projections.
    #[must_use]
    pub const fn version() -> &'static str {
        COMPILER_VERSION
    }
}

fn validate_manifest(manifest: &AdapterManifestV5) -> Result<(), AdapterCompileError> {
    if manifest.schema_version != 5 {
        return Err(AdapterCompileError::Unsupported("schema_version"));
    }
    validate_id("definition_id", &manifest.definition_id)?;
    validate_id("adapter_id", &manifest.adapter_id)?;
    validate_id("definition_revision", &manifest.definition_revision)?;
    if let Some(display_name) = &manifest.display_name {
        validate_bounded_text("display_name", display_name, MAX_SCOPE_BYTES)?;
    }
    validate_origin(&manifest.origin)?;
    if manifest.operations.is_empty() || manifest.operations.len() > MAX_OPERATIONS {
        return Err(AdapterCompileError::Invalid("operations"));
    }
    validate_authentication(manifest)?;
    validate_quota(manifest)?;
    validate_gates(&manifest.gates)?;
    let mut operation_ids = BTreeSet::new();
    for operation in &manifest.operations {
        if !operation_ids.insert(operation.operation_id.as_str()) {
            return Err(AdapterCompileError::Invalid("duplicate_operation_id"));
        }
        validate_operation(operation)?;
    }
    Ok(())
}

fn validate_origin(origin: &str) -> Result<(), AdapterCompileError> {
    if origin.contains(['{', '}']) {
        return Err(AdapterCompileError::Invalid("dynamic_origin"));
    }
    let parsed = Url::parse(origin).map_err(|_| AdapterCompileError::Invalid("origin"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return Err(AdapterCompileError::Invalid("origin"));
    }
    Ok(())
}

fn validate_authentication(manifest: &AdapterManifestV5) -> Result<(), AdapterCompileError> {
    if manifest.authentication.scopes().len() > 128 {
        return Err(AdapterCompileError::Invalid("authentication_scopes"));
    }
    let mut scopes = BTreeSet::new();
    for scope in manifest.authentication.scopes() {
        validate_bounded_text("authentication_scope", scope, MAX_SCOPE_BYTES)?;
        if !scopes.insert(scope) {
            return Err(AdapterCompileError::Invalid(
                "duplicate_authentication_scope",
            ));
        }
    }
    match &manifest.authentication {
        crate::AuthenticationSchemeV4::None => {}
        crate::AuthenticationSchemeV4::Credential(config) => {
            validate_credential_setup(&config.setup)?;
            validate_luau("request_auth", &config.request_auth)?;
        }
        crate::AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(config) => {
            validate_oauth_config(config).map_err(AdapterCompileError::Invalid)?;
            if config.setups.is_empty() || config.setups.len() > 2 {
                return Err(AdapterCompileError::Invalid("oauth2_setups"));
            }
            let mut modes = BTreeSet::new();
            for setup in &config.setups {
                if !modes.insert(setup.callback_mode) {
                    return Err(AdapterCompileError::Invalid("oauth2_setups"));
                }
                validate_credential_setup(&setup.setup)?;
                let CredentialInput::Document { fields, .. } = &setup.setup.input else {
                    return Err(AdapterCompileError::Invalid("oauth2_setup_input"));
                };
                let ids = fields
                    .iter()
                    .map(|field| field.id.as_str())
                    .collect::<BTreeSet<_>>();
                let expected =
                    if config.client_authentication == crate::Oauth2ClientAuthentication::None {
                        BTreeSet::from(["client_id"])
                    } else {
                        BTreeSet::from(["client_id", "client_secret"])
                    };
                if ids != expected {
                    return Err(AdapterCompileError::Invalid("oauth2_credential_fields"));
                }
            }
        }
    }
    Ok(())
}

fn validate_credential_setup(setup: &crate::CredentialSetup) -> Result<(), AdapterCompileError> {
    validate_bounded_text("credential_type", &setup.credential_type, 128)?;
    validate_setup_url(&setup.setup_url)?;
    if setup.instructions.is_empty() || setup.instructions.len() > 8 {
        return Err(AdapterCompileError::Invalid("credential_instructions"));
    }
    for instruction in &setup.instructions {
        validate_bounded_text("credential_instruction", instruction, 512)?;
    }
    let fields = setup.input.fields();
    if fields.is_empty() || fields.len() > 16 {
        return Err(AdapterCompileError::Invalid("credential_fields"));
    }
    let mut ids = BTreeSet::new();
    for field in fields {
        validate_id("credential_field", &field.id)?;
        validate_bounded_text("credential_field_label", &field.label, 128)?;
        if !ids.insert(field.id.as_str()) {
            return Err(AdapterCompileError::Invalid("credential_fields"));
        }
    }
    if let CredentialInput::Document {
        media_type,
        normalize,
        ..
    } = &setup.input
    {
        if media_type != "application/json" {
            return Err(AdapterCompileError::Unsupported(
                "credential_document_media_type",
            ));
        }
        validate_luau("credential_normalize", normalize)?;
    }
    Ok(())
}

fn validate_setup_url(value: &str) -> Result<(), AdapterCompileError> {
    if value.len() > MAX_SETUP_URL_BYTES || value.trim() != value {
        return Err(AdapterCompileError::Invalid("credential_setup_url"));
    }
    let parsed =
        Url::parse(value).map_err(|_| AdapterCompileError::Invalid("credential_setup_url"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AdapterCompileError::Invalid("credential_setup_url"));
    }
    Ok(())
}

fn validate_luau(
    field: &'static str,
    transform: &crate::LuauTransform,
) -> Result<(), AdapterCompileError> {
    let source = transform.source();
    if source.is_empty()
        || source.len() > 32 * 1024
        || source
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        || crate::luau::validate_source(source).is_err()
    {
        return Err(AdapterCompileError::Invalid(field));
    }
    Ok(())
}

fn validate_account_identity(
    manifest: &AdapterManifestV5,
    compiled: &CompiledAdapterDefinition,
) -> Result<(), AdapterCompileError> {
    let Some(probe) = manifest.authentication.account_identity() else {
        return Ok(());
    };
    let declared_operation = manifest
        .operations
        .iter()
        .find(|operation| operation.operation_id == probe.operation_id)
        .ok_or(AdapterCompileError::Invalid("account_identity_operation"))?;
    let operation = compiled
        .operations
        .iter()
        .find(|operation| operation.operation_id == probe.operation_id)
        .ok_or(AdapterCompileError::Invalid("account_identity_operation"))?;
    if manifest.authentication.mode() != crate::AuthenticationMode::Oauth2AuthorizationCodePkce
        || operation.method != HttpMethod::Get
        || !operation.behavior.read_only
        || !operation.behavior.idempotent
        || operation.behavior.destructive
        || operation.behavior.open_world
        || operation.retry != RetryPolicy::TransportSafeRead
        || !matches!(operation.pagination, PaginationPolicy::None)
        || declared_operation.event.is_some()
        || !valid_json_pointer(&probe.output_pointer)
    {
        return Err(AdapterCompileError::Invalid("account_identity"));
    }
    crate::request::encode_request(
        compiled,
        operation,
        &Value::Object(probe.arguments.clone().into_iter().collect()),
    )
    .map_err(|_| AdapterCompileError::Invalid("account_identity_arguments"))?;
    Ok(())
}

fn valid_json_pointer(value: &str) -> bool {
    value.len() <= 256
        && (value.is_empty() || value.starts_with('/'))
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && value.as_bytes().iter().enumerate().all(|(index, byte)| {
            *byte != b'~'
                || value
                    .as_bytes()
                    .get(index + 1)
                    .is_some_and(|next| matches!(*next, b'0' | b'1'))
        })
}

fn validate_quota(manifest: &AdapterManifestV5) -> Result<(), AdapterCompileError> {
    if let Some(bucket) = &manifest.quota.bucket {
        validate_id("quota_bucket", bucket)?;
    }
    if manifest.quota.request_units == Some(0) {
        return Err(AdapterCompileError::Invalid("quota_request_units"));
    }
    Ok(())
}

fn validate_gates(gates: &[crate::AccountGate]) -> Result<(), AdapterCompileError> {
    if gates.len() > 64 {
        return Err(AdapterCompileError::Invalid("gates"));
    }
    for gate in gates {
        let value = match gate {
            crate::AccountGate::AccountKind(value)
            | crate::AccountGate::Region(value)
            | crate::AccountGate::ApiVersion(value)
            | crate::AccountGate::AuthEligibility(value)
            | crate::AccountGate::ProductTier(value)
            | crate::AccountGate::AccessReview(value)
            | crate::AccountGate::NotificationEndpoint(value) => value,
        };
        validate_bounded_text("gate", value, MAX_SCOPE_BYTES)?;
    }
    Ok(())
}

pub(crate) fn validate_operation(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
    validate_id("operation_id", &operation.operation_id)?;
    if let Some(description) = &operation.source_description {
        validate_bounded_text("source_description", description, 4_096)?;
    }
    if operation.path.len() > 2_048
        || !operation.path.starts_with('/')
        || operation.path.starts_with("//")
        || operation.path.contains("://")
        || operation.path.contains(['?', '#', '\\'])
        || operation
            .path
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
        || {
            let lower = operation.path.to_ascii_lowercase();
            lower.contains("%2e") || lower.contains("%2f") || lower.contains("%5c")
        }
        || operation.path.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(AdapterCompileError::Invalid("operation_path"));
    }
    if operation.arguments.len() > MAX_ARGUMENTS {
        return Err(AdapterCompileError::Invalid("arguments"));
    }
    if operation.event.is_some() {
        return Err(AdapterCompileError::Unsupported("event_workflow"));
    }
    crate::continuation::validate_pagination(&operation.pagination, &operation.arguments)?;
    validate_response_contract(&operation.response)?;
    if operation
        .response
        .output_schema
        .properties
        .contains_key("continuation")
    {
        return Err(AdapterCompileError::Invalid("reserved_response_field"));
    }
    if matches!(operation.pagination, PaginationPolicy::ResponseToken { .. })
        && (operation.response.transform.is_none()
            || operation.response.output_schema.value_type != crate::OutputType::Object
            || operation
                .arguments
                .iter()
                .any(|argument| argument.name == "continuation")
            || !crate::output_schema::maximum_serialized_bytes(&operation.response.output_schema)
                .is_some_and(|size| {
                    size + crate::continuation::CONTINUATION_JSON_OVERHEAD_BYTES
                        <= crate::output_schema::MAX_MODEL_RESULT_BYTES
                }))
    {
        return Err(AdapterCompileError::Invalid("pagination_response"));
    }
    validate_behavior(operation)?;
    if operation.retry == RetryPolicy::TransportSafeRead
        && (operation.behavior.idempotent.value != Some(true)
            || operation.method != HttpMethod::Get)
    {
        return Err(AdapterCompileError::Invalid("unsafe_retry"));
    }
    validate_gates(&operation.gates)?;
    validate_headers(&operation.fixed_headers)?;
    validate_fixed_query(operation)?;
    validate_arguments(operation)
}

fn validate_response_contract(
    response: &crate::ResponseContract,
) -> Result<(), AdapterCompileError> {
    if response.accepted_content_types.is_empty() || response.accepted_content_types.len() > 16 {
        return Err(AdapterCompileError::Invalid("response_content_types"));
    }
    let mut unique = BTreeSet::new();
    for content_type in &response.accepted_content_types {
        let Some((kind, subtype)) = content_type.split_once('/') else {
            return Err(AdapterCompileError::Invalid("response_content_type"));
        };
        if kind.is_empty()
            || subtype.is_empty()
            || content_type.len() > 128
            || content_type.contains(['*', ';'])
            || content_type.as_str() != content_type.to_ascii_lowercase()
            || !content_type.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(
                        byte,
                        b'/' | b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            })
            || !unique.insert(content_type)
        {
            return Err(AdapterCompileError::Invalid("response_content_type"));
        }
    }
    if let Some(crate::ResponseTransform::Luau { source }) = &response.transform {
        if source.is_empty()
            || source.len() > 32 * 1024
            || source
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            || crate::luau::validate_source(source).is_err()
        {
            return Err(AdapterCompileError::Invalid("response_transform"));
        }
    } else if response
        .accepted_content_types
        .iter()
        .any(|value| value != "application/json" && !value.ends_with("+json"))
    {
        return Err(AdapterCompileError::Invalid("response_content_type"));
    }
    if !crate::output_schema::validate(&response.output_schema) {
        return Err(AdapterCompileError::Invalid("response_schema"));
    }
    if !crate::output_schema::maximum_serialized_bytes(&response.output_schema)
        .is_some_and(|size| size <= crate::output_schema::MAX_MODEL_RESULT_BYTES)
    {
        return Err(AdapterCompileError::Invalid("response_size"));
    }
    Ok(())
}

fn validate_behavior(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
    for hint in [
        &operation.behavior.read_only,
        &operation.behavior.idempotent,
        &operation.behavior.destructive,
        &operation.behavior.open_world,
    ] {
        if hint.value.is_some() != hint.source.is_some()
            || hint.source.is_some_and(|source| {
                !matches!(
                    source,
                    CapabilityToolHintSource::Model | CapabilityToolHintSource::SafeDefault
                )
            })
        {
            return Err(AdapterCompileError::Invalid("operation_behavior"));
        }
    }
    Ok(())
}

fn validate_headers(headers: &BTreeMap<String, String>) -> Result<(), AdapterCompileError> {
    if headers.len() > 32 {
        return Err(AdapterCompileError::Invalid("fixed_headers"));
    }
    let mut normalized_names = BTreeSet::new();
    for (name, value) in headers {
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.len() > MAX_HEADER_BYTES
            || value.bytes().any(|byte| byte == b'\r' || byte == b'\n')
        {
            return Err(AdapterCompileError::Invalid("fixed_header"));
        }
        let lower = name.to_ascii_lowercase();
        if !normalized_names.insert(lower.clone())
            || !matches!(lower.as_str(), "accept" | "content-type")
            || !matches!(value.as_str(), "application/json")
        {
            return Err(AdapterCompileError::Invalid("authority_header"));
        }
    }
    Ok(())
}

fn validate_fixed_query(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
    if operation.fixed_query.len() > 32 {
        return Err(AdapterCompileError::Invalid("fixed_query"));
    }
    let model_query_names = operation
        .arguments
        .iter()
        .filter(|argument| argument.location == ArgumentLocation::Query)
        .map(|argument| argument.name.as_str())
        .collect::<BTreeSet<_>>();
    for (name, value) in &operation.fixed_query {
        validate_id("fixed_query_name", name)?;
        if value.len() > 4_096
            || value.bytes().any(|byte| byte.is_ascii_control())
            || model_query_names.contains(name.as_str())
        {
            return Err(AdapterCompileError::Invalid("fixed_query"));
        }
    }
    let collides_with_runtime = match &operation.pagination {
        PaginationPolicy::ResponseToken {
            request_argument,
            page_size,
            ..
        } => {
            operation.fixed_query.contains_key(request_argument)
                || page_size.as_ref().is_some_and(|page_size| {
                    operation
                        .fixed_query
                        .contains_key(&page_size.request_argument)
                })
        }
        PaginationPolicy::ProviderLink {
            request_argument, ..
        } => request_argument
            .as_ref()
            .is_some_and(|name| operation.fixed_query.contains_key(name)),
        PaginationPolicy::DeltaCursor {
            request_argument, ..
        } => operation.fixed_query.contains_key(request_argument),
        PaginationPolicy::None => false,
    };
    if collides_with_runtime {
        return Err(AdapterCompileError::Invalid("fixed_query"));
    }
    Ok(())
}

fn validate_arguments(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
    let mut names = BTreeSet::new();
    let mut path_names = BTreeSet::new();
    for argument in &operation.arguments {
        validate_id("argument_name", &argument.name)?;
        if !names.insert(argument.name.as_str()) {
            return Err(AdapterCompileError::Invalid("duplicate_argument"));
        }
        if argument.location == ArgumentLocation::Path {
            if !argument.required {
                return Err(AdapterCompileError::Invalid("optional_path_argument"));
            }
            path_names.insert(argument.name.as_str());
        }
        if !argument.enum_values.is_empty() && argument.argument_type != ArgumentType::String {
            return Err(AdapterCompileError::Invalid("argument_enum"));
        }
        if argument.enum_values.len() > 128 {
            return Err(AdapterCompileError::Invalid("argument_enum"));
        }
        for value in &argument.enum_values {
            validate_bounded_text("argument_enum", value, MAX_SCOPE_BYTES)?;
        }
    }
    let placeholders = path_placeholders(&operation.path)?;
    if placeholders != path_names {
        return Err(AdapterCompileError::Invalid("path_arguments"));
    }
    validate_json_body_template(operation)?;
    Ok(())
}

fn validate_json_body_template(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
    let Some(template) = &operation.json_body_template else {
        return Ok(());
    };
    if !template.is_object()
        || serde_json::to_vec(template)
            .map_err(|_| AdapterCompileError::Manifest)?
            .len()
            > MAX_JSON_BODY_TEMPLATE_BYTES
    {
        return Err(AdapterCompileError::Invalid("json_body_template"));
    }
    let body_arguments = operation
        .arguments
        .iter()
        .filter(|argument| argument.location == ArgumentLocation::JsonBody)
        .map(|argument| (argument.name.as_str(), argument.required))
        .collect::<BTreeMap<_, _>>();
    let mut references = BTreeMap::new();
    let mut nodes = 0;
    validate_json_body_value(template, 0, &mut nodes, &body_arguments, &mut references)?;
    if references.len() != body_arguments.len()
        || references
            .iter()
            .any(|(name, count)| *count != 1 || body_arguments.get(name) != Some(&true))
    {
        return Err(AdapterCompileError::Invalid("json_body_template_arguments"));
    }
    Ok(())
}

fn validate_json_body_value<'a>(
    value: &'a Value,
    depth: usize,
    nodes: &mut usize,
    body_arguments: &BTreeMap<&'a str, bool>,
    references: &mut BTreeMap<&'a str, usize>,
) -> Result<(), AdapterCompileError> {
    *nodes += 1;
    if depth > MAX_JSON_BODY_TEMPLATE_DEPTH || *nodes > MAX_JSON_BODY_TEMPLATE_NODES {
        return Err(AdapterCompileError::Invalid("json_body_template"));
    }
    match value {
        Value::Object(object) if object.contains_key("$argument") => {
            let Some(name) = (object.len() == 1)
                .then(|| object["$argument"].as_str())
                .flatten()
            else {
                return Err(AdapterCompileError::Invalid(
                    "json_body_template_placeholder",
                ));
            };
            let Some((name, _)) = body_arguments.get_key_value(name) else {
                return Err(AdapterCompileError::Invalid("json_body_template_argument"));
            };
            *references.entry(name).or_default() += 1;
        }
        Value::Object(object) => {
            for value in object.values() {
                validate_json_body_value(value, depth + 1, nodes, body_arguments, references)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_json_body_value(value, depth + 1, nodes, body_arguments, references)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn path_placeholders(path: &str) -> Result<BTreeSet<&str>, AdapterCompileError> {
    let mut placeholders = BTreeSet::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        if rest[..start].contains('}') {
            return Err(AdapterCompileError::Invalid("operation_path"));
        }
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .ok_or(AdapterCompileError::Invalid("operation_path"))?;
        let name = &after[..end];
        validate_id("path_argument", name)?;
        if !placeholders.insert(name)
            || after[end + 1..].contains('}') && !after[end + 1..].contains('{')
        {
            return Err(AdapterCompileError::Invalid("operation_path"));
        }
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return Err(AdapterCompileError::Invalid("operation_path"));
    }
    Ok(placeholders)
}

fn compile_operation(
    operation: &AdapterOperation,
    semantic_digest: &SemanticDigest,
) -> Result<CompiledOperation, AdapterCompileError> {
    let operation_value =
        semantic_operation_value(operation).map_err(|_| AdapterCompileError::Manifest)?;
    let operation_digest = OperationDigest::compute(
        &canonical_json_bytes(&json!({
            "definition": semantic_digest.as_str(),
            "operation": operation_value,
        }))
        .map_err(|_| AdapterCompileError::Manifest)?,
    );
    let token = DefinitionOperationToken(format!(
        "adp1:{}:{}",
        semantic_digest.as_str(),
        operation_digest.as_str()
    ));
    if token.0.len() > MAX_TOKEN_BYTES {
        return Err(AdapterCompileError::Invalid("operation_token"));
    }
    let mut arguments = operation.arguments.clone();
    arguments.sort_by(|left, right| left.name.cmp(&right.name));
    let status = if [
        operation.behavior.read_only.value,
        operation.behavior.idempotent.value,
        operation.behavior.destructive.value,
        operation.behavior.open_world.value,
    ]
    .into_iter()
    .all(|value| value.is_some())
    {
        CapabilityToolPolicyStatus::Ready
    } else {
        CapabilityToolPolicyStatus::Pending
    };
    let proposed_policy = CapabilityToolPolicy {
        tool_id: operation.operation_id.clone(),
        read_only: operation.behavior.read_only.clone(),
        idempotent: operation.behavior.idempotent.clone(),
        destructive: operation.behavior.destructive.clone(),
        open_world: operation.behavior.open_world.clone(),
        status,
        policy_revision: 1,
        source_revision: operation_digest.to_string(),
    };
    let tool_policy = if proposed_policy.is_callable() {
        proposed_policy
    } else {
        apply_tool_safe_defaults(proposed_policy)
    };
    let behavior = tool_policy
        .behavior()
        .ok_or(AdapterCompileError::Invalid("operation_behavior"))?;
    Ok(CompiledOperation {
        operation_id: operation.operation_id.clone(),
        method: operation.method,
        path: operation.path.clone(),
        fixed_headers: operation.fixed_headers.clone(),
        fixed_query: operation.fixed_query.clone(),
        arguments,
        json_body_template: operation.json_body_template.clone(),
        input_schema: input_schema(operation),
        behavior,
        tool_policy,
        retry: operation.retry,
        pagination: operation.pagination.clone(),
        response: operation.response.clone(),
        gates: operation.gates.clone(),
        operation_digest,
        token,
    })
}

fn input_schema(operation: &AdapterOperation) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut arguments = operation.arguments.iter().collect::<Vec<_>>();
    arguments.sort_by(|left, right| left.name.cmp(&right.name));
    for argument in arguments {
        let mut schema = Map::new();
        match argument.argument_type {
            ArgumentType::String => {
                schema.insert("type".to_string(), json!("string"));
            }
            ArgumentType::Integer => {
                schema.insert("type".to_string(), json!("integer"));
            }
            ArgumentType::Number => {
                schema.insert("type".to_string(), json!("number"));
            }
            ArgumentType::Boolean => {
                schema.insert("type".to_string(), json!("boolean"));
            }
            ArgumentType::StringArray => {
                schema.insert("type".to_string(), json!("array"));
                schema.insert("items".to_string(), json!({"type":"string"}));
            }
        }
        if !argument.enum_values.is_empty() {
            let mut values = argument.enum_values.clone();
            values.sort();
            schema.insert("enum".to_string(), json!(values));
        }
        properties.insert(argument.name.clone(), Value::Object(schema));
        if argument.required {
            required.push(argument.name.clone());
        }
    }
    if matches!(operation.pagination, PaginationPolicy::ResponseToken { .. }) {
        properties.insert(
            "continuation".to_string(),
            json!({"type": "string", "maxLength": crate::continuation::MAX_REFERENCE_BYTES}),
        );
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn validate_id(field: &'static str, value: &str) -> Result<(), AdapterCompileError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value.trim() != value
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(AdapterCompileError::Invalid(field));
    }
    Ok(())
}

fn validate_bounded_text(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), AdapterCompileError> {
    if value.is_empty()
        || value.len() > max
        || value.trim() != value
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(AdapterCompileError::Invalid(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
