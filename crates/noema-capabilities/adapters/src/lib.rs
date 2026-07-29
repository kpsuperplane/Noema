//! Provider-neutral, filesystem-canonical adapter definitions.
//!
//! This crate compiles reviewed data into immutable operation plans. It has no
//! HTTP client, credential access, provider dispatch, or SQLite dependency.

mod catalog;
mod compiler;
mod connection;
mod connection_store;
mod continuation;
mod credential_import;
mod definition;
mod definition_store;
mod digest;
mod event;
mod invocation;
mod json_limits;
mod legacy;
mod luau;
mod network;
mod oauth;
mod openapi;
mod openapi31;
mod openapi_normalize;
mod openapi_schema;
mod output_schema;
mod private_fs;
mod request;
mod response;
mod schedule;
mod service;
mod setup;

pub use catalog::{AdapterCatalogCompiler, AdapterCatalogError};
pub use compiler::{
    AdapterCompileError, AdapterCompiler, CompiledAdapterDefinition, CompiledOperation,
    ConnectionSlug, DefinitionOperationToken, SemanticChange,
};
pub use connection::{
    AdapterConnectionRevisions, AdapterConnectionStatus, AdapterConnectionV3,
    AdapterCredentialGenerationV1, AdapterCredentialMaterial,
};
pub use connection_store::{
    AdapterConnectionStore, ConnectionInstall, ConnectionProjection, ConnectionScan,
    ConnectionScanDiagnostic, ConnectionStoreError,
};
pub use continuation::{
    ContinuationAuthBinding, ContinuationEligibility, ContinuationError, ContinuationGateError,
    CursorBinding, CursorHandle, CursorSecret, CursorStatus, CursorStore, DurableCursorError,
    DurableCursorStore, ValidatedProviderLink, parse_retry_after, validate_provider_link,
};
pub use credential_import::{AdapterCredentialImportError, import_client_json};
pub use definition::{
    AccountGate, AccountIdentityProbe, AdapterManifestV3, AdapterOperation,
    AdapterOperationBehavior, ArgumentDefinition, ArgumentLocation, ArgumentSource, ArgumentType,
    AuthenticationMode, AuthenticationRequirement, ContinuationCredentialMode, CostClass,
    CredentialImportKind, CredentialImportLayout, CredentialImportSchema, EventAuthenticity,
    EventMetadata, EventTransport, HttpMethod, Oauth2AuthorizationCodePkceConfig,
    Oauth2CallbackMode, Oauth2ClientAuthentication, OutputSchema, OutputType, PaginationPolicy,
    ProviderLinkKind, QuotaPolicy, ResponseContract, ResponseTransform, RetryPolicy,
};
pub use definition_store::{
    AdapterDefinitionStore, DefinitionInstall, DefinitionProjection, DefinitionProvenance,
    DefinitionScan, DefinitionScanDiagnostic, DefinitionStoreError, StoredAdapterDefinition,
};
pub use digest::{OperationDigest, SemanticDigest, SourceDigest};
pub use event::{
    ChallengeVerifier, EventAuthenticityContract, EventDeduplicator, EventError, HmacEventPolicy,
    RawEventRequest, VerifiedEvent, VerifiedEventRequest, verified_event, verify_challenge,
    verify_hmac_event,
};
pub use openapi::{
    OpenApiActivation, OpenApiActivationError, OpenApiCandidate, OpenApiDiagnostic,
    OpenApiDiagnosticSeverity, OpenApiImportError, OpenApiImporter, OpenApiOperationProposal,
    OpenApiReviewClaim, OpenApiSelection, OpenApiSelectionError, OpenApiSourceFormat,
};
pub use schedule::{
    PollCheckpoint, PollRetryPolicy, PollSchedule, ScheduleClaim, ScheduleError, ScheduleInstall,
    ScheduleLease, ScheduleProjection, ScheduleStore,
};
pub use service::{
    AdapterCapabilityService, AdapterConnectionSetupError, AdapterManagementError,
    AdapterManagementFence, AdapterManagementSnapshot, AdapterMigrationError,
    AdapterOAuthSetupCompletion, AdapterOAuthSetupError, AdapterOAuthSetupStart,
};
