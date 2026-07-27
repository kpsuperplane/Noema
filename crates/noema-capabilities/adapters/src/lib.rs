//! Provider-neutral, filesystem-canonical adapter definitions.
//!
//! This crate compiles reviewed data into immutable operation plans. It has no
//! HTTP client, credential access, provider dispatch, or SQLite dependency.

mod compiler;
mod connection;
mod connection_store;
mod definition;
mod definition_store;
mod digest;
mod private_fs;

pub use compiler::{
    AdapterCompileError, AdapterCompiler, CompiledAdapterDefinition, CompiledOperation,
    CompiledPersistencePolicy, ConnectionSlug, DefinitionOperationToken, SemanticChange,
};
pub use connection::{
    AdapterConnectionRevisions, AdapterConnectionStatus, AdapterConnectionV1,
    AdapterCredentialGenerationV1, AdapterCredentialMaterial,
};
pub use connection_store::{
    AdapterConnectionStore, ConnectionInstall, ConnectionProjection, ConnectionScan,
    ConnectionScanDiagnostic, ConnectionStoreError,
};
pub use definition::{
    AccountGate, AdapterManifestV1, AdapterOperation, AdmissionMode, ArgumentDefinition,
    ArgumentLocation, ArgumentSource, ArgumentType, AuthenticationMode, AuthenticationRequirement,
    CostClass, EventAuthenticity, EventMetadata, EventTransport, HttpMethod, ModelPayload,
    ModelRoute, OperationEffect, PaginationPolicy, PersistenceMode, ProviderDataPolicy,
    ProviderRetention, QuotaPolicy, ResultClassification, ResultDefinition, RetryPolicy,
};
pub use definition_store::{
    AdapterDefinitionStore, DefinitionInstall, DefinitionProjection, DefinitionProvenance,
    DefinitionScan, DefinitionScanDiagnostic, DefinitionStoreError,
};
pub use digest::{OperationDigest, SemanticDigest, SourceDigest};
