//! Provider capability-assignment persistence boundary.

use noema_capabilities::{CapabilityId, ToolName};

use crate::{
    ProviderCapabilityAssignment, ProviderPersistenceError, ProviderPersistenceFuture,
    provider_capability_assignment_pair_is_supported,
};

/// Validated replacement for one ordered provider capability route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceProviderCapabilityRouteRequest {
    key: ProviderCapabilityAssignmentKey,
    account_references: Vec<ProviderCapabilityAccountReference>,
}

impl ReplaceProviderCapabilityRouteRequest {
    /// Build one non-empty route for a supported tool and capability.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty route, duplicate accounts, or multiple
    /// assignments outside `web.browse`.
    pub fn new(
        tool_name: ToolName,
        capability_id: CapabilityId,
        account_references: Vec<ProviderCapabilityAccountReference>,
    ) -> Result<Self, ProviderPersistenceError> {
        let key = ProviderCapabilityAssignmentKey::new(tool_name, capability_id)?;
        if account_references.is_empty() {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_route_empty",
            });
        }
        if account_references.len() > 1 && key.tool_name_str() != "web.browse" {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_route_unsupported",
            });
        }
        let mut account_ids = std::collections::HashSet::new();
        if account_references
            .iter()
            .any(|reference| !account_ids.insert(reference.provider_account_id()))
        {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_route_duplicate",
            });
        }
        Ok(Self {
            key,
            account_references,
        })
    }

    /// Return the route key.
    #[must_use]
    pub const fn key(&self) -> &ProviderCapabilityAssignmentKey {
        &self.key
    }

    /// Return provider accounts in route order.
    #[must_use]
    pub fn account_references(&self) -> &[ProviderCapabilityAccountReference] {
        &self.account_references
    }
}

/// Typed provider-account reference for one capability assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityAccountReference {
    provider_account_id: String,
}

impl ProviderCapabilityAccountReference {
    /// Reference an account that must exist in durable storage.
    #[must_use]
    pub fn persisted(provider_account_id: impl Into<String>) -> Self {
        Self {
            provider_account_id: provider_account_id.into(),
        }
    }

    /// Return the referenced provider account id.
    #[must_use]
    pub fn provider_account_id(&self) -> &str {
        &self.provider_account_id
    }
}

/// Validated tool/capability key for assignment reads and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityAssignmentKey {
    tool_name: String,
    capability_id: String,
}

impl ProviderCapabilityAssignmentKey {
    /// Build one supported tool-to-capability assignment key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderPersistenceError::InvalidRequest`] when the tool and
    /// capability do not form a supported provider assignment.
    pub fn new(
        tool_name: ToolName,
        capability_id: CapabilityId,
    ) -> Result<Self, ProviderPersistenceError> {
        Self::from_storage_values(tool_name.as_str(), capability_id.as_str())
    }

    /// Build a supported assignment key from persistence-facing strings.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderPersistenceError::InvalidRequest`] when the values do
    /// not form a supported provider assignment.
    pub fn from_storage_values(
        tool_name: impl Into<String>,
        capability_id: impl Into<String>,
    ) -> Result<Self, ProviderPersistenceError> {
        let tool_name = tool_name.into();
        let capability_id = capability_id.into();
        if !provider_capability_assignment_pair_is_supported(&tool_name, &capability_id) {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_assignment_pair",
            });
        }
        Ok(Self {
            tool_name,
            capability_id,
        })
    }

    /// Return the model-visible tool name as a storage string.
    #[must_use]
    pub fn tool_name_str(&self) -> &str {
        &self.tool_name
    }

    /// Return the capability id as a storage string.
    #[must_use]
    pub fn capability_id_str(&self) -> &str {
        &self.capability_id
    }
}

/// Validated provider capability assignment write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertProviderCapabilityAssignmentRequest {
    key: ProviderCapabilityAssignmentKey,
    /// Provider account selected for the assignment.
    account_reference: ProviderCapabilityAccountReference,
}

impl UpsertProviderCapabilityAssignmentRequest {
    /// Build one supported tool-to-capability assignment request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderPersistenceError::InvalidRequest`] when the tool and
    /// capability do not form a supported provider assignment.
    pub fn new(
        tool_name: ToolName,
        capability_id: CapabilityId,
        account_reference: ProviderCapabilityAccountReference,
    ) -> Result<Self, ProviderPersistenceError> {
        let key = ProviderCapabilityAssignmentKey::new(tool_name, capability_id)?;
        Ok(Self {
            key,
            account_reference,
        })
    }

    /// Build one supported assignment request from persistence-facing strings.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderPersistenceError::InvalidRequest`] when the values do
    /// not form a supported provider assignment.
    pub fn from_storage_values(
        tool_name: impl Into<String>,
        capability_id: impl Into<String>,
        account_reference: ProviderCapabilityAccountReference,
    ) -> Result<Self, ProviderPersistenceError> {
        Ok(Self {
            key: ProviderCapabilityAssignmentKey::from_storage_values(tool_name, capability_id)?,
            account_reference,
        })
    }

    /// Return the model-visible tool name as a storage string.
    #[must_use]
    pub fn tool_name_str(&self) -> &str {
        self.key.tool_name_str()
    }

    /// Return the assigned provider capability as a storage string.
    #[must_use]
    pub fn capability_id_str(&self) -> &str {
        self.key.capability_id_str()
    }

    /// Return the selected provider account id.
    #[must_use]
    pub fn provider_account_id(&self) -> &str {
        self.account_reference.provider_account_id()
    }

    /// Return the typed provider-account reference selected by the caller.
    #[must_use]
    pub const fn account_reference(&self) -> &ProviderCapabilityAccountReference {
        &self.account_reference
    }
}

/// Provider capability assignment reads and writes.
pub trait ProviderCapabilityAssignmentPersistence: Send + Sync {
    /// Read one assignment by tool and capability.
    fn provider_capability_assignment<'a>(
        &'a self,
        key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderCapabilityAssignment>>;

    /// Read all assignments in route order.
    fn provider_capability_route<'a>(
        &'a self,
        key: &'a ProviderCapabilityAssignmentKey,
    ) -> ProviderPersistenceFuture<'a, Vec<ProviderCapabilityAssignment>>;

    /// Create or replace one assignment.
    fn upsert_provider_capability_assignment(
        &self,
        request: UpsertProviderCapabilityAssignmentRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderCapabilityAssignment>;

    /// Replace one ordered route atomically.
    fn replace_provider_capability_route(
        &self,
        request: ReplaceProviderCapabilityRouteRequest,
    ) -> ProviderPersistenceFuture<'_, Vec<ProviderCapabilityAssignment>>;

    /// Remove a set of assignments atomically.
    fn clear_provider_capability_assignments<'a>(
        &'a self,
        keys: &'a [ProviderCapabilityAssignmentKey],
    ) -> ProviderPersistenceFuture<'a, ()>;
}
