//! Immutable server-only capability bindings and persistence views.

use crate::{
    CapabilityDestination, CapabilityFuture, InvokerKey, ToolName, ToolSpec,
    normalize_capability_connection_label, sanitize_standard_credentials, web,
};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// Opaque child/runtime-owned operation token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OperationToken(String);

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityTarget {
    invoker_key: InvokerKey,
    operation_token: OperationToken,
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

/// Provider-visible behavior hints used by execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityToolBehavior {
    /// The operation does not modify state.
    pub read_only: bool,
    /// Repeating the operation with identical arguments has no additional effect.
    pub idempotent: bool,
    /// The operation can perform a destructive change.
    pub destructive: bool,
    /// The operation may interact with an unbounded external world.
    pub open_world: bool,
}

impl CapabilityToolBehavior {
    /// Return whether the original trust policy considers this operation risky.
    #[must_use]
    pub const fn is_risky(self) -> bool {
        !self.read_only && (self.destructive || self.open_world)
    }
}

/// Execution route selected for a capability invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityExecutionDecision {
    /// Execute without review.
    ExecuteImmediately,
    /// Persist the exact proposal and require human review.
    HumanReview,
    /// Persist the exact proposal and let the reviewer decide whether to execute.
    LlmReview,
}

impl CapabilityExecutionDecision {
    /// Return whether the binding requires exact reviewed authorization.
    #[must_use]
    pub const fn requires_review(self) -> bool {
        !matches!(self, Self::ExecuteImmediately)
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

/// Source-owned check for one tool's executable input rules.
pub trait ToolInputCheck: Send + Sync {
    /// Return whether the exact source binding accepts these arguments.
    fn accepts(&self, arguments: &Value) -> bool;
}

impl<F> ToolInputCheck for F
where
    F: Fn(&Value) -> bool + Send + Sync,
{
    fn accepts(&self, arguments: &Value) -> bool {
        self(arguments)
    }
}

/// Remove exact standard credential fields while retaining ordinary payloads.
#[derive(Debug, Default)]
pub struct RedactingPayloadSanitizer;

impl PayloadSanitizer for RedactingPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(sanitize_standard_credentials(arguments))
    }
}

/// Sanitize credential-bearing URL components and standard credential fields.
#[derive(Debug, Default)]
pub struct UrlPayloadSanitizer;

impl PayloadSanitizer for UrlPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(sanitize_standard_credentials(
            &web::fetch::sanitize_payload_for_storage(arguments),
        ))
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
    behavior: CapabilityToolBehavior,
    execution_decision: CapabilityExecutionDecision,
    scope: CapabilityScope,
    destination: Option<CapabilityDestination>,
    service_context: Option<CapabilityServiceContext>,
    input_check: Arc<dyn ToolInputCheck>,
    sanitizer: Arc<dyn PayloadSanitizer>,
}

/// Bounded model-facing identity for one external service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityServiceContext {
    display_name: String,
    description: Option<String>,
    connection_label: Option<String>,
}

