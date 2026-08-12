//! Provider-neutral, filesystem-canonical adapter definitions.
//!
//! This crate compiles reviewed data into immutable operation plans. It has no
//! provider-specific dispatch or SQLite dependency; Rust retains HTTP and
//! private credential lifecycle authority around reviewed Luau transforms.

mod catalog;
mod compiler;
mod connection;
mod connection_store;
mod continuation;
mod credential_import;
mod definition;
mod definition_store;
mod digest;
mod invocation;
mod json_limits;
mod luau;
mod network;
mod oauth;
mod oauth_authority;
mod oauth_authority_fs;
mod oauth_authority_store;
mod oauth_authority_validation;
mod openapi;
mod openapi31;
mod openapi_normalize;
mod openapi_schema;
mod output_schema;
mod private_fs;
mod request;
mod response;
#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;
mod schedule;
mod service;
mod setup;
mod transition;

pub use catalog::{AdapterCatalogCompiler, AdapterCatalogError};
pub use compiler::{
    AdapterCompileError, AdapterCompiler, CompiledAdapterDefinition, CompiledOperation,
    ConnectionSlug, DefinitionOperationToken, SemanticChange,
};
pub use connection::{
    AdapterConnectionAuthenticationV1, AdapterConnectionStatus, AdapterConnectionV4,
    AdapterCredentialGenerationV2, AdapterCredentialMaterial,
};
pub use connection_store::{
    AdapterConnectionStore, ConnectionInstall, ConnectionProjection, ConnectionScan,
    ConnectionScanDiagnostic, ConnectionStoreError,
};
pub use continuation::{
    ContinuationError, CursorBinding, CursorHandle, CursorSecret, DurableCursorError,
    DurableCursorStore, parse_retry_after,
};
pub use credential_import::{AdapterCredentialImportError, setup_credential};
pub use definition::{
    AccountIdentityProbe, AdapterManifest, AdapterOperation, AdapterOperationBehavior,
    ArgumentDefinition, ArgumentLocation, ArgumentType, AuthenticationMode, AuthenticationSchemeV4,
    CredentialAuthentication, CredentialField, CredentialInput, CredentialSetup, HttpMethod,
    LuauTransform, Oauth2AuthorizationCodePkceConfig, Oauth2CallbackMode,
    Oauth2ClientAuthentication, Oauth2CredentialSetup, OperationAuthorization, OutputSchema,
    OutputType, PageSizePolicy, PaginationPolicy, ResponseContract, ResponseTransform, RetryPolicy,
};
pub use definition_store::{
    AdapterDefinitionStore, DefinitionInstall, DefinitionProjection, DefinitionProvenance,
    DefinitionScan, DefinitionScanDiagnostic, DefinitionStoreError, StoredAdapterDefinition,
};
pub use digest::{OperationDigest, SemanticDigest, SourceDigest};
pub use oauth_authority::{
    AuthorizationGrantStatus, AuthorizationGrantV1, ExternalAccountV1,
    OauthApplicationCredentialV1, OauthApplicationStatus, OauthApplicationV1, OauthGrantTokenV1,
    OauthProfileV1, OauthScopeResponsePolicy, reviewed_google_oauth_profile,
    reviewed_google_oauth_profile_digest,
};
pub use oauth_authority_store::{
    OauthAuthoritySnapshot, OauthAuthorityStore, OauthAuthorityStoreError, OauthProfileInstall,
};
pub use openapi::{
    OpenApiActivation, OpenApiActivationError, OpenApiCandidate, OpenApiDiagnostic,
    OpenApiDiagnosticSeverity, OpenApiImportError, OpenApiImporter, OpenApiOperationProposal,
    OpenApiReviewClaim, OpenApiSelection, OpenApiSelectionError, OpenApiSourceFormat,
};
pub use private_fs::{
    create_private_dir, random_hex, read_bounded_regular_file, require_regular_directory,
    sync_directory, write_new_file,
};
pub use schedule::{
    PollCheckpoint, PollRetryPolicy, PollSchedule, ScheduleClaim, ScheduleError, ScheduleInstall,
    ScheduleLease, ScheduleProjection, ScheduleStore,
};
pub use service::{
    AdapterCapabilityService, AdapterConnectionSetupError, AdapterManagementError,
    AdapterManagementFence, AdapterManagementSnapshot, AdapterMigrationError,
    AdapterOAuthAttemptEvent, AdapterOAuthAttemptStatus, AdapterOAuthAuthorizationRequest,
    AdapterOAuthServiceSelection, AdapterOAuthSetupCompletion, AdapterOAuthSetupError,
    AdapterOAuthSetupStart,
};
pub use transition::AdapterDefinitionTransition;
