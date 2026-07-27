//! Provider-neutral, filesystem-canonical adapter definitions.
//!
//! This crate compiles reviewed data into immutable operation plans. It has no
//! HTTP client, credential access, provider dispatch, or SQLite dependency.

mod catalog;
mod compiler;
mod connection;
mod connection_store;
mod credential_import;
mod definition;
mod definition_store;
mod digest;
mod invocation;
mod json_limits;
mod network;
mod private_fs;
mod request;
mod service;

pub use catalog::{AdapterCatalogCompiler, AdapterCatalogError};
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
pub use credential_import::{AdapterCredentialImportError, import_client_json};
pub use definition::{
    AccountGate, AdapterManifestV1, AdapterOperation, AdmissionMode, ArgumentDefinition,
    ArgumentLocation, ArgumentSource, ArgumentType, AuthenticationMode, AuthenticationRequirement,
    CostClass, CredentialImportKind, CredentialImportLayout, CredentialImportSchema,
    EventAuthenticity, EventMetadata, EventTransport, HttpMethod, ModelPayload, ModelRoute,
    OperationEffect, PaginationPolicy, PersistenceMode, ProviderDataPolicy, ProviderRetention,
    QuotaPolicy, ResultClassification, ResultDefinition, RetryPolicy,
};
pub use definition_store::{
    AdapterDefinitionStore, DefinitionInstall, DefinitionProjection, DefinitionProvenance,
    DefinitionScan, DefinitionScanDiagnostic, DefinitionStoreError,
};
pub use digest::{OperationDigest, SemanticDigest, SourceDigest};
pub use service::AdapterCapabilityService;