impl CapabilityServiceContext {
    /// Construct one bounded service identity.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityServiceContextError`] when either value is blank,
    /// oversized, or contains control characters.
    pub fn new(
        display_name: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Result<Self, CapabilityServiceContextError> {
        Ok(Self {
            display_name: validate_service_context_text("display_name", display_name.into(), 256)?,
            description: description
                .map(Into::into)
                .map(|value| validate_service_context_text("description", value, 512))
                .transpose()?,
            connection_label: None,
        })
    }

    /// Attach a recognizable connection label without making it destination authority.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityServiceContextError`] for blank, oversized, or
    /// control-bearing labels.
    pub fn with_connection_label(
        mut self,
        connection_label: impl Into<String>,
    ) -> Result<Self, CapabilityServiceContextError> {
        self.connection_label = normalize_capability_connection_label(Some(
            connection_label.into(),
        ))
        .map_err(|error| match error {
            crate::CapabilityConnectionLabelError::TooLong => {
                CapabilityServiceContextError::TooLong("connection_label")
            }
            crate::CapabilityConnectionLabelError::Invalid => {
                CapabilityServiceContextError::Invalid("connection_label")
            }
        })?;
        Ok(self)
    }

    /// Return the human-visible service name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Return the optional discovered service description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the optional recognizable connection label.
    #[must_use]
    pub fn connection_label(&self) -> Option<&str> {
        self.connection_label.as_deref()
    }
}

/// Invalid model-facing service identity.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityServiceContextError {
    /// A required value is blank.
    #[error("capability service context is blank: {0}")]
    Blank(&'static str),
    /// A value exceeds its model-context bound.
    #[error("capability service context is too long: {0}")]
    TooLong(&'static str),
    /// A value contains unsafe control characters.
    #[error("capability service context is invalid: {0}")]
    Invalid(&'static str),
}

fn validate_service_context_text(
    field: &'static str,
    value: String,
    max_bytes: usize,
) -> Result<String, CapabilityServiceContextError> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(CapabilityServiceContextError::Blank(field));
    }
    if value.len() > max_bytes {
        return Err(CapabilityServiceContextError::TooLong(field));
    }
    if value.chars().any(char::is_control) {
        return Err(CapabilityServiceContextError::Invalid(field));
    }
    Ok(value)
}

impl std::fmt::Debug for CapabilityBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityBinding")
            .field("spec", &self.spec)
            .field("target", &self.target)
            .field("behavior", &self.behavior)
            .field("execution_decision", &self.execution_decision)
            .field("scope", &self.scope)
            .field("destination", &self.destination)
            .field("service_context", &self.service_context)
            .finish_non_exhaustive()
    }
}

impl CapabilityBinding {
    /// Construct a binding from provider-visible semantics and server-only authority.
    #[must_use]
    pub fn new(
        spec: ToolSpec,
        target: CapabilityTarget,
        behavior: CapabilityToolBehavior,
        execution_decision: CapabilityExecutionDecision,
        scope: CapabilityScope,
        input_check: Arc<dyn ToolInputCheck>,
        sanitizer: Arc<dyn PayloadSanitizer>,
    ) -> Self {
        Self {
            spec,
            target,
            behavior,
            execution_decision,
            scope,
            destination: None,
            service_context: None,
            input_check,
            sanitizer,
        }
    }

    /// Require at least model review before this binding can execute.
    #[must_use]
    pub fn with_llm_review(mut self) -> Self {
        if self.execution_decision == CapabilityExecutionDecision::ExecuteImmediately {
            self.execution_decision = CapabilityExecutionDecision::LlmReview;
        }
        self
    }

    /// Pin the exact non-secret destination used by this binding.
    #[must_use]
    pub fn with_destination(mut self, destination: CapabilityDestination) -> Self {
        self.destination = Some(destination);
        self
    }

