//! Provider capability-assignment persistence boundary.

use std::sync::Arc;

use noema_capabilities::{CapabilityId, ToolName};

use crate::{
    ProviderCapabilityAssignment, ProviderPersistenceError, ProviderPersistenceFuture,
    provider_capability_assignment_pair_is_supported,
};

/// Validated provider capability assignment write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertProviderCapabilityAssignmentRequest {
    /// Model-visible tool name.
    tool_name: ToolName,
    /// Provider capability selected for that tool.
    capability_id: CapabilityId,
    /// Provider account selected for the assignment.
    provider_account_id: String,
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
        provider_account_id: impl Into<String>,
    ) -> Result<Self, ProviderPersistenceError> {
        if !provider_capability_assignment_pair_is_supported(
            tool_name.as_str(),
            capability_id.as_str(),
        ) {
            return Err(ProviderPersistenceError::InvalidRequest {
                kind: "provider_capability_assignment_pair",
            });
        }
        Ok(Self {
            tool_name,
            capability_id,
            provider_account_id: provider_account_id.into(),
        })
    }

    /// Return the model-visible tool name.
    #[must_use]
    pub fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    /// Return the assigned provider capability.
    #[must_use]
    pub const fn capability_id(&self) -> CapabilityId {
        self.capability_id
    }

    /// Return the selected provider account id.
    #[must_use]
    pub fn provider_account_id(&self) -> &str {
        &self.provider_account_id
    }
}

/// Provider capability assignment reads and writes.
pub trait ProviderCapabilityAssignmentPersistence: Send + Sync {
    /// Read one assignment by tool and capability.
    fn provider_capability_assignment<'a>(
        &'a self,
        tool_name: &'a ToolName,
        capability_id: CapabilityId,
    ) -> ProviderPersistenceFuture<'a, Option<ProviderCapabilityAssignment>>;

    /// Create or replace one assignment.
    fn upsert_provider_capability_assignment(
        &self,
        request: UpsertProviderCapabilityAssignmentRequest,
    ) -> ProviderPersistenceFuture<'_, ProviderCapabilityAssignment>;
}

/// Clonable provider capability-assignment persistence handle.
pub type ProviderCapabilityAssignmentPersistenceHandle =
    Arc<dyn ProviderCapabilityAssignmentPersistence>;
