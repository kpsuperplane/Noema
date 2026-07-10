//! Server-derived principals and request context.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;
use std::{fmt, net::SocketAddr};
use thiserror::Error;

use super::{GovernedScope, ScopeId};

const OPAQUE_ID_BYTES: usize = 32;

macro_rules! opaque_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Generate a cryptographically random opaque id.
            ///
            /// # Errors
            ///
            /// Returns [`AuthorityIdError`] if the operating-system random source fails.
            pub fn generate() -> Result<Self, AuthorityIdError> {
                let mut bytes = [0_u8; OPAQUE_ID_BYTES];
                SystemRandom::new()
                    .fill(&mut bytes)
                    .map_err(|_| AuthorityIdError::RandomnessUnavailable)?;
                Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
            }

            /// Borrow the opaque id.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

opaque_id!(
    PrincipalId,
    "Opaque id for one authenticated principal instance."
);
opaque_id!(
    CorrelationId,
    "Opaque id shared by an API error and its diagnostic."
);

impl CorrelationId {
    pub(crate) fn unavailable() -> Self {
        Self("correlation-unavailable".to_string())
    }
}

/// Opaque-id generation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AuthorityIdError {
    /// The operating-system randomness source failed.
    #[error("secure randomness is unavailable")]
    RandomnessUnavailable,
}

/// Canonical subject represented by a request principal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrincipalSubject {
    /// A concrete human.
    Human(ScopeId),
    /// A concrete Noema agent.
    Agent(ScopeId),
    /// A daemon-owned system principal.
    System(SystemPrincipal),
}

impl PrincipalSubject {
    /// Construct a concrete human subject.
    #[must_use]
    pub const fn human(id: ScopeId) -> Self {
        Self::Human(id)
    }

    /// Construct a concrete agent subject.
    #[must_use]
    pub const fn agent(id: ScopeId) -> Self {
        Self::Agent(id)
    }
}

/// Closed set of daemon-owned system principals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemPrincipal {
    /// System administrator authority.
    Administrator,
    /// Internal daemon work without administrator authority.
    Runtime,
}

/// Trusted transport that derived a principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransportKind {
    /// Authenticated local web transport.
    Web,
    /// Trusted desktop command boundary.
    Desktop,
    /// Internal daemon work.
    Internal,
}

/// Server-derived callback boundary. Client payloads never construct this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "address", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CallbackAuthority {
    /// Exact web listener address.
    Web(SocketAddr),
    /// Trusted desktop callback listener.
    Desktop,
}

/// Authenticated principal for one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestPrincipal {
    /// Opaque principal instance id.
    pub id: PrincipalId,
    /// Canonical concrete or system subject.
    pub subject: PrincipalSubject,
    /// Transport that derived this principal.
    pub transport: TransportKind,
}

/// Server-derived authority supplied to an API operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestContext {
    /// Authenticated request principal.
    pub principal: RequestPrincipal,
    /// Diagnostic/API correlation id.
    pub correlation_id: CorrelationId,
    /// Exact callback authority, when this transport supports callbacks.
    pub callback_authority: Option<CallbackAuthority>,
}

/// Typed purpose for a daemon run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunPurpose {
    /// A user-submitted conversation turn.
    ConversationTurn,
    /// Memory maintenance initiated by Noema.
    MemoryMaintenance,
    /// Governed tool execution.
    ToolExecution,
}

/// Immutable authority carried through one daemon run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunAuthority {
    /// Authenticated principal that initiated the run.
    pub principal: RequestPrincipal,
    /// Canonical conversation id.
    pub conversation_id: ScopeId,
    /// Canonicalized active scopes.
    pub active_scopes: Vec<GovernedScope>,
    /// Structured purpose.
    pub purpose: RunPurpose,
}
