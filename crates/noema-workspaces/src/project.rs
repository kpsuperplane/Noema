use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{ProjectId, WorkspaceId, WorkspaceInputError};

/// Durable project projection inside one workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct ProjectRecord {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
    pub folder: Option<String>,
    pub revision: u64,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ProjectRecord {
    /// Normalize and validate project-owned fields.
    /// # Errors
    /// Returns [`WorkspaceInputError`] when the name or timestamps are blank,
    /// or when the persisted revision is zero.
    pub fn normalized(mut self) -> Result<Self, WorkspaceInputError> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty() {
            return Err(WorkspaceInputError::EmptyField("project.name"));
        }
        self.folder = self
            .folder
            .take()
            .map(|folder| folder.trim().to_owned())
            .filter(|folder| !folder.is_empty());
        if self
            .folder
            .as_deref()
            .is_some_and(|folder| folder.contains('\0') || !Path::new(folder).is_absolute())
        {
            return Err(WorkspaceInputError::InvalidFolder);
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
}
