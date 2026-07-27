//! Immutable server-only capability bindings and persistence views.

use crate::{CapabilityDestination, CapabilityFuture, InvokerKey, ToolName, ToolSpec, web};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// Opaque child/runtime-owned operation token.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OperationToken(String);

impl std::fmt::Debug for OperationToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OperationToken([REDACTED])")
    }
}

impl OperationToken {
    /// Construct a server-owned operation token.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the opaque value to its owning server-side adapter.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-serializable execution target retained beside a provider-visible spec.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityTarget {
    invoker_key: InvokerKey,
    operation_token: OperationToken,
}

impl std::fmt::Debug for CapabilityTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CapabilityTarget([REDACTED])")
    }
}

impl CapabilityTarget {
    /// Construct a server-only target.
    #[must_use]
    pub fn new(invoker_key: InvokerKey, operation_token: OperationToken) -> Self {
        Self {
            invoker_key,
            operation_token,
        }
    }

    /// Return the registered invoker key.
    #[must_use]
    pub fn invoker_key(&self) -> &InvokerKey {
        &self.invoker_key
    }

    /// Return the operation token.
    #[must_use]
    pub fn operation_token(&self) -> &OperationToken {
        &self.operation_token
    }
}

/// Neutral side-effect class. Role/terminal policy remains in runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityEffect {
    /// Retrieval or inspection only.
    ReadOnly,
    /// State-changing operation.
    Mutating,
    /// External state-changing operation that requires governed admission.
    ExternalWrite,
    /// External data egress that requires governed admission.
    ExternalExport,
    /// External state change and data egress that require governed admission.
    ExternalWriteAndExport,
    /// Internal control operation.
    Internal,
}

impl CapabilityEffect {
    /// Return whether this effect must pass the governed-action gateway.
    #[must_use]
    pub const fn requires_governed_admission(self) -> bool {
        matches!(
            self,
            Self::ExternalWrite | Self::ExternalExport | Self::ExternalWriteAndExport
        )
    }
}

/// Admission route selected for a capability invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAdmissionPolicy {
    /// Execute without creating a governed action.
    Direct,
    /// Execute an external effect directly because a reviewed provider policy
    /// explicitly selected that route.
    PolicyAuthorizedDirect,
    /// Let the deterministic policy and reviewer admit the action or ask a human.
    ReviewerMayApprove,
    /// Persist the exact proposal and require human approval without model review.
    AlwaysAsk,
}

impl CapabilityAdmissionPolicy {
    /// Return whether the binding requires an exact governed admission token.
    #[must_use]
    pub const fn requires_governed_admission(self) -> bool {
        !matches!(self, Self::Direct | Self::PolicyAuthorizedDirect)
    }
}

/// Neutral ownership scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityScope {
    /// Owned by the current execution/task.
    ExecutionOwned,
    /// Owned by the current conversation.
    ConversationOwned,
    /// Global or host-wide scope.
    Global,
}

/// Neutral access metadata carried by a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityAccess {
    /// Side-effect class.
    pub effect: CapabilityEffect,
    /// Ownership scope.
    pub scope: CapabilityScope,
}

/// Explicit persisted views for arguments and output.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedCapabilityPayload {
    /// Sanitized arguments, or `None` when omitted wholesale.
    pub arguments: Option<Value>,
    /// Sanitized output, or `None` when omitted wholesale.
    pub output: Option<Value>,
}

impl PersistedCapabilityPayload {
    /// Omit both views. Used when no binding was resolved and therefore no
    /// policy exists that could safely inspect provider arguments.
    #[must_use]
    pub const fn omitted() -> Self {
        Self {
            arguments: None,
            output: None,
        }
    }
}

/// Binding-owned synchronous persistence policy.
pub trait PayloadSanitizer: Send + Sync {
    /// Produce an argument view before execution begins.
    fn persist_arguments(&self, arguments: &Value) -> Option<Value>;

    /// Produce an output view for success, tool-declared failure, or a fixed
    /// safe adapter/control-plane failure payload.
    fn persist_output(&self, output: &Value) -> Option<Value> {
        self.persist_arguments(output)
    }
}

/// Recursively redact common secret fields while retaining ordinary payloads.
#[derive(Debug, Default)]
pub struct RedactingPayloadSanitizer;

impl PayloadSanitizer for RedactingPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(redact_secret_fields(arguments))
    }
}

/// Redact sensitive fetch URLs and recursively redact other secret fields.
#[derive(Debug, Default)]
pub struct WebFetchPayloadSanitizer;

impl PayloadSanitizer for WebFetchPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(redact_secret_fields(
            &web::fetch::sanitize_payload_for_storage(arguments),
        ))
    }
}

