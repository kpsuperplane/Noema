use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{WorkspaceId, WorkspaceInputError};

/// The one exposed workspace in the first release.
pub const PERSONAL_WORKSPACE_ID: &str = "workspace:personal";

/// Membership role persisted for a workspace member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    /// Owns the workspace.
    Owner,
    /// Has membership visibility without ownership.
    Member,
}

impl WorkspaceRole {
    /// Return the stable persisted value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Member => "member",
        }
    }
}

impl fmt::Display for WorkspaceRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for WorkspaceRole {
    type Err = WorkspaceInputError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "member" => Ok(Self::Member),
            value => Err(WorkspaceInputError::InvalidEnum {
                kind: "workspace_role",
                value: value.to_string(),
            }),
        }
    }
}

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

    /// Whether this row is the seeded Personal workspace.
    #[must_use]
    pub fn is_personal_workspace(&self) -> bool {
        self.is_personal && self.workspace_id.as_str() == PERSONAL_WORKSPACE_ID
    }
}

/// Durable membership projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct WorkspaceMembership {
    pub workspace_id: WorkspaceId,
    pub human_id: String,
    pub role: WorkspaceRole,
    pub created_at: String,
}

impl WorkspaceMembership {
    /// Validate membership-owned fields.
    /// # Errors
    /// Returns [`WorkspaceInputError`] for blank required fields, a zero
    /// revision, or an archived Personal workspace.
    pub fn normalized(mut self) -> Result<Self, WorkspaceInputError> {
        self.human_id = non_empty(self.human_id, "workspace_membership.human_id")?;
        if self.created_at.trim().is_empty() {
            return Err(WorkspaceInputError::EmptyField(
                "workspace_membership.created_at",
            ));
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
