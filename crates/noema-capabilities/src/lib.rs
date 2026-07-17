//! Provider-neutral capability contracts.
//!
//! This crate deliberately contains no provider transport, persistence, or
//! runtime implementation. Serializable [`ToolSpec`] values describe what a
//! model may request; non-serializable [`CapabilityBinding`] values retain the
//! authority needed to execute only the exact catalog that was advertised.

mod binding;
mod metadata;
mod router;
mod tool;
pub mod web;

pub use binding::{
    ArtifactPayloadSanitizer, CapabilityAccess, CapabilityAvailabilityNotice,
    CapabilityAvailabilityStatus, CapabilityBinding, CapabilityBindingSource,
    CapabilityBindingSourceError, CapabilityBindingSourceHandle, CapabilityCatalogBuilder,
    CapabilityCatalogError, CapabilityCatalogResult, CapabilityCatalogSnapshot, CapabilityEffect,
    CapabilityScope, CapabilityTarget, OmitPayloadSanitizer, OperationToken, PayloadSanitizer,
    PersistedCapabilityPayload, RedactingPayloadSanitizer, WebFetchPayloadSanitizer,
};
pub use metadata::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ReliabilityContract, ResultPersistencePolicy,
};
pub use router::{
    CapabilityDispatch, CapabilityDispatchFailure, CapabilityError, CapabilityFuture,
    CapabilityInvocation, CapabilityInvoker, CapabilityInvokerHandle,
    CapabilityInvokerRegistration, CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter,
    CapabilityRouterConstructionError, CapabilityRouterHandle, InvokerKey,
};
pub use tool::{ToolContractError, ToolName, ToolSchema, ToolSpec};
