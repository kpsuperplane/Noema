//! Server-derived request authority, trust, scope, and policy contracts.

mod policy;
mod principal;
mod safe_error;
mod scope;
mod trust;

pub use policy::{Authorizer, PolicyDecision, PolicyReason};
pub use principal::{
    AuthorityIdError, CallbackAuthority, CorrelationId, PrincipalId, PrincipalSubject,
    RequestContext, RequestPrincipal, RunAuthority, RunPurpose, SystemPrincipal, TransportKind,
};
pub use safe_error::{
    ApiErrorCode, InternalErrorDiagnostic, InternalErrorEventKind, InternalErrorReporter,
    SafeApiError, report_internal,
};
pub use scope::{
    GovernedScope, ScopeId, ScopeIdError, UnsupportedScope, canonicalize_scopes,
    governed_scope_fingerprint,
};
pub use trust::{ContentTrust, TrustedEnvelope};

#[cfg(test)]
mod tests;