    /// Attach model-facing context for the exact configured destination.
    #[must_use]
    pub fn with_service_context(mut self, context: CapabilityServiceContext) -> Self {
        self.service_context = Some(context);
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

    /// Return the complete provider-visible behavior hints.
    #[must_use]
    pub const fn behavior(&self) -> CapabilityToolBehavior {
        self.behavior
    }

    /// Return the execution route selected for this binding.
    #[must_use]
    pub const fn execution_decision(&self) -> CapabilityExecutionDecision {
        self.execution_decision
    }

    /// Return the ownership scope used only for role access.
    #[must_use]
    pub const fn scope(&self) -> CapabilityScope {
        self.scope
    }

    /// Return the exact non-secret destination, when the operation is
    /// connection-backed.
    #[must_use]
    pub const fn destination(&self) -> Option<&CapabilityDestination> {
        self.destination.as_ref()
    }

    /// Return model-facing context for the owning service, when available.
    #[must_use]
    pub const fn service_context(&self) -> Option<&CapabilityServiceContext> {
        self.service_context.as_ref()
    }

    /// Check arguments against the exact source rules for this binding.
    #[must_use]
    pub fn accepts_arguments(&self, arguments: &Value) -> bool {
        self.input_check.accepts(arguments)
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

    /// Produce both binding-owned saved views for one invocation result.
    #[must_use]
    pub fn persisted_payload(
        &self,
        arguments: &Value,
        output: &Value,
    ) -> PersistedCapabilityPayload {
        PersistedCapabilityPayload {
            arguments: self.persist_arguments(arguments),
            output: self.persist_output(output),
        }
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
    service_contexts: BTreeMap<(String, String, Option<String>, String), CapabilityServiceContext>,
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
        if let Some(context) = binding.service_context() {
            let destination = binding
                .destination()
                .ok_or(CapabilityCatalogError::ServiceContextWithoutDestination)?;
            let key = (
                destination.service_id().to_string(),
                destination.connection_id().to_string(),
                destination.account_id().map(str::to_string),
                destination.revision().to_string(),
            );
            if self
                .service_contexts
                .get(&key)
                .is_some_and(|existing| existing != context)
            {
                return Err(CapabilityCatalogError::ConflictingServiceContext);
            }
            self.service_contexts.insert(key, context.clone());
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
    /// Model-facing service context lacks an exact destination.
    #[error("capability service context has no destination")]
    ServiceContextWithoutDestination,
    /// Bindings for one exact destination disagree about service identity.
    #[error("capability service context conflicts for one destination")]
    ConflictingServiceContext,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAvailabilityStatus {
    /// Capability is temporarily unavailable.
    Unavailable,
    /// Capability needs user authentication.
    AuthenticationRequired,
    /// An active authorization does not satisfy one reviewed operation.
    AuthorizationScopeUnavailable {
        /// Exact definition revision that declared the operation authorization.
        definition_digest: String,
        /// Stable operation within that definition.
        operation_id: String,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapabilityExecutionDecision, CapabilityScope, CapabilityToolBehavior};

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
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
            Arc::new(|_: &Value| true),
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
        let debug = format!("{binding:?}");
        assert!(debug.contains("InvokerKey(\"test\")"));
        assert!(debug.contains("OperationToken(\"read_docs\")"));
        let views = persisted_views(
            binding,
            &json!({"private":"workspace query"}),
            &json!({"private":"workspace result"}),
        );
        assert_eq!(views.arguments, None);
        assert_eq!(views.output, None);
    }

    #[test]
    fn url_policy_composes_url_and_exact_standard_credential_cleanup() {
        let sanitizer = UrlPayloadSanitizer;
        let views = PersistedCapabilityPayload {
            arguments: sanitizer.persist_arguments(&json!({
                "url":"https://user:secret@example.com/path?view=full#section",
                "headers":{"Authorization":"Bearer private"},
                "content":"authorized private source"
            })),
            output: sanitizer.persist_output(&json!({
                "access_token":"private",
                "snapshot":{"text":"authorized private page"}
            })),
        };
        let arguments = views.arguments.expect("arguments");
        let output = views.output.expect("output");
        assert_json_fields!(arguments,
            "/url" => "https://example.com/path?view=full#section",
            "/headers/Authorization" => "[REDACTED]",
            "/content" => "authorized private source",
        );
        assert_json_fields!(output,
            "/access_token" => "[REDACTED]",
            "/snapshot/text" => "authorized private page",
        );
        assert!(!arguments.to_string().contains("user:secret"));
    }

    #[test]
    fn artifact_policy_preserves_content_and_redacts_secrets_on_both_paths() {
        let binding = binding(
            "artifact.create_local_file",
            Arc::new(RedactingPayloadSanitizer),
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
            "/versions/0/content" => "argument body",
            "/api_key" => "[REDACTED]",
        );
        assert_json_fields!(output,
            "/artifact_id" => "artifact:1",
            "/versions/0/title" => "Saved",
            "/versions/0/content" => "output body",
            "/access_token" => "[REDACTED]",
        );

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
            "/arguments/versions/0/content" => "nested argument body",
            "/arguments/password" => "[REDACTED]",
        );
        assert_json_fields!(nested_output,
            "/result/artifact_id" => "artifact:2",
            "/result/versions/0/content" => "nested output body",
            "/result/cookie" => "[REDACTED]",
        );
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
