//! Typed non-secret authentication challenges emitted by capability invokers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_AUTHORITY_COMPONENT_BYTES: usize = 512;

/// Provider-neutral kind of connection authority that must be authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAuthenticationAuthorityKind {
    /// A configured Model Context Protocol server.
    McpServer,
    /// A declarative HTTP adapter connection.
    AdapterConnection,
    /// A reusable OAuth authorization grant used by API connections.
    AdapterGrant,
}

impl CapabilityAuthenticationAuthorityKind {
    /// Return the stable persisted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::McpServer => "mcp_server",
            Self::AdapterConnection => "adapter_connection",
            Self::AdapterGrant => "adapter_grant",
        }
    }
}

/// Interaction required to satisfy a capability authentication challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAuthenticationChallengeKind {
    /// Refresh or repeat an interactive connection authorization.
    Reauthenticate,
    /// Replace a user-supplied static credential.
    ReplaceCredential,
}

impl CapabilityAuthenticationChallengeKind {
    /// Return the stable persisted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reauthenticate => "reauthenticate",
            Self::ReplaceCredential => "replace_credential",
        }
    }
}

/// Authentication interaction required before an exact capability can resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAuthenticationChallenge {
    challenge_kind: CapabilityAuthenticationChallengeKind,
    authority_kind: CapabilityAuthenticationAuthorityKind,
    authority_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    destination_id: Option<String>,
    authority_revision: String,
}

impl CapabilityAuthenticationChallenge {
    /// Construct a bounded challenge containing only stable, non-secret identity.
    ///
    /// # Errors
    /// Returns an error when an identity component is blank, oversized, or not
    /// safe for durable comparison.
    pub fn new(
        challenge_kind: CapabilityAuthenticationChallengeKind,
        authority_kind: CapabilityAuthenticationAuthorityKind,
        authority_id: impl Into<String>,
        authority_revision: impl Into<String>,
    ) -> Result<Self, CapabilityAuthenticationChallengeError> {
        let authority_id = authority_id.into();
        let authority_revision = authority_revision.into();
        validate_component(&authority_id)?;
        validate_component(&authority_revision)?;
        Ok(Self {
            challenge_kind,
            authority_kind,
            authority_id,
            destination_id: None,
            authority_revision,
        })
    }

    /// Construct a challenge whose reusable authority differs from its destination.
    ///
    /// # Errors
    ///
    /// Returns an error when an authority or destination component is invalid.
    pub fn new_for_destination(
        challenge_kind: CapabilityAuthenticationChallengeKind,
        authority_kind: CapabilityAuthenticationAuthorityKind,
        authority_id: impl Into<String>,
        destination_id: impl Into<String>,
        authority_revision: impl Into<String>,
    ) -> Result<Self, CapabilityAuthenticationChallengeError> {
        let mut challenge = Self::new(
            challenge_kind,
            authority_kind,
            authority_id,
            authority_revision,
        )?;
        let destination_id = destination_id.into();
        validate_component(&destination_id)?;
        challenge.destination_id = Some(destination_id);
        Ok(challenge)
    }

    /// Return the requested authentication interaction.
    #[must_use]
    pub const fn challenge_kind(&self) -> CapabilityAuthenticationChallengeKind {
        self.challenge_kind
    }

    /// Return the concrete authority family.
    #[must_use]
    pub const fn authority_kind(&self) -> CapabilityAuthenticationAuthorityKind {
        self.authority_kind
    }

    /// Return the stable connection authority identifier.
    #[must_use]
    pub fn authority_id(&self) -> &str {
        &self.authority_id
    }

    /// Return the concrete connection destination for this challenge.
    #[must_use]
    pub fn destination_id(&self) -> &str {
        self.destination_id.as_deref().unwrap_or(&self.authority_id)
    }

    /// Return the exact authority revision observed by the invoker.
    #[must_use]
    pub fn authority_revision(&self) -> &str {
        &self.authority_revision
    }

    /// Match the complete non-secret destination identity expected by this challenge.
    #[must_use]
    pub fn matches_destination(
        &self,
        service_id: &str,
        connection_id: &str,
        revision: &str,
    ) -> bool {
        let family_matches = match self.authority_kind {
            CapabilityAuthenticationAuthorityKind::McpServer => service_id == "mcp",
            CapabilityAuthenticationAuthorityKind::AdapterConnection
            | CapabilityAuthenticationAuthorityKind::AdapterGrant => service_id == "adapter",
        };
        family_matches
            && self.destination_id.as_deref().unwrap_or(&self.authority_id) == connection_id
            && self.authority_revision == revision
    }
}

/// Invalid authentication challenge identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("capability authentication challenge identity is invalid")]
pub struct CapabilityAuthenticationChallengeError;

fn validate_component(value: &str) -> Result<(), CapabilityAuthenticationChallengeError> {
    if value.is_empty()
        || value.len() > MAX_AUTHORITY_COMPONENT_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.' | b'/')
        })
    {
        return Err(CapabilityAuthenticationChallengeError);
    }
    Ok(())
}

#[cfg(test)]
#[path = "authentication/tests.rs"]
mod tests;
