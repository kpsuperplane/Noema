//! Object-safe invocation and routing contracts.

use crate::{
    CapabilityCatalogSnapshot, CapabilityTarget, OperationToken, PersistedCapabilityPayload,
    ToolName,
};
use serde_json::Value;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use thiserror::Error;

/// Opaque key identifying one server-owned invoker registration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvokerKey(String);

impl InvokerKey {
    /// Construct a server-owned invoker key.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the opaque key for server-side diagnostics and registration.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Boxed future returned by object-safe capability boundaries.
pub type CapabilityFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One resolved invocation without provider correlation identifiers or a
/// generic runtime-context property bag.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityInvocation {
    /// Canonical operation name from the advertised binding.
    pub operation: ToolName,
    /// Opaque child/runtime-owned operation token from that binding.
    pub operation_token: OperationToken,
    /// Provider-supplied JSON arguments.
    pub arguments: Value,
    /// Runtime-issued one-shot authorization for an exact reviewed invocation.
    pub reviewed_authorization: Option<ReviewedCapabilityAuthorization>,
}

/// One action-revision authorization issued by the trusted runtime gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedCapabilityAuthorization {
    /// Durable action id.
    pub action_id: String,
    /// Immutable action revision.
    pub revision: u64,
    /// Digest of the exact persisted argument payload.
    pub arguments_sha256: String,
}

impl ReviewedCapabilityAuthorization {
    /// Return whether this authorization binds the exact canonical arguments.
    #[must_use]
    pub fn matches_arguments(&self, arguments: &Value) -> bool {
        self.arguments_sha256 == arguments_sha256(arguments)
    }
}

/// Model-visible capability result.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityOutput {
    /// Whether the tool-declared operation succeeded.
    pub success: bool,
    /// Structured model-visible payload.
    pub payload: Value,
    /// Server-owned recovery semantics for a completed tool-declared failure.
    pub failure: Option<CapabilityFailure>,
}

/// Provider-neutral category for a completed remote rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityFailureKind {
    /// The provider rejected malformed or unsupported request arguments.
    InvalidRequest,
    /// The authenticated principal lacks permission for the operation.
    PermissionDenied,
    /// The referenced provider resource does not exist at that identifier.
    ResourceNotFound,
    /// Current provider state conflicts with the requested operation.
    Conflict,
    /// The provider is rate limiting requests.
    RateLimited,
    /// The provider is temporarily unable to serve the request.
    RemoteUnavailable,
    /// The provider rejected the request without a more specific category.
    RemoteRejected,
}

impl CapabilityFailureKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::PermissionDenied => "permission_denied",
            Self::ResourceNotFound => "resource_not_found",
            Self::Conflict => "conflict",
            Self::RateLimited => "rate_limited",
            Self::RemoteUnavailable => "remote_unavailable",
            Self::RemoteRejected => "remote_rejected",
        }
    }
}

/// Provider-neutral next step for a completed remote rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRecovery {
    /// Repair arguments using the provider response and known context.
    CorrectArguments,
    /// Resolve the provider's current resource identity before retrying.
    ResolveResource,
    /// Wait or report the temporary failure before another attempt.
    RetryLater,
    /// Stop this operation instead of attempting a workaround.
    Stop,
}

impl CapabilityRecovery {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CorrectArguments => "correct_arguments",
            Self::ResolveResource => "resolve_resource",
            Self::RetryLater => "retry_later",
            Self::Stop => "stop",
        }
    }
}

/// Typed semantics attached by the server to a failed capability result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityFailure {
    /// Stable provider-neutral failure category.
    pub kind: CapabilityFailureKind,
    /// Stable provider-neutral recovery direction.
    pub recovery: CapabilityRecovery,
}

/// Completed dispatch with binding-produced persisted views. A tool-declared
/// failure is still a completed dispatch with `output.success == false`.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityDispatch {
    /// Model-visible operation output.
    pub output: CapabilityOutput,
    /// Persisted views selected by the resolved binding.
    pub persisted: PersistedCapabilityPayload,
}

/// Control-plane or routing failure with all persistence policy already
/// applied. Unknown/unadvertised calls have `None` views because no binding was
/// resolved; their raw arguments must never be persisted.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityDispatchFailure {
    /// Fixed safe failure category.
    pub error: CapabilityError,
    /// Binding-produced views, or omitted views for an unknown call.
    pub persisted: PersistedCapabilityPayload,
}