/// Omit artifact file content and recursively redact remaining secrets.
#[derive(Debug, Default)]
pub struct ArtifactPayloadSanitizer;

impl PayloadSanitizer for ArtifactPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(redact_secret_fields(&omit_artifact_content(arguments)))
    }
}

/// Omit arguments and outputs wholesale, as required for MCP payloads.
#[derive(Debug, Default)]
pub struct OmitPayloadSanitizer;

impl PayloadSanitizer for OmitPayloadSanitizer {
    fn persist_arguments(&self, _arguments: &Value) -> Option<Value> {
        None
    }
}

/// One immutable capability binding.
#[derive(Clone)]
pub struct CapabilityBinding {
    spec: ToolSpec,
    target: CapabilityTarget,
    access: CapabilityAccess,
    admission_policy: CapabilityAdmissionPolicy,
    destination: Option<CapabilityDestination>,
    sanitizer: Arc<dyn PayloadSanitizer>,
}

impl std::fmt::Debug for CapabilityBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityBinding")
            .field("spec", &self.spec)
            .field("target", &self.target)
            .field("access", &self.access)
            .field("admission_policy", &self.admission_policy)
            .field("destination", &self.destination)
            .finish_non_exhaustive()
    }
}

impl CapabilityBinding {
    /// Construct a binding from provider-visible semantics and server-only authority.
    #[must_use]
    pub fn new(
        spec: ToolSpec,
        target: CapabilityTarget,
        access: CapabilityAccess,
        sanitizer: Arc<dyn PayloadSanitizer>,
    ) -> Self {
        let admission_policy = if access.effect.requires_governed_admission() {
            CapabilityAdmissionPolicy::ReviewerMayApprove
        } else {
            CapabilityAdmissionPolicy::Direct
        };
        Self {
            spec,
            target,
            access,
            admission_policy,
            destination: None,
            sanitizer,
        }
    }

    /// Override the default admission route derived from the effect.
    #[must_use]
    pub const fn with_admission_policy(mut self, policy: CapabilityAdmissionPolicy) -> Self {
        self.admission_policy = policy;
        self
    }

    /// Pin the exact non-secret destination used by this binding.
    #[must_use]
    pub fn with_destination(mut self, destination: CapabilityDestination) -> Self {
        self.destination = Some(destination);
        self
    }

    /// Return the canonical provider-visible specification.
    #[must_use]
    pub fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    /// Return the server-only target.
    #[must_use]
    pub fn target(&self) -> &CapabilityTarget {
        &self.target
    }

    /// Return neutral access metadata.
    #[must_use]
    pub const fn access(&self) -> CapabilityAccess {
        self.access
    }

    /// Return the admission route selected for this binding.
    #[must_use]
    pub const fn admission_policy(&self) -> CapabilityAdmissionPolicy {
        self.admission_policy
    }

    /// Return the exact non-secret destination, when the operation is
    /// connection-backed.
    #[must_use]
    pub const fn destination(&self) -> Option<&CapabilityDestination> {
        self.destination.as_ref()
    }

    /// Produce persisted argument and output views.
    #[must_use]
    pub fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        self.sanitizer.persist_arguments(arguments)
    }

    /// Produce a persisted output view for every output path.
    #[must_use]
    pub fn persist_output(&self, output: &Value) -> Option<Value> {
        self.sanitizer.persist_output(output)
    }
}

/// Immutable request-local catalog that retains exact execution authority.
#[derive(Debug, Clone, Default)]
pub struct CapabilityCatalogSnapshot {
    entries: Arc<Vec<CapabilityBinding>>,
    by_canonical_name: Arc<BTreeMap<String, usize>>,
}

impl CapabilityCatalogSnapshot {
    /// Resolve only a canonical name advertised in this snapshot.
    #[must_use]
    pub fn resolve(&self, canonical_name: &str) -> Option<&CapabilityBinding> {
        self.by_canonical_name
            .get(canonical_name)
            .and_then(|index| self.entries.get(*index))
    }

    /// Return canonical provider-visible specs with no execution authority.
    /// Provider adapters own request-local dialect lowering and map-back.
    #[must_use]
    pub fn provider_specs(&self) -> Vec<ToolSpec> {
        self.entries
            .iter()
            .map(|binding| binding.spec.clone())
            .collect()
    }

    /// Iterate bindings in stable insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &CapabilityBinding> {
        self.entries.iter()
    }

    /// Return the number of bindings in the snapshot.
    #[must_use]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Builder that rejects duplicate canonical names.
#[derive(Debug, Default)]
pub struct CapabilityCatalogBuilder {
    entries: Vec<CapabilityBinding>,
    by_canonical_name: BTreeMap<String, usize>,
}

