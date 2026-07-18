//! Provider-owned persistence boundaries.
//!
//! These contracts expose provider operations rather than repository
//! implementation details. Concrete stores own transactionality, row decoding,
//! and conversion into the transport-neutral persistence error vocabulary.

mod accounts;
mod capabilities;
mod catalog;
mod error;
mod local_models;

pub use accounts::{
    ProviderAccountPersistence, ProviderAccountPersistenceHandle, ProviderAccountStatusUpdate,
    UpdateProviderAccountRequest,
};
pub use capabilities::{
    ProviderCapabilityAccountReference, ProviderCapabilityAccountReferenceMode,
    ProviderCapabilityAssignmentKey, ProviderCapabilityAssignmentPersistence,
    UpsertProviderCapabilityAssignmentRequest,
};
pub use catalog::{
    PersistProviderModelCatalogRequest, ProviderModelCatalogPersistence,
    ProviderModelCatalogPersistenceHandle,
};
pub use error::ProviderPersistenceError;
pub use local_models::{
    ClaimedLocalModelInstallation, LocalModelActivationPersistence,
    LocalModelActivationPersistenceHandle, LocalModelInstallationPersistence,
    LocalModelInstallationPersistenceHandle, LocalModelInstanceReference,
    LocalModelInstanceReferenceSource, LocalModelLifecyclePersistence,
    LocalModelLifecyclePersistenceHandle, LocalModelReconstructionSnapshot,
    LocalModelRetirementClaimResult, LocalModelRuntimeRetirementResult,
};

use std::{future::Future, pin::Pin};

/// Boxed future returned by object-safe provider persistence contracts.
pub type ProviderPersistenceFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderPersistenceError>> + Send + 'a>>;
