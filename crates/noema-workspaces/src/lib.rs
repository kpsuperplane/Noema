//! Durable workspace and project domain contracts.
//!
//! This crate owns workspace/project identities and records.  It deliberately
//! contains no persistence, task-stage, capability, or runtime behavior.

mod error;
mod ids;
mod project;
mod workspace;

pub use error::WorkspaceInputError;
pub use ids::{ProjectId, WorkspaceId};
pub use project::ProjectRecord;
pub use workspace::{PERSONAL_WORKSPACE_ID, WorkspaceMembership, WorkspaceRecord, WorkspaceRole};