impl CapabilityDispatchFailure {
    /// Build a failure through the exact binding's persistence policy.
    #[must_use]
    pub fn from_snapshot(
        snapshot: &CapabilityCatalogSnapshot,
        canonical_name: &str,
        arguments: &Value,
        error: CapabilityError,
    ) -> Self {
        let Some(binding) = snapshot.resolve(canonical_name) else {
            return Self {
                error: CapabilityError::UnknownOperation,
                persisted: PersistedCapabilityPayload::omitted(),
            };
        };
        Self {
            persisted: PersistedCapabilityPayload {
                arguments: binding.persist_arguments(arguments),
                output: binding.persist_output(&error.safe_payload()),
            },
            error,
        }
    }
}

impl CapabilityOutput {
    /// Construct a successful output.
    #[must_use]
    pub fn success(payload: Value) -> Self {
        Self {
            success: true,
            payload,
            failure: None,
        }
    }

    /// Construct a tool-declared failed output. Transport and control-plane
    /// failures use [`CapabilityError`] instead.
    #[must_use]
    pub fn failed(payload: Value) -> Self {
        Self {
            success: false,
            payload,
            failure: None,
        }
    }

    /// Construct a failed output with server-owned model recovery semantics.
    #[must_use]
    pub fn failed_with_recovery(payload: Value, failure: CapabilityFailure) -> Self {
        Self {
            success: false,
            payload,
            failure: Some(failure),
        }
        .materialize_failure()
    }

    fn materialize_failure(mut self) -> Self {
        let Some(failure) = self.failure else {
            return self;
        };
        match &mut self.payload {
            Value::Object(payload) => {
                payload.insert(
                    "failure_kind".to_string(),
                    Value::String(failure.kind.as_str().to_string()),
                );
                payload.insert(
                    "recovery".to_string(),
                    Value::String(failure.recovery.as_str().to_string()),
                );
            }
            payload => {
                let result = std::mem::take(payload);
                *payload = serde_json::json!({
                    "result": result,
                    "failure_kind": failure.kind.as_str(),
                    "recovery": failure.recovery.as_str(),
                });
            }
        }
        self
    }
}

/// Sanitized invocation failure categories.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityError {
    /// No invoker is registered for the binding target.
    #[error("capability invoker is unavailable")]
    UnknownInvoker,
    /// The operation token is unknown or stale.
    #[error("capability operation is unavailable")]
    UnknownOperation,
    /// Arguments violate the operation contract.
    #[error("capability arguments are invalid")]
    InvalidArguments,
    /// Current execution policy denies the operation.
    #[error("capability invocation was denied")]
    Denied,
    /// The capability is temporarily unavailable.
    #[error("capability is unavailable")]
    Unavailable,
    /// The remote authority requires interactive authentication before retrying.
    #[error("capability authentication is required")]
    AuthenticationRequired {
        /// Typed authority identity and revision observed under the invoker's
        /// lifecycle fence.
        challenge: crate::CapabilityAuthenticationChallenge,
    },
    /// The implementation failed without a safe tool-declared result.
    #[error("capability invocation failed")]
    Failed,
    /// The remote call was sent, but its externally visible outcome is unknown.
    #[error("capability outcome is uncertain")]
    OutcomeUncertain,
}

/// Object-safe implementation of one family of capability targets.
pub trait CapabilityInvoker: Send + Sync {
    /// Invoke one operation resolved from the immutable advertised catalog.
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>>;
}

/// Clonable capability invoker handle.
pub type CapabilityInvokerHandle = Arc<dyn CapabilityInvoker>;

/// One opaque invoker registration assembled outside generic runtime consumers.
#[derive(Clone)]
pub struct CapabilityInvokerRegistration {
    key: InvokerKey,
    invoker: CapabilityInvokerHandle,
}

impl CapabilityInvokerRegistration {
    /// Bind one child-owned invoker key to its implementation.
    #[must_use]
    pub fn new(key: InvokerKey, invoker: CapabilityInvokerHandle) -> Self {
        Self { key, invoker }
    }

    /// Return the opaque child-owned registration key.
    #[must_use]
    pub const fn key(&self) -> &InvokerKey {
        &self.key
    }

    /// Return the registered invoker handle.
    #[must_use]
    pub const fn invoker(&self) -> &CapabilityInvokerHandle {
        &self.invoker
    }
}

