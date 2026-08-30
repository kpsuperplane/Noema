//! Mutable Markdown owned by one project.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use cap_std::{ambient_authority, fs::Dir};
use noema_workspaces::{ProjectId, ProjectRecord};
use thiserror::Error;

use crate::{NoemaStore, StoreError, TASK_FILE_TEXT_LIMIT};

/// Durable project context document.
pub const PROJECT_DOCUMENT: &str = "PROJECT.md";

/// One exact project document and its transient write digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDocumentRead {
    /// Exact UTF-8 Markdown.
    pub content: String,
    /// Lower-case SHA-256 of the content bytes.
    pub digest: String,
}

/// Prepared working-folder change which can be rolled back before commit.
#[derive(Debug)]
pub struct ProjectFileMove {
    created_path: Option<PathBuf>,
}

/// A project document operation failed.
#[derive(Debug, Error)]
pub enum ProjectFileError {
    /// The project document context is unavailable.
    #[error("Project document context is unavailable")]
    Unavailable,
    /// A symbolic link appeared in the project document path.
    #[error("Project document path contains a symbolic link")]
    SymbolicLink,
    /// The project document is not a regular UTF-8 file.
    #[error("Project document is not a regular UTF-8 file")]
    InvalidFile,
    /// The project document exceeds the model-facing limit.
    #[error("Project document exceeds the 64 KiB model-facing limit")]
    TooLarge,
    /// A new working folder contains different project context.
    #[error("The destination folder contains a different PROJECT.md")]
    Conflict,
    /// The saved project document changed after it was read.
    #[error("Project document changed elsewhere")]
    StaleDigest,
    /// The filesystem operation failed.
    #[error("Project document operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// The project record could not be read.
    #[error("Project document context could not be loaded: {0}")]
    Store(#[from] StoreError),
}

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl NoemaStore {
    /// Read the current project document.
    ///
    /// # Errors
    ///
    /// Returns an error when the project or its document is unavailable.
    pub async fn read_project_document(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectDocumentRead, ProjectFileError> {
        let project = self.project_record(project_id).await?;
        read_document_at(&project_document_path(&self.home_root, &project))
    }

    /// Create a missing project document without replacing an existing file.
    pub(crate) async fn ensure_project_document_from(
        &self,
        project: &ProjectRecord,
        content: &str,
    ) -> Result<bool, ProjectFileError> {
        ensure_document_at(&project_document_path(&self.home_root, project), content)
    }

    /// Replace a project document after checking its exact current digest.
    pub(crate) async fn replace_project_document(
        &self,
        project_id: &ProjectId,
        expected_digest: &str,
        content: &str,
    ) -> Result<String, ProjectFileError> {
        if content.len() > TASK_FILE_TEXT_LIMIT {
            return Err(ProjectFileError::TooLarge);
        }
        let project = self.project_record(project_id).await?;
        let path = project_document_path(&self.home_root, &project);
        let current = read_document_at(&path)?;
        if current.digest != expected_digest {
            return Err(ProjectFileError::StaleDigest);
        }
        write_document_at(&path, content)?;
        Ok(current.content)
    }

    /// Restore a project document after a failed stored-state update.
    pub(crate) async fn restore_project_document(
        &self,
        project_id: &ProjectId,
        content: &str,
    ) -> Result<(), ProjectFileError> {
        let project = self.project_record(project_id).await?;
        write_document_at(&project_document_path(&self.home_root, &project), content)
    }

    /// Prepare the document destination for a working-folder change.
    pub(crate) async fn prepare_project_document_move(
        &self,
        project_id: &ProjectId,
        next_folder: Option<&str>,
    ) -> Result<ProjectFileMove, ProjectFileError> {
        let project = self.project_record(project_id).await?;
        let source = project_document_path(&self.home_root, &project);
        let destination = document_path(
            &self.home_root,
            &project.workspace_id.to_string(),
            &project.project_id.to_string(),
            next_folder,
        );
        if source == destination {
            return Ok(ProjectFileMove { created_path: None });
        }
        let current = read_document_at(&source)?;
        match read_document_at(&destination) {
            Ok(existing) if existing.content == current.content => {
                Ok(ProjectFileMove { created_path: None })
            }
            Ok(_) => Err(ProjectFileError::Conflict),
            Err(ProjectFileError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                write_document_at(&destination, &current.content)?;
                Ok(ProjectFileMove {
                    created_path: Some(destination),
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Remove only the destination file created during move preparation.
    pub(crate) fn rollback_project_document_move(
        prepared: ProjectFileMove,
    ) -> Result<(), ProjectFileError> {
        if let Some(path) = prepared.created_path {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    async fn project_record(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectRecord, ProjectFileError> {
        let project_id = project_id.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            crate::work_reads::rows::load_project_optional(&transaction, &project_id)
        })
        .await?
        .ok_or(ProjectFileError::Unavailable)
    }
}

pub(crate) fn default_project_document(name: &str, description: &str) -> String {
    let mut content = format!("# {}\n", name.trim());
    if !description.trim().is_empty() {
        content.push('\n');
        content.push_str(description.trim());
        content.push('\n');
    }
    content
}

pub(crate) fn ensure_migrated_project_document(
    home_root: &Path,
    workspace_id: &str,
    project_id: &str,
    name: &str,
    description: &str,
    folder: Option<&str>,
) -> Result<(), ProjectFileError> {
    let path = document_path(home_root, workspace_id, project_id, folder);
    ensure_document_at(&path, &default_project_document(name, description)).map(drop)
}

fn project_document_path(home_root: &Path, project: &ProjectRecord) -> PathBuf {
    document_path(
        home_root,
        project.workspace_id.as_str(),
        project.project_id.as_str(),
        project.folder.as_deref(),
    )
}

fn document_path(
    home_root: &Path,
    workspace_id: &str,
    project_id: &str,
    folder: Option<&str>,
) -> PathBuf {
    folder.map_or_else(
        || {
            home_root
                .join("workspaces")
                .join(id_suffix(workspace_id))
                .join("projects")
                .join(id_suffix(project_id))
                .join("docs")
                .join(PROJECT_DOCUMENT)
        },
        |folder| PathBuf::from(folder).join(PROJECT_DOCUMENT),
    )
}

fn id_suffix(value: &str) -> &str {
    value.split_once(':').map_or(value, |(_, suffix)| suffix)
}

fn ensure_document_at(path: &Path, content: &str) -> Result<bool, ProjectFileError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ProjectFileError::SymbolicLink),
        Ok(metadata) if metadata.is_file() => {
            read_document_at(path)?;
            Ok(true)
        }
        Ok(_) => Err(ProjectFileError::InvalidFile),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_document_at(path, content)?;
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

fn read_document_at(path: &Path) -> Result<ProjectDocumentRead, ProjectFileError> {
    let parent = path.parent().ok_or(ProjectFileError::InvalidFile)?;
    verify_ambient_components(parent)?;
    let root = Dir::open_ambient_dir(parent, ambient_authority())?;
    let name = Path::new(path.file_name().ok_or(ProjectFileError::InvalidFile)?);
    let metadata = root.symlink_metadata(name)?;
    if metadata.file_type().is_symlink() {
        return Err(ProjectFileError::SymbolicLink);
    }
    if !metadata.is_file() {
        return Err(ProjectFileError::InvalidFile);
    }
    if metadata.len() > TASK_FILE_TEXT_LIMIT as u64 {
        return Err(ProjectFileError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    root.open(name)?
        .take((TASK_FILE_TEXT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > TASK_FILE_TEXT_LIMIT {
        return Err(ProjectFileError::TooLarge);
    }
    let content = String::from_utf8(bytes).map_err(|_| ProjectFileError::InvalidFile)?;
    let digest = crate::work_row::sha256_hex(content.as_bytes());
    Ok(ProjectDocumentRead { content, digest })
}

fn write_document_at(path: &Path, content: &str) -> Result<(), ProjectFileError> {
    if content.len() > TASK_FILE_TEXT_LIMIT {
        return Err(ProjectFileError::TooLarge);
    }
    let parent = path.parent().ok_or(ProjectFileError::InvalidFile)?;
    ensure_ambient_directories(parent)?;
    let root = Dir::open_ambient_dir(parent, ambient_authority())?;
    let name = Path::new(path.file_name().ok_or(ProjectFileError::InvalidFile)?);
    match root.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(ProjectFileError::SymbolicLink);
        }
        Ok(metadata) if !metadata.is_file() => return Err(ProjectFileError::InvalidFile),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{}.noema-{}-{sequence}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let temporary_name = Path::new(temporary.file_name().ok_or(ProjectFileError::InvalidFile)?);
    let mut options = cap_std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = root.open_with(temporary_name, &options)?;
    let result = (|| {
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        root.rename(temporary_name, &root, name)?;
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = root.remove_file(temporary_name);
    }
    result.map_err(ProjectFileError::from)
}

fn verify_ambient_components(path: &Path) -> Result<(), ProjectFileError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)?
            .file_type()
            .is_symlink()
        {
            return Err(ProjectFileError::SymbolicLink);
        }
    }
    Ok(())
}

fn ensure_ambient_directories(path: &Path) -> Result<(), ProjectFileError> {
    let root = Dir::open_ambient_dir(Path::new("/"), ambient_authority())?;
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::RootDir => continue,
            std::path::Component::Normal(value) => relative.push(value),
            _ => return Err(ProjectFileError::InvalidFile),
        }
        match root.symlink_metadata(&relative) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ProjectFileError::SymbolicLink);
            }
            Ok(metadata) if !metadata.is_dir() => return Err(ProjectFileError::InvalidFile),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                root.create_dir(&relative)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
