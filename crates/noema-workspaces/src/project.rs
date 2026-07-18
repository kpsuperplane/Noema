use serde::{Deserialize, Serialize};

use crate::{ProjectId, WorkspaceId, WorkspaceInputError};

/// Durable project projection inside one workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// Stable project identity.
    pub project_id: ProjectId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Human-visible project name.
    pub name: String,
    /// Descriptive project context.
    pub description: String,
    /// Optimistic row revision.
    pub revision: u64,
    /// Archive timestamp, when archived.
    pub archived_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl ProjectRecord {
    /// Normalize and validate project-owned fields.
    pub fn normalized(mut self) -> Result<Self, WorkspaceInputError> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty() {
            return Err(WorkspaceInputError::EmptyField("project.name"));
        }
        if self.revision == 0 {
            return Err(WorkspaceInputError::InvalidRecord(
                "revision must be at least one",
            ));
        }
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(WorkspaceInputError::EmptyField("project.timestamp"));
        }
        Ok(self)
    }

    /// A project can receive newly created or moved tasks only while active.
    #[must_use]
    pub const fn accepts_new_tasks(&self) -> bool {
        self.archived_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectRecord;
    use crate::{ProjectId, WorkspaceId};

    fn project() -> ProjectRecord {
        ProjectRecord {
            project_id: ProjectId::new("project:alpha").unwrap(),
            workspace_id: WorkspaceId::new("workspace:personal").unwrap(),
            name: " Alpha ".to_string(),
            description: "Description".to_string(),
            revision: 1,
            archived_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn project_normalizes_name_and_reports_task_acceptance() {
        let active = project().normalized().unwrap();
        assert_eq!(active.name, "Alpha");
        assert!(active.accepts_new_tasks());

        let mut archived = project();
        archived.archived_at = Some("2026-01-02T00:00:00Z".to_string());
        assert!(!archived.accepts_new_tasks());
        assert!(archived.normalized().is_ok());
    }

    #[test]
    fn project_rejects_blank_name_zero_revision_and_timestamps() {
        let mut blank_name = project();
        blank_name.name = " ".to_string();
        assert!(blank_name.normalized().is_err());

        let mut zero_revision = project();
        zero_revision.revision = 0;
        assert!(zero_revision.normalized().is_err());

        let mut blank_timestamp = project();
        blank_timestamp.updated_at = " ".to_string();
        assert!(blank_timestamp.normalized().is_err());
    }
}