impl CapabilityCatalogBuilder {
    /// Construct an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one canonical binding.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityCatalogError::DuplicateCanonicalName`] if the name
    /// was already added.
    pub fn add(&mut self, binding: CapabilityBinding) -> Result<(), CapabilityCatalogError> {
        let canonical_name = binding.spec.name.as_str().to_string();
        if self.by_canonical_name.contains_key(&canonical_name) {
            return Err(CapabilityCatalogError::DuplicateCanonicalName);
        }
        self.by_canonical_name
            .insert(canonical_name, self.entries.len());
        self.entries.push(binding);
        Ok(())
    }

    /// Freeze the catalog into an immutable snapshot.
    #[must_use]
    pub fn build(self) -> CapabilityCatalogSnapshot {
        CapabilityCatalogSnapshot {
            entries: Arc::new(self.entries),
            by_canonical_name: Arc::new(self.by_canonical_name),
        }
    }
}

/// Catalog construction error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityCatalogError {
    /// Two entries share a canonical operation name.
    #[error("duplicate canonical capability name")]
    DuplicateCanonicalName,
}

/// Safe source failure categories.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityBindingSourceError {
    /// Catalog is temporarily unavailable.
    #[error("capability catalog is unavailable")]
    Unavailable,
    /// Catalog data violates an internal invariant.
    #[error("capability catalog is invalid")]
    Invalid,
}

/// Safe availability status supplied alongside a catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAvailabilityStatus {
    /// Capability is temporarily unavailable.
    Unavailable,
    /// Capability needs user authentication.
    AuthenticationRequired,
    /// Capability is disabled by reviewed policy.
    Disabled,
}

/// Typed availability notice. It cannot carry repository, transport, or raw
/// diagnostic strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAvailabilityNotice {
    /// Canonical capability name, if one can safely be named.
    pub capability: Option<ToolName>,
    /// Safe availability category.
    pub status: CapabilityAvailabilityStatus,
}

/// One source result with safe availability notices.
#[derive(Debug, Clone, Default)]
pub struct CapabilityCatalogResult {
    /// Immutable bindings available for this provider request.
    pub snapshot: CapabilityCatalogSnapshot,
    /// Safe, model-visible availability notices.
    pub availability_notices: Vec<CapabilityAvailabilityNotice>,
}

/// Object-safe request-local binding source.
pub trait CapabilityBindingSource: Send + Sync {
    /// Load one immutable catalog snapshot.
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>>;
}

/// Clonable source handle.
pub type CapabilityBindingSourceHandle = Arc<dyn CapabilityBindingSource>;

const SENSITIVE_FIELDS: &str =
    "authorization api_key apikey access_token refresh_token password secret cookie";

fn redact_secret_fields(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase();
                    (
                        key.clone(),
                        if SENSITIVE_FIELDS
                            .split_whitespace()
                            .any(|needle| normalized.contains(needle))
                        {
                            Value::String("[REDACTED]".to_string())
                        } else {
                            redact_secret_fields(value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_secret_fields).collect()),
        _ => value.clone(),
    }
}

fn omit_artifact_content(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let sanitized = if key == "versions" {
                        omit_artifact_version_contents(value)
                    } else {
                        omit_artifact_content(value)
                    };
                    (key.clone(), sanitized)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(omit_artifact_content).collect()),
        _ => value.clone(),
    }
}

