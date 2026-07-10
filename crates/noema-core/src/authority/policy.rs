//! Central policy-neutral authorization operations.

use serde::Serialize;

use super::{
    CorrelationId, PrincipalSubject, RequestContext, SafeApiError, ScopeId, SystemPrincipal,
};

/// Stable reason attached to a policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyReason {
    /// System administrator authority allowed the operation.
    SystemAdministrator,
    /// Principal exactly matched the concrete owner.
    OwnerMatch,
    /// Principal owns the concrete conversation.
    ConversationOwner,
    /// Principal owns the concrete artifact.
    ArtifactOwner,
    /// Request authority was missing.
    MissingAuthority,
    /// Principal did not match the required owner.
    CrossOwner,
}

/// Typed allow/deny policy outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "decision",
    content = "reason",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum PolicyDecision {
    /// Operation is allowed for the stable reason.
    Allow(PolicyReason),
    /// Operation is denied for the stable reason.
    Deny(PolicyReason),
}

/// Centralized policy-neutral authorization checks.
pub struct Authorizer;

impl Authorizer {
    /// Require daemon system-administrator authority.
    ///
    /// # Errors
    ///
    /// Returns a transport-safe authentication or authorization error.
    pub fn require_system_admin(
        context: Option<&RequestContext>,
    ) -> Result<PolicyDecision, SafeApiError> {
        let context = require_context(context)?;
        if is_admin(&context.principal.subject) {
            Ok(PolicyDecision::Allow(PolicyReason::SystemAdministrator))
        } else {
            Err(SafeApiError::forbidden(context.correlation_id.clone()))
        }
    }

    /// Require the authenticated principal to be the concrete owner or an administrator.
    ///
    /// # Errors
    ///
    /// Returns a transport-safe authentication or authorization error.
    pub fn require_owner(
        context: Option<&RequestContext>,
        owner: &PrincipalSubject,
    ) -> Result<PolicyDecision, SafeApiError> {
        let context = require_context(context)?;
        if is_admin(&context.principal.subject) {
            return Ok(PolicyDecision::Allow(PolicyReason::SystemAdministrator));
        }
        if &context.principal.subject == owner {
            return Ok(PolicyDecision::Allow(PolicyReason::OwnerMatch));
        }
        Err(SafeApiError::forbidden(context.correlation_id.clone()))
    }

    /// Require access to a concrete conversation owned by `owner`.
    ///
    /// # Errors
    ///
    /// Returns a transport-safe authentication or authorization error.
    pub fn require_conversation_access(
        context: Option<&RequestContext>,
        _conversation_id: &ScopeId,
        owner: &PrincipalSubject,
    ) -> Result<PolicyDecision, SafeApiError> {
        Self::require_owner(context, owner).map(|decision| match decision {
            PolicyDecision::Allow(PolicyReason::OwnerMatch) => {
                PolicyDecision::Allow(PolicyReason::ConversationOwner)
            }
            other => other,
        })
    }

    /// Require access to a concrete artifact owned by `owner`.
    ///
    /// # Errors
    ///
    /// Returns a transport-safe authentication or authorization error.
    pub fn require_artifact_access(
        context: Option<&RequestContext>,
        _artifact_id: &ScopeId,
        owner: &PrincipalSubject,
    ) -> Result<PolicyDecision, SafeApiError> {
        Self::require_owner(context, owner).map(|decision| match decision {
            PolicyDecision::Allow(PolicyReason::OwnerMatch) => {
                PolicyDecision::Allow(PolicyReason::ArtifactOwner)
            }
            other => other,
        })
    }
}

fn require_context(context: Option<&RequestContext>) -> Result<&RequestContext, SafeApiError> {
    context.ok_or_else(|| {
        let correlation_id =
            CorrelationId::generate().unwrap_or_else(|_| CorrelationId::unavailable());
        SafeApiError::missing_authority(correlation_id)
    })
}

fn is_admin(subject: &PrincipalSubject) -> bool {
    matches!(
        subject,
        PrincipalSubject::System(SystemPrincipal::Administrator)
    )
}
