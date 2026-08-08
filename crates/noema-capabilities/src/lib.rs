//! Provider-neutral capability contracts.
//!
//! This crate deliberately contains no provider transport, persistence, or
//! runtime implementation. Serializable [`ToolSpec`] values describe what a
//! model may request; non-serializable [`CapabilityBinding`] values retain the
//! authority needed to execute only the exact catalog that was advertised.

mod authentication;
mod binding;
mod composite;
mod credential_sanitization;
mod integration;
mod metadata;
mod policy;
mod router;
mod tool;
pub mod web;

pub use authentication::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeError, CapabilityAuthenticationChallengeKind,
};
pub use binding::{
    ArtifactPayloadSanitizer, CapabilityAvailabilityNotice, CapabilityAvailabilityStatus,
    CapabilityBinding, CapabilityBindingSource, CapabilityBindingSourceError,
    CapabilityBindingSourceHandle, CapabilityCatalogBuilder, CapabilityCatalogError,
    CapabilityCatalogResult, CapabilityCatalogSnapshot, CapabilityExecutionDecision,
    CapabilityScope, CapabilityServiceContext, CapabilityServiceContextError, CapabilityTarget,
    CapabilityToolBehavior, OmitPayloadSanitizer, OperationToken, PayloadSanitizer,
    PersistedCapabilityPayload, RedactingPayloadSanitizer, WebBrowsePayloadSanitizer,
    WebFetchPayloadSanitizer,
};
pub use composite::CompositeCapabilityBindingSource;
pub use credential_sanitization::{
    sanitize_standard_credentials, sanitize_standard_credentials_with_additional_names,
    sanitize_url_credentials,
};
pub use integration::{
    CapabilityConnectionLabelError, CapabilityConnectionPolicy, CapabilityDataSharingPolicy,
    CapabilityIntegrationKind, CapabilityPolicyValueError, CapabilityToolClassificationError,
    CapabilityToolHint, CapabilityToolHintCompletion, CapabilityToolHintSource,
    CapabilityToolPolicy, CapabilityToolPolicyOverride, CapabilityToolPolicyStatus,
    CapabilityUnsafeActionPolicy, apply_tool_classification, apply_tool_safe_defaults,
    build_tool_classification_prompt, normalize_capability_connection_label,
    parse_tool_classification_response, resolve_capability_execution_decision,
    validate_capability_connection_policy,
};
pub use metadata::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ReliabilityContract, ResultPersistencePolicy,
};
pub use policy::{CapabilityDestination, CapabilityDestinationError};
pub use router::{
    CapabilityDispatch, CapabilityDispatchFailure, CapabilityError, CapabilityFailure,
    CapabilityFailureKind, CapabilityFuture, CapabilityInvocation, CapabilityInvoker,
    CapabilityInvokerHandle, CapabilityInvokerRegistration, CapabilityOutput, CapabilityRecovery,
    CapabilityRegistryRouter, CapabilityRouterConstructionError, InvokerKey,
    ReviewedCapabilityAuthorization,
};
pub use tool::{ToolContractError, ToolName, ToolSchema, ToolSpec};
