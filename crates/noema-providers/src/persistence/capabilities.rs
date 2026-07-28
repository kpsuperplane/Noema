//! Provider capability-assignment persistence boundary.

use noema_capabilities::{CapabilityId, ToolName};

use crate::{
    ProviderCapabilityAssignment, ProviderPersistenceError, ProviderPersistenceFuture,
    provider_capability_assignment_pair_is_supported, system_provider_accounts,
};

/// How a validated capability assignment refers to its provider account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCapabilityAccountReferenceMode {
    /// The account must exist in durable provider-account storage.
    Persisted,
    /// A consuming provider boundary already validated a built-in system account.
    ValidatedSystem,
}

/// Typed provider-account reference for one capability assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityAccountReference {
    provider_account_id: String,
    mode: ProviderCapabilityAccountReferenceMode,
}

impl ProviderCapabilityAccountReference {
    /// Reference an account that must exist in durable storage.
    #[must_use]
    pub fn persisted(provider_account_id: impl Into<String>) -> Self {
        Self {
            provider_account_id: provider_account_id.into(),
            mode: ProviderCapabilityAccountReferenceMode::Persisted,
        }
    }

    /// Reference a provider-owned built-in system account.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderPersistenceError::InvalidRequest`] when the id does
    /// not identify a declared system provider account.
    pub fn validated_system(
        provider_account_id: impl Into<String>,
    ) -> Result<Self, ProviderPersistenceError> {
        let provider_account_id = provider_account_id.into();
        if !system_provider_accounts()
            .iter()
            .any(|account| account.provider_account_id == provider_account_id)
        {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_assignment_system_account",
            });
        }
        Ok(Self {
            provider_account_id,
            mode: ProviderCapabilityAccountReferenceMode::ValidatedSystem,
        })
    }

    /// Return the referenced provider account id.
    #[must_use]
    pub fn provider_account_id(&self) -> &str {
        &self.provider_account_id
    }

    /// Return how persistence must interpret this reference.
    #[must_use]
    pub const fn mode(&self) -> ProviderCapabilityAccountReferenceMode {
        self.mode
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

    /// Create or replace one assignment.
    fn upsert_provider_capability_assignment(
        &self,
        request: UpsertProviderCapabilityAssignmentRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderCapabilityAssignment>;

    /// Remove a set of assignments atomically.
    fn clear_provider_capability_assignments<'a>(
        &'a self,
        keys: &'a [ProviderCapabilityAssignmentKey],
    ) -> ProviderPersistenceFuture<'a, ()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validated_system_reference_rejects_undeclared_account() {
        let error =
            ProviderCapabilityAccountReference::validated_system("provider_account:unknown:system")
                .expect_err("undeclared system account");

        assert_eq!(
            error,
            ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_assignment_system_account",
            }
        );
    }
}