impl std::fmt::Debug for CapabilityInvokerRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityInvokerRegistration")
            .field("key", &self.key)
            .field("invoker", &"[CONFIGURED]")
            .finish()
    }
}

/// Router construction error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CapabilityRouterConstructionError {
    /// Two invokers were registered under the same opaque key.
    #[error("duplicate capability invoker registration")]
    DuplicateInvoker,
}

/// Generic strict registry router over opaque invoker keys and tokens.
#[derive(Clone, Default)]
pub struct CapabilityRegistryRouter<'a> {
    invokers: Arc<HashMap<InvokerKey, Arc<dyn CapabilityInvoker + 'a>>>,
}

impl std::fmt::Debug for CapabilityRegistryRouter<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityRegistryRouter")
            .field("invoker_count", &self.invokers.len())
            .finish()
    }
}

impl<'a> CapabilityRegistryRouter<'a> {
    /// Build a router, rejecting duplicate invoker registrations.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityRouterConstructionError::DuplicateInvoker`] when a
    /// key appears more than once.
    pub fn new<I>(invokers: I) -> Result<Self, CapabilityRouterConstructionError>
    where
        I: IntoIterator<Item = (InvokerKey, Arc<dyn CapabilityInvoker + 'a>)>,
    {
        let mut registered = HashMap::new();
        for (key, invoker) in invokers {
            if registered.insert(key, invoker).is_some() {
                return Err(CapabilityRouterConstructionError::DuplicateInvoker);
            }
        }
        Ok(Self {
            invokers: Arc::new(registered),
        })
    }

    /// Resolve and dispatch only through the supplied immutable snapshot.
    /// # Errors
    /// Returns a sanitized failure when resolution, authorization, or invocation fails.
    pub async fn dispatch(
        &self,
        snapshot: CapabilityCatalogSnapshot,
        canonical_name: String,
        arguments: Value,
    ) -> Result<CapabilityDispatch, CapabilityDispatchFailure> {
        self.dispatch_resolved(&snapshot, &canonical_name, arguments, None)
            .await
    }

    /// Dispatch a reviewed invocation through a runtime-issued authorization.
    /// # Errors
    /// Returns a sanitized failure when resolution, authorization, or invocation fails.
    pub async fn dispatch_reviewed(
        &self,
        snapshot: CapabilityCatalogSnapshot,
        canonical_name: String,
        arguments: Value,
        authorization: ReviewedCapabilityAuthorization,
    ) -> Result<CapabilityDispatch, CapabilityDispatchFailure> {
        self.dispatch_resolved(&snapshot, &canonical_name, arguments, Some(authorization))
            .await
    }

    /// Resolve an advertised name through the exact immutable snapshot and
    /// dispatch its stored target. Unknown names never fall back to a global
    /// catalog or to parsing the provider-returned text.
    async fn dispatch_resolved(
        &self,
        snapshot: &CapabilityCatalogSnapshot,
        canonical_name: &str,
        arguments: Value,
        reviewed_authorization: Option<ReviewedCapabilityAuthorization>,
    ) -> Result<CapabilityDispatch, CapabilityDispatchFailure> {
        let Some(binding) = snapshot.resolve(canonical_name) else {
            return Err(CapabilityDispatchFailure::from_snapshot(
                snapshot,
                canonical_name,
                &arguments,
                CapabilityError::UnknownOperation,
            ));
        };
        if !binding.execution_decision().requires_review() && reviewed_authorization.is_some() {
            return Err(CapabilityDispatchFailure::from_snapshot(
                snapshot,
                canonical_name,
                &arguments,
                CapabilityError::Denied,
            ));
        }
        if binding.execution_decision().requires_review() && binding.destination().is_none() {
            return Err(CapabilityDispatchFailure::from_snapshot(
                snapshot,
                canonical_name,
                &arguments,
                CapabilityError::Denied,
            ));
        }
        if binding.execution_decision().requires_review() && reviewed_authorization.is_none() {
            return Err(CapabilityDispatchFailure::from_snapshot(
                snapshot,
                canonical_name,
                &arguments,
                CapabilityError::Denied,
            ));
        }
        if reviewed_authorization
            .as_ref()
            .is_some_and(|admission| !admission.matches_arguments(&arguments))
        {
            return Err(CapabilityDispatchFailure::from_snapshot(
                snapshot,
                canonical_name,
                &arguments,
                CapabilityError::Denied,
            ));
        }
        let persisted_arguments = binding.persist_arguments(&arguments);
        match self
            .invoke_target(
                binding.target(),
                binding.spec().name.clone(),
                arguments,
                reviewed_authorization,
            )
            .await
        {
            Ok(output) => {
                let output = output.materialize_failure();
                Ok(CapabilityDispatch {
                    persisted: PersistedCapabilityPayload {
                        arguments: persisted_arguments,
                        output: binding.persist_output(&output.payload),
                    },
                    output,
                })
            }
            Err(error) => Err(CapabilityDispatchFailure {
                persisted: PersistedCapabilityPayload {
                    arguments: persisted_arguments,
                    output: binding.persist_output(&error.safe_payload()),
                },
                error,
            }),
        }
    }

    async fn invoke_target(
        &self,
        target: &CapabilityTarget,
        operation: ToolName,
        arguments: Value,
        reviewed_authorization: Option<ReviewedCapabilityAuthorization>,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let invoker = self
            .invokers
            .get(target.invoker_key())
            .ok_or(CapabilityError::UnknownInvoker)?;
        invoker
            .invoke(CapabilityInvocation {
                operation,
                operation_token: target.operation_token().clone(),
                arguments,
                reviewed_authorization,
            })
            .await
    }
}

impl ReviewedCapabilityAuthorization {
    /// Build an authorization for one exact durable action revision and payload.
    #[must_use]
    pub fn for_action(action_id: impl Into<String>, revision: u64, arguments: &Value) -> Self {
        Self {
            action_id: action_id.into(),
            revision,
            arguments_sha256: arguments_sha256(arguments),
        }
    }

