//! Typed governed scopes and deterministic policy evidence.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::digest::{SHA256, digest};
use serde::Serialize;
use std::fmt;
use thiserror::Error;

/// Non-empty id for a concrete governed object.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ScopeId(String);

impl ScopeId {
    /// Construct a server-derived governed object id.
    ///
    /// # Errors
    ///
    /// Returns [`ScopeIdError`] when the id is empty or only whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, ScopeIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ScopeIdError::Empty);
        }
        Ok(Self(value))
    }

    /// Borrow the concrete id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ScopeId").field(&self.0).finish()
    }
}

/// Invalid governed object id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ScopeIdError {
    /// The id was empty.
    #[error("governed scope id must not be empty")]
    Empty,
}

/// Typed governable context. This is an interface, not a universal store row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GovernedScope {
    /// Daemon-wide system scope.
    System,
    /// Concrete human scope.
    Human(ScopeId),
    /// Concrete agent scope.
    Agent(ScopeId),
    /// Concrete conversation scope.
    Conversation(ScopeId),
    /// Future workspace scope; persistence is unsupported.
    Workspace(ScopeId),
    /// Future project scope; persistence is unsupported.
    Project(ScopeId),
    /// Future task scope; persistence is unsupported.
    Task(ScopeId),
    /// Concrete tool scope.
    Tool(ScopeId),
}

impl GovernedScope {
    /// Construct a human scope.
    #[must_use]
    pub const fn human(id: ScopeId) -> Self {
        Self::Human(id)
    }

    /// Construct an agent scope.
    #[must_use]
    pub const fn agent(id: ScopeId) -> Self {
        Self::Agent(id)
    }

    /// Construct a conversation scope.
    #[must_use]
    pub const fn conversation(id: ScopeId) -> Self {
        Self::Conversation(id)
    }

    /// Construct a workspace scope that remains unsupported for persistence.
    #[must_use]
    pub const fn workspace(id: ScopeId) -> Self {
        Self::Workspace(id)
    }

    /// Return a stable persistence key for currently canonical scopes.
    ///
    /// # Errors
    ///
    /// Returns a typed [`UnsupportedScope`] for scopes whose canonical tables
    /// do not exist yet.
    pub fn persistence_key(&self) -> Result<String, UnsupportedScope> {
        match self {
            Self::System => Ok("system".to_string()),
            Self::Human(id) => Ok(format!("human:{}", id.as_str())),
            Self::Agent(id) => Ok(format!("agent:{}", id.as_str())),
            Self::Conversation(id) => Ok(format!("conversation:{}", id.as_str())),
            Self::Tool(id) => Ok(format!("tool:{}", id.as_str())),
            Self::Workspace(_) => Err(UnsupportedScope::Workspace),
            Self::Project(_) => Err(UnsupportedScope::Project),
            Self::Task(_) => Err(UnsupportedScope::Task),
        }
    }

    fn canonical_key(&self) -> (u8, &str) {
        match self {
            Self::System => (0, ""),
            Self::Human(id) => (1, id.as_str()),
            Self::Agent(id) => (2, id.as_str()),
            Self::Conversation(id) => (3, id.as_str()),
            Self::Workspace(id) => (4, id.as_str()),
            Self::Project(id) => (5, id.as_str()),
            Self::Task(id) => (6, id.as_str()),
            Self::Tool(id) => (7, id.as_str()),
        }
    }
}

/// Scope kinds that cannot be persisted until canonical tables exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnsupportedScope {
    /// Workspace persistence is not yet representable.
    Workspace,
    /// Project persistence is not yet representable.
    Project,
    /// Task persistence is not yet representable.
    Task,
}

/// Sort and deduplicate governed scopes using a stable typed ordering.
#[must_use]
pub fn canonicalize_scopes(scopes: impl IntoIterator<Item = GovernedScope>) -> Vec<GovernedScope> {
    let mut scopes = scopes.into_iter().collect::<Vec<_>>();
    scopes.sort_by(|left, right| left.canonical_key().cmp(&right.canonical_key()));
    scopes.dedup();
    scopes
}

/// Fingerprint canonical governed scopes for policy evidence.
#[must_use]
pub fn governed_scope_fingerprint(scopes: impl IntoIterator<Item = GovernedScope>) -> String {
    let canonical = canonicalize_scopes(scopes);
    let mut bytes = Vec::new();
    for scope in canonical {
        let (kind, id) = scope.canonical_key();
        bytes.push(kind);
        bytes.extend_from_slice(&(id.len() as u64).to_be_bytes());
        bytes.extend_from_slice(id.as_bytes());
    }
    URL_SAFE_NO_PAD.encode(digest(&SHA256, &bytes).as_ref())
}