fn omit_artifact_version_contents(value: &Value) -> Value {
    let Value::Array(versions) = value else {
        return omit_artifact_content(value);
    };
    Value::Array(
        versions
            .iter()
            .map(|version| match version {
                Value::Object(object) => Value::Object(
                    object
                        .iter()
                        .map(|(key, value)| {
                            (
                                key.clone(),
                                if key == "content" {
                                    json!({
                                        "omitted": true,
                                        "character_count": value.as_str().map(|text| text.chars().count())
                                    })
                                } else {
                                    omit_artifact_content(value)
                                },
                            )
                        })
                        .collect(),
                ),
                _ => omit_artifact_content(version),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapabilityEffect, CapabilityScope};

    macro_rules! assert_json_fields {
        ($value:expr, $($pointer:literal => $expected:expr),+ $(,)?) => {{
            let value = &$value;
            $(assert_eq!(value.pointer($pointer), Some(&json!($expected)), "{}", $pointer);)+
        }};
    }

    fn binding(name: &str, sanitizer: Arc<dyn PayloadSanitizer>) -> CapabilityBinding {
        CapabilityBinding::new(
            ToolSpec::new(name, "Test operation.", json!({"type":"object"})).expect("spec"),
            CapabilityTarget::new(InvokerKey::new("test"), OperationToken::new(name)),
            CapabilityAccess {
                effect: CapabilityEffect::ReadOnly,
                scope: CapabilityScope::Global,
            },
            sanitizer,
        )
    }

    fn persisted_views(
        binding: &CapabilityBinding,
        arguments: &Value,
        output: &Value,
    ) -> PersistedCapabilityPayload {
        PersistedCapabilityPayload {
            arguments: binding.persist_arguments(arguments),
            output: binding.persist_output(output),
        }
    }

    #[test]
    fn renamed_mcp_binding_omits_arguments_and_outputs() {
        let mut builder = CapabilityCatalogBuilder::new();
        builder
            .add(binding("read_docs", Arc::new(OmitPayloadSanitizer)))
            .expect("entry");
        let snapshot = builder.build();
        let binding = snapshot.resolve("read_docs").expect("binding");
        let views = persisted_views(
            binding,
            &json!({"private":"workspace query"}),
            &json!({"private":"workspace result"}),
        );
        assert_eq!(views.arguments, None);
        assert_eq!(views.output, None);
    }

    #[test]
    fn web_fetch_policy_composes_url_and_recursive_secret_redaction() {
        let sanitizer = WebFetchPayloadSanitizer;
        let views = PersistedCapabilityPayload {
            arguments: sanitizer.persist_arguments(&json!({
                "url":"https://user:secret@example.com/path#token",
                "headers":{"Authorization":"Bearer private"}
            })),
            output: sanitizer.persist_output(&json!({"access_token":"private"})),
        };
        let arguments = views.arguments.expect("arguments");
        let output = views.output.expect("output");
        assert_json_fields!(arguments,
            "/url" => web::fetch::REDACTED_SENSITIVE_URL,
            "/headers/Authorization" => "[REDACTED]",
        );
        assert_json_fields!(output, "/access_token" => "[REDACTED]");
        assert!(!arguments.to_string().contains("private"));
    }

    #[test]
    fn artifact_policy_omits_content_and_redacts_secrets_on_both_paths() {
        let binding = binding(
            "artifact.create_local_file",
            Arc::new(ArtifactPayloadSanitizer),
        );
        let views = persisted_views(
            &binding,
            &json!({
                "filename": "draft.md",
                "api_key": "private-argument",
                "versions": [{"title": "Draft", "content": "argument body"}]
            }),
            &json!({
                "artifact_id": "artifact:1",
                "access_token": "private-output",
                "versions": [{"title": "Saved", "content": "output body"}]
            }),
        );
        let arguments = views.arguments.expect("argument view");
        let output = views.output.expect("output view");

        assert_json_fields!(arguments,
            "/filename" => "draft.md",
            "/versions/0/title" => "Draft",
            "/versions/0/content" => json!({"omitted": true, "character_count": 13}),
            "/api_key" => "[REDACTED]",
        );
        assert_json_fields!(output,
            "/artifact_id" => "artifact:1",
            "/versions/0/title" => "Saved",
            "/versions/0/content" => json!({"omitted": true, "character_count": 11}),
            "/access_token" => "[REDACTED]",
        );
        assert!(!arguments.to_string().contains("argument body"));
        assert!(!output.to_string().contains("output body"));

        let nested_views = persisted_views(
            &binding,
            &json!({
                "arguments": {
                    "filename": "nested.md",
                    "password": "private-nested-argument",
                    "versions": [{"title": "Nested draft", "content": "nested argument body"}]
                }
            }),
            &json!({
                "result": {
                    "artifact_id": "artifact:2",
                    "cookie": "private-nested-output",
                    "versions": [{"title": "Nested saved", "content": "nested output body"}]
                }
            }),
        );
        let nested_arguments = nested_views.arguments.expect("nested argument view");
        let nested_output = nested_views.output.expect("nested output view");
        assert_json_fields!(nested_arguments,
            "/arguments/filename" => "nested.md",
            "/arguments/versions/0/content" => json!({"omitted": true, "character_count": 20}),
            "/arguments/password" => "[REDACTED]",
        );
        assert_json_fields!(nested_output,
            "/result/artifact_id" => "artifact:2",
            "/result/versions/0/content" => json!({"omitted": true, "character_count": 18}),
            "/result/cookie" => "[REDACTED]",
        );
        assert!(
            !nested_arguments
                .to_string()
                .contains("nested argument body")
        );
        assert!(!nested_output.to_string().contains("nested output body"));
    }

    #[test]
    fn catalog_rejects_duplicate_authority_names() {
        let mut builder = CapabilityCatalogBuilder::new();
        builder
            .add(binding("one", Arc::new(RedactingPayloadSanitizer)))
            .expect("first");
        assert_eq!(
            builder
                .add(binding("one", Arc::new(RedactingPayloadSanitizer)))
                .expect_err("duplicate canonical name"),
            CapabilityCatalogError::DuplicateCanonicalName
        );
        builder
            .add(binding("two", Arc::new(RedactingPayloadSanitizer)))
            .expect("builder remains reusable after rejection");
    }
}