    /// Build deterministic authorization for a bare exact observed-URL fetch.
    #[must_use]
    pub fn for_observed_url(arguments: &Value) -> Self {
        Self::for_action("observed_url", 0, arguments)
    }
}

fn arguments_sha256(arguments: &Value) -> String {
    let encoded = serde_json::to_vec(arguments).unwrap_or_default();
    ring::digest::digest(&ring::digest::SHA256, &encoded)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl CapabilityError {
    fn safe_payload(&self) -> Value {
        serde_json::json!({"error": self.safe_code()})
    }

    const fn safe_code(&self) -> &'static str {
        match self {
            Self::UnknownInvoker => "unknown_invoker",
            Self::UnknownOperation => "unknown_operation",
            Self::InvalidArguments => "invalid_arguments",
            Self::Denied => "denied",
            Self::Unavailable => "unavailable",
            Self::AuthenticationRequired { .. } => "authentication_required",
            Self::Failed => "failed",
            Self::OutcomeUncertain => "outcome_uncertain",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityBinding, CapabilityCatalogBuilder, CapabilityDestination,
        CapabilityExecutionDecision, CapabilityScope, CapabilityToolBehavior, OmitPayloadSanitizer,
        PayloadSanitizer, RedactingPayloadSanitizer, ToolSpec,
    };
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingInvoker(Mutex<Vec<CapabilityInvocation>>);

    impl CapabilityInvoker for RecordingInvoker {
        fn invoke(
            &self,
            invocation: CapabilityInvocation,
        ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
            self.0.lock().expect("recording lock").push(invocation);
            Box::pin(async { Ok(CapabilityOutput::success(json!({"ok": true}))) })
        }
    }

    struct FixedInvoker(Result<CapabilityOutput, CapabilityError>);

    impl CapabilityInvoker for FixedInvoker {
        fn invoke(
            &self,
            _invocation: CapabilityInvocation,
        ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
            let result = self.0.clone();
            Box::pin(async move { result })
        }
    }

    struct TokenCheckingInvoker;

    impl CapabilityInvoker for TokenCheckingInvoker {
        fn invoke(
            &self,
            invocation: CapabilityInvocation,
        ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
            Box::pin(async move {
                if invocation.operation_token.as_str() == "current-token" {
                    Ok(CapabilityOutput::success(json!({"ok":true})))
                } else {
                    Err(CapabilityError::UnknownOperation)
                }
            })
        }
    }

    fn snapshot() -> CapabilityCatalogSnapshot {
        snapshot_with(
            InvokerKey::new("mcp"),
            OperationToken::new("reviewed:1"),
            Arc::new(RedactingPayloadSanitizer),
        )
    }

    fn snapshot_with(
        invoker_key: InvokerKey,
        operation_token: OperationToken,
        sanitizer: Arc<dyn PayloadSanitizer>,
    ) -> CapabilityCatalogSnapshot {
        let binding = CapabilityBinding::new(
            ToolSpec::new("mcp.docs.read", "Read docs.", json!({"type":"object"})).expect("spec"),
            CapabilityTarget::new(invoker_key, operation_token),
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
            sanitizer,
        );
        let mut builder = CapabilityCatalogBuilder::new();
        builder.add(binding).expect("catalog entry");
        builder.build()
    }

    #[test]
    fn strict_resolution_rejects_unknown_and_forwards_exact_target() {
        let invoker = Arc::new(RecordingInvoker::default());
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            invoker.clone() as CapabilityInvokerHandle,
        )])
        .expect("router");
        let snapshot = snapshot();

