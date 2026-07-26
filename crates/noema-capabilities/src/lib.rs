//! Provider-neutral capability contracts.
//!
//! This crate deliberately contains no provider transport, persistence, or
//! runtime implementation. Serializable [`ToolSpec`] values describe what a
//! model may request; non-serializable [`CapabilityBinding`] values retain the
//! authority needed to execute only the exact catalog that was advertised.

mod authentication;
mod binding;
mod composite;
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
    ArtifactPayloadSanitizer, CapabilityAccess, CapabilityAdmissionPolicy,
    CapabilityAvailabilityNotice, CapabilityAvailabilityStatus, CapabilityBinding,
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogBuilder, CapabilityCatalogError, CapabilityCatalogResult,
    CapabilityCatalogSnapshot, CapabilityEffect, CapabilityScope, CapabilityTarget,
    OmitPayloadSanitizer, OperationToken, PayloadSanitizer, PersistedCapabilityPayload,
    RedactingPayloadSanitizer, WebFetchPayloadSanitizer,
};
pub use composite::CompositeCapabilityBindingSource;
pub use metadata::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ReliabilityContract, ResultPersistencePolicy,
};
pub use policy::{
    CapabilityDestination, CapabilityDestinationError, CapabilityModelPayloadPolicy,
    CapabilityModelRoutePolicy, CapabilityProviderRetentionPolicy, CapabilityResultPolicy,
};
pub use router::{
    CapabilityDispatch, CapabilityDispatchFailure, CapabilityError, CapabilityFuture,
    CapabilityInvocation, CapabilityInvoker, CapabilityInvokerHandle,
    CapabilityInvokerRegistration, CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter,
    CapabilityRouterConstructionError, GovernedCapabilityAdmission, InvokerKey,
};
pub use tool::{ToolContractError, ToolName, ToolSchema, ToolSpec};
