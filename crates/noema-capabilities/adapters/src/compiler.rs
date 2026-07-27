//! Deterministic manifest validation and compilation.

use crate::{
    AdapterManifestV3, AdapterOperation, ArgumentLocation, ArgumentType, HttpMethod,
    PaginationPolicy, RetryPolicy,
    credential_import::validate_import_schema,
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

const COMPILER_VERSION: &str = "adapter-compiler-v3";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_OPERATIONS: usize = 256;
const MAX_ARGUMENTS: usize = 128;
const MAX_ID_BYTES: usize = 96;
const MAX_SCOPE_BYTES: usize = 256;
const MAX_HEADER_BYTES: usize = 4_096;
const MAX_TOKEN_BYTES: usize = 160;
const MAX_SETUP_URL_BYTES: usize = 2_048;

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
    /// Exact definition revision.
    pub definition_revision: String,
    /// Whether this exact manifest was reviewed.
    pub reviewed: bool,
    /// Fixed request origin shared by every operation.
    pub origin: String,
    /// Credential family/scopes retained as immutable connection input.
    pub authentication: crate::AuthenticationRequirement,
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
    /// Exact validated wire argument mappings.
    pub arguments: Vec<crate::ArgumentDefinition>,
    /// Provider-visible schema with no auth/runtime fields.
    pub input_schema: Value,
    /// Complete effective behavior for the definition's proposed policy.
    pub behavior: CapabilityToolBehavior,
    /// Provenance-bearing proposed policy for this exact source revision.
    pub tool_policy: CapabilityToolPolicy,
    /// Exact safe retry contract.
    pub retry: RetryPolicy,
    /// Exact pagination contract; M1 accepts only bounded single-page plans.
    pub pagination: PaginationPolicy,
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
/// This is not invocation authority. M2 must wrap it with the exact connection,
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
        let manifest: AdapterManifestV3 =
            serde_json::from_slice(bytes).map_err(|_| AdapterCompileError::Manifest)?;
        Self::compile(&manifest)
    }

    /// Validate and deterministically compile one v3 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterCompileError`] when any authority, schema, policy, or
    /// currently unsupported workflow is unsafe or ambiguous.
    pub fn compile(
        manifest: &AdapterManifestV3,
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
        Ok(CompiledAdapterDefinition {
            definition_id: manifest.definition_id.clone(),
            adapter_id: manifest.adapter_id.clone(),
            definition_revision: manifest.definition_revision.clone(),
            reviewed: manifest.reviewed,
            origin: manifest.origin.clone(),
            authentication: manifest.authentication.clone(),
            gates: manifest.gates.clone(),
            quota: manifest.quota.clone(),
            semantic_digest,
            operations,
        })
    }

    /// Return the compiler identity stored in rebuildable projections.
    #[must_use]
    pub const fn version() -> &'static str {
        COMPILER_VERSION
    }
}

fn validate_manifest(manifest: &AdapterManifestV3) -> Result<(), AdapterCompileError> {
    if manifest.schema_version != 3 {
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

fn validate_authentication(manifest: &AdapterManifestV3) -> Result<(), AdapterCompileError> {
    if manifest.authentication.scopes.len() > 128 {
        return Err(AdapterCompileError::Invalid("authentication_scopes"));
    }
    let mut scopes = BTreeSet::new();
    for scope in &manifest.authentication.scopes {
        validate_bounded_text("authentication_scope", scope, MAX_SCOPE_BYTES)?;
        if !scopes.insert(scope) {
            return Err(AdapterCompileError::Invalid(
                "duplicate_authentication_scope",
            ));
        }
    }
    if matches!(
        manifest.authentication.mode,
        crate::AuthenticationMode::None
    ) && !manifest.authentication.scopes.is_empty()
    {
        return Err(AdapterCompileError::Invalid("authentication_scopes"));
    }
    if let Some(setup_url) = &manifest.authentication.client_setup_url {
        if manifest.authentication.mode != crate::AuthenticationMode::Oauth2AuthorizationCodePkce
            || setup_url.len() > MAX_SETUP_URL_BYTES
            || setup_url.trim() != setup_url
        {
            return Err(AdapterCompileError::Invalid("client_setup_url"));
        }
        let parsed =
            Url::parse(setup_url).map_err(|_| AdapterCompileError::Invalid("client_setup_url"))?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(AdapterCompileError::Invalid("client_setup_url"));
        }
    }
    if let Some(schema) = &manifest.authentication.credential_import {
        if manifest.authentication.mode != crate::AuthenticationMode::Oauth2AuthorizationCodePkce {
            return Err(AdapterCompileError::Invalid("credential_import_mode"));
        }
        if schema.kind != crate::CredentialImportKind::OauthClientJson {
            return Err(AdapterCompileError::Unsupported("credential_import"));
        }
        validate_import_schema(schema).map_err(AdapterCompileError::Invalid)?;
    }
    if let Some(oauth) = &manifest.authentication.oauth2 {
        if manifest.authentication.mode != crate::AuthenticationMode::Oauth2AuthorizationCodePkce {
            return Err(AdapterCompileError::Invalid("oauth2_mode"));
        }
        validate_oauth_config(oauth).map_err(AdapterCompileError::Invalid)?;
    }
    Ok(())
}

fn validate_quota(manifest: &AdapterManifestV3) -> Result<(), AdapterCompileError> {
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

fn validate_operation(operation: &AdapterOperation) -> Result<(), AdapterCompileError> {
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
    validate_behavior(operation)?;
    if operation.retry == RetryPolicy::TransportSafeRead
        && (operation.behavior.idempotent.value != Some(true)
            || operation.method != HttpMethod::Get)
    {
        return Err(AdapterCompileError::Invalid("unsafe_retry"));
    }
    validate_gates(&operation.gates)?;
    validate_headers(&operation.fixed_headers)?;
    validate_arguments(operation)
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
        arguments,
        input_schema: input_schema(operation),
        behavior,
        tool_policy,
        retry: operation.retry,
        pagination: operation.pagination.clone(),
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
