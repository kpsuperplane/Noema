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
pub struct WorkspaceRecord {
    /// Stable workspace identity.
    pub workspace_id: WorkspaceId,
    /// Human-visible name.
    pub name: String,
    /// Descriptive workspace context.
    pub description: String,
    /// Whether this is the seeded Personal workspace.
    pub is_personal: bool,
    /// Archive timestamp, when archived.
    pub archived_at: Option<String>,
    /// Optimistic row revision.
    pub revision: u64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl WorkspaceRecord {
    /// Normalize and validate record-owned text and invariants.
    ///
    /// # Errors
    ///
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
pub struct WorkspaceMembership {
    /// Workspace being joined.
    pub workspace_id: WorkspaceId,
    /// Existing human identity owned by the conversations domain.
    pub human_id: String,
    /// Membership role.
    pub role: WorkspaceRole,
    /// Membership creation timestamp.
    pub created_at: String,
}

impl WorkspaceMembership {
    /// Validate membership-owned fields.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceInputError`] when the human identity or creation
    /// timestamp is blank.
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

#[cfg(test)]
mod tests {
    use super::{PERSONAL_WORKSPACE_ID, WorkspaceMembership, WorkspaceRecord, WorkspaceRole};
    use crate::WorkspaceId;

    fn workspace() -> WorkspaceRecord {
        WorkspaceRecord {
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).unwrap(),
            name: " Personal ".to_string(),
            description: "A workspace".to_string(),
            is_personal: true,
            archived_at: None,
            revision: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn workspace_normalizes_name_and_rejects_personal_archive() {
        let normalized = workspace().normalized().unwrap();
        assert_eq!(normalized.name, "Personal");
        assert!(normalized.is_personal_workspace());

        let mut archived = workspace();
        archived.archived_at = Some("2026-01-02T00:00:00Z".to_string());
        assert!(archived.normalized().is_err());
    }

    #[test]
    fn workspace_rejects_blank_fields_zero_revision_and_timestamps() {
        let mut blank_name = workspace();
        blank_name.name = "  ".to_string();
        assert!(blank_name.normalized().is_err());

        let mut zero_revision = workspace();
        zero_revision.revision = 0;
        assert!(zero_revision.normalized().is_err());

        let mut blank_timestamp = workspace();
        blank_timestamp.created_at = " ".to_string();
        assert!(blank_timestamp.normalized().is_err());
    }

    #[test]
    fn membership_normalizes_human_and_rejects_blank_timestamp() {
        let membership = WorkspaceMembership {
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).unwrap(),
            human_id: " human:one ".to_string(),
            role: WorkspaceRole::Member,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(
            membership.clone().normalized().unwrap().human_id,
            "human:one"
        );

        let mut blank_human = membership;
        blank_human.human_id = " ".to_string();
        assert!(blank_human.normalized().is_err());
        let mut blank_timestamp = WorkspaceMembership {
            workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).unwrap(),
            human_id: "human:one".to_string(),
            role: WorkspaceRole::Owner,
            created_at: " ".to_string(),
        };
        assert!(blank_timestamp.clone().normalized().is_err());
        blank_timestamp.created_at = "2026-01-01T00:00:00Z".to_string();
        assert!(blank_timestamp.normalized().is_ok());
    }
}