        let success = poll_ready(router.dispatch(
            snapshot.clone(),
            "mcp.docs.read".to_string(),
            json!({
                "query":"rust",
                "invoker_key":"forged",
                "operation_token":"forged"
            }),
        ));
        assert_eq!(
            success.expect("dispatch").output.payload,
            json!({"ok":true})
        );
        assert_eq!(
            invoker.0.lock().expect("recording lock")[0]
                .operation_token
                .as_str(),
            "reviewed:1"
        );
        assert_eq!(
            invoker.0.lock().expect("recording lock")[0].arguments,
            json!({
                "query":"rust",
                "invoker_key":"forged",
                "operation_token":"forged"
            })
        );

        let error = poll_ready(router.dispatch(
            snapshot,
            "mcp.hidden.write".to_string(),
            json!({"private":"never persist"}),
        ))
        .expect_err("unknown advertised name rejected");
        assert_eq!(error.error, CapabilityError::UnknownOperation);
        assert_eq!(error.persisted, PersistedCapabilityPayload::omitted());
    }

    #[test]
    fn reviewed_decision_requires_explicit_reviewed_dispatch() {
        let invoker = Arc::new(RecordingInvoker::default());
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            invoker.clone() as CapabilityInvokerHandle,
        )])
        .expect("router");
        let mut builder = CapabilityCatalogBuilder::new();
        builder
            .add(
                CapabilityBinding::new(
                    ToolSpec::new("mcp.docs.write", "Write docs.", json!({"type":"object"}))
                        .expect("spec"),
                    CapabilityTarget::new(
                        InvokerKey::new("mcp"),
                        OperationToken::new("reviewed:write"),
                    ),
                    CapabilityToolBehavior {
                        read_only: false,
                        idempotent: false,
                        destructive: true,
                        open_world: true,
                    },
                    CapabilityExecutionDecision::LlmReview,
                    CapabilityScope::Global,
                    Arc::new(OmitPayloadSanitizer),
                )
                .with_destination(
                    CapabilityDestination::new("mcp", "mcp:docs", None::<String>, "1")
                        .expect("destination"),
                ),
            )
            .expect("binding");
        let snapshot = builder.build();

        let denied = poll_ready(router.dispatch(
            snapshot.clone(),
            "mcp.docs.write".to_string(),
            json!({"body":"exact"}),
        ))
        .expect_err("ordinary path must deny external write");
        assert_eq!(denied.error, CapabilityError::Denied);
        assert!(invoker.0.lock().expect("recording lock").is_empty());

        let arguments = json!({"body":"exact"});
        poll_ready(router.dispatch_reviewed(
            snapshot,
            "mcp.docs.write".to_string(),
            arguments.clone(),
            ReviewedCapabilityAuthorization {
                action_id: "action:test".to_string(),
                revision: 1,
                arguments_sha256: arguments_sha256(&arguments),
            },
        ))
        .expect("governed dispatch");
        assert_eq!(
            invoker.0.lock().expect("recording lock")[0]
                .reviewed_authorization
                .as_ref()
                .map(|authorization| authorization.action_id.as_str()),
            Some("action:test")
        );
    }

    #[test]
    fn immediate_external_tool_reaches_the_invoker() {
        let invoker = Arc::new(RecordingInvoker::default());
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            invoker.clone() as CapabilityInvokerHandle,
        )])
        .expect("router");
        let binding = CapabilityBinding::new(
            ToolSpec::new("mcp.docs.write", "Write docs.", json!({"type":"object"})).expect("spec"),
            CapabilityTarget::new(
                InvokerKey::new("mcp"),
                OperationToken::new("reviewed:write"),
            ),
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
            Arc::new(OmitPayloadSanitizer),
        )
        .with_destination(
            CapabilityDestination::new("mcp", "mcp:docs", None::<String>, "1")
                .expect("destination"),
        );
        let mut builder = CapabilityCatalogBuilder::new();
        builder.add(binding).expect("binding");

        poll_ready(router.dispatch(
            builder.build(),
            "mcp.docs.write".to_string(),
            json!({"body":"exact"}),
        ))
        .expect("safe external tool executes immediately");
        assert_eq!(invoker.0.lock().expect("recording lock").len(), 1);
    }

    fn control_plane_failure(
        sanitizer: Arc<dyn PayloadSanitizer>,
        arguments: Value,
    ) -> CapabilityDispatchFailure {
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            Arc::new(FixedInvoker(Err(CapabilityError::Unavailable))) as CapabilityInvokerHandle,
        )])
        .expect("router");
        poll_ready(router.dispatch(
            snapshot_with(
                InvokerKey::new("mcp"),
                OperationToken::new("reviewed:1"),
                sanitizer,
            ),
            "mcp.docs.read".to_string(),
            arguments,
        ))
        .expect_err("control-plane failure")
    }

    #[test]
    fn binding_policy_applies_to_every_control_plane_failure_view() {
        let omitted = control_plane_failure(
            Arc::new(OmitPayloadSanitizer),
            json!({"private":"workspace"}),
        );
        assert_eq!(omitted.error, CapabilityError::Unavailable);
        assert_eq!(omitted.persisted, PersistedCapabilityPayload::omitted());

        let redacted = control_plane_failure(
            Arc::new(RedactingPayloadSanitizer),
            json!({"api_key":"private", "query":"safe"}),
        );
        assert_eq!(
            redacted.persisted.arguments,
            Some(json!({"api_key":"[REDACTED]", "query":"safe"}))
        );
        assert_eq!(
            redacted.persisted.output,
            Some(json!({"error":"unavailable"}))
        );
    }

    #[test]
    fn tool_declared_failure_is_completed_dispatch_with_views() {
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            Arc::new(FixedInvoker(Ok(CapabilityOutput::failed_with_recovery(
                json!({
                    "error":"tool_declared",
                    "password":"private"
                }),
                CapabilityFailure {
                    kind: CapabilityFailureKind::InvalidRequest,
                    recovery: CapabilityRecovery::CorrectArguments,
                },
            )))) as CapabilityInvokerHandle,
        )])
        .expect("router");
        let dispatch = poll_ready(router.dispatch(
            snapshot(),
            "mcp.docs.read".to_string(),
            json!({"query":"safe"}),
        ))
        .expect("completed tool failure");
        assert!(!dispatch.output.success);
        assert_eq!(
            dispatch.persisted.output,
            Some(json!({
                "error":"tool_declared",
                "failure_kind":"invalid_request",
                "password":"[REDACTED]",
                "recovery":"correct_arguments"
            }))
        );
    }

    #[test]
    fn unknown_invoker_and_stale_token_are_typed_and_sanitized() {
        let missing = poll_ready(CapabilityRegistryRouter::default().dispatch(
            snapshot(),
            "mcp.docs.read".to_string(),
            json!({"query":"safe"}),
        ))
        .expect_err("unknown invoker");
        assert_eq!(missing.error, CapabilityError::UnknownInvoker);
        assert_eq!(missing.persisted.arguments, Some(json!({"query":"safe"})));

        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new("mcp"),
            Arc::new(TokenCheckingInvoker) as CapabilityInvokerHandle,
        )])
        .expect("router");
        let stale = poll_ready(router.dispatch(
            snapshot_with(
                InvokerKey::new("mcp"),
                OperationToken::new("stale-token"),
                Arc::new(RedactingPayloadSanitizer),
            ),
            "mcp.docs.read".to_string(),
            json!({}),
        ))
        .expect_err("stale token");
        assert_eq!(stale.error, CapabilityError::UnknownOperation);
    }

    #[test]
    fn duplicate_invoker_registration_is_rejected() {
        let invoker = Arc::new(RecordingInvoker::default()) as CapabilityInvokerHandle;
        let error = CapabilityRegistryRouter::new([
            (InvokerKey::new("runtime"), invoker.clone()),
            (InvokerKey::new("runtime"), invoker),
        ])
        .expect_err("duplicate rejected");
        assert_eq!(error, CapabilityRouterConstructionError::DuplicateInvoker);
    }

    fn poll_ready<F: Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }
}
