use serde::{Deserialize, Serialize};

use crate::{WorkspaceId, WorkspaceInputError};

/// The one exposed workspace in the first release.
pub const PERSONAL_WORKSPACE_ID: &str = "workspace:personal";

/// Durable workspace projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkspaceRecord {
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
    pub is_personal: bool,
    pub archived_at: Option<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkspaceRecord {
    /// Normalize and validate record-owned text and invariants.
    /// # Errors
    /// Returns [`WorkspaceInputError`] for blank required fields, a zero
    /// revision, or an archived Personal workspace.
    pub fn normalized(mut self) -> Result<Self, WorkspaceInputError> {
        self.name = non_empty(self.name, "workspace.name")?;
        if self.revision == 0 {
            return Err(WorkspaceInputError::InvalidRecord(
                "revision must be at least one",
            ));
        }
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(WorkspaceInputError::EmptyField("workspace.timestamp"));
        }
        if self.is_personal && self.archived_at.is_some() {
            return Err(WorkspaceInputError::PersonalWorkspaceArchived);
        }
        Ok(self)
    }
}

fn non_empty(value: String, field: &'static str) -> Result<String, WorkspaceInputError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(WorkspaceInputError::EmptyField(field))
    } else {
        Ok(value)
    }
}
