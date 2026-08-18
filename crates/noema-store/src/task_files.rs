//! Mutable text files owned by one Task working directory.

use std::{
    ffi::OsStr,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use cap_std::{ambient_authority, fs::Dir};
use noema_tasks::TaskId;
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{NoemaStore, StoreError};

/// Main mutable Task document.
pub const TASK_DOCUMENT: &str = "TASK.md";
/// Current mutable submitted result.
pub const TASK_RESULT: &str = "RESULT.md";
/// Current mutable review feedback.
pub const TASK_REVIEW: &str = "REVIEW.md";
/// Maximum UTF-8 text accepted by one model-facing file operation.
pub const TASK_FILE_TEXT_LIMIT: usize = 64 * 1024;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn allocate_task_directory_tx(
    transaction: &Transaction<'_>,
    workspace_id: &WorkspaceId,
    project_id: Option<&ProjectId>,
    title: &str,
) -> Result<String, StoreError> {
    let base = task_slug(title);
    for suffix in 1_u32..=u32::MAX {
        let candidate = if suffix == 1 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let existing = transaction
            .query_row(
                "SELECT 1 FROM tasks WHERE workspace_id = ?1 AND project_id IS ?2 AND task_directory = ?3 LIMIT 1",
                params![
                    workspace_id.as_str(),
                    project_id.map(ProjectId::as_str),
                    candidate,
                ],
                |_| Ok(()),
            )
            .optional()?;
        if existing.is_none() {
            return Ok(candidate);
        }
    }
    Err(StoreError::InvariantViolation {
        message: "Task directory suffix space is exhausted".to_string(),
    })
}

fn task_slug(title: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in title.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else {
            separator = true;
        }
        if slug.len() >= 64 {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "task".to_string()
    } else {
        slug
    }
}

/// One safe directory-list entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFileEntry {
    /// Path relative to the Task working directory.
    pub path: String,
    /// Whether the entry is a directory.
    pub is_directory: bool,
    /// File size for regular files.
    pub size_bytes: Option<u64>,
}

/// A Task file request failed without disclosing file contents.
#[derive(Debug, Error)]
pub enum TaskFileError {
    /// The current Task or its filesystem context is unavailable.
    #[error("Task file context is unavailable")]
    Unavailable,
    /// The supplied path leaves the authorized Task or project boundary.
    #[error("Task file path is outside the authorized boundary")]
    UnsafePath,
    /// A symbolic link appeared in an authorized path.
    #[error("Task file path contains a symbolic link")]
    SymbolicLink,
    /// The requested entry is not a regular UTF-8 text file.
    #[error("Task file is not a regular UTF-8 text file")]
    InvalidFile,
    /// The requested model-facing operation exceeds its text limit.
    #[error("Task file exceeds the 64 KiB model-facing limit")]
    TooLarge,
    /// Required Task documents cannot be removed through Task file tools.
    #[error("TASK.md and RESULT.md cannot be deleted")]
    RequiredDocument,
    /// The filesystem operation failed.
    #[error("Task file operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// The current Task could not be read.
    #[error("Task file context could not be loaded: {0}")]
    Store(#[from] StoreError),
}

#[derive(Debug)]
struct TaskFileAccess {
    task_root: PathBuf,
    boundary_root: PathBuf,
    task_relative: PathBuf,
    boundary: Dir,
}

impl NoemaStore {
    /// Create `TASK.md` from the current authenticated Task body when absent.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task or its working directory is unsafe.
    pub async fn ensure_task_document(&self, task_id: &TaskId) -> Result<(), TaskFileError> {
        let task_id_for_read = task_id.clone();
        let (title, description) = self
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT title, description_markdown FROM tasks WHERE task_id = ?1",
                        [task_id_for_read.as_str()],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .map_err(StoreError::from)
            })
            .await?;
        let content = if description.trim().is_empty() {
            format!("# {title}\n")
        } else {
            format!("# {title}\n\n{}\n", description.trim())
        };
        self.ensure_task_document_from(task_id, &content).await
    }

    pub(crate) async fn ensure_task_document_from(
        &self,
        task_id: &TaskId,
        content: &str,
    ) -> Result<(), TaskFileError> {
        let access = self.task_file_access(task_id).await?;
        let relative = access.task_relative.join(TASK_DOCUMENT);
        match access.boundary.symlink_metadata(&relative) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TaskFileError::SymbolicLink);
            }
            Ok(metadata) if metadata.is_file() => return Ok(()),
            Ok(_) => return Err(TaskFileError::InvalidFile),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if content.len() > TASK_FILE_TEXT_LIMIT {
            return Err(TaskFileError::TooLarge);
        }
        atomic_write(&access.boundary, &relative, content.as_bytes())
    }

    /// Resolve the current Task working directory from live Task and project rows.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task or path is unavailable.
    pub async fn task_working_directory(&self, task_id: &TaskId) -> Result<PathBuf, TaskFileError> {
        Ok(self.task_file_access(task_id).await?.task_root)
    }

    /// List one Task or project directory without following symbolic links.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task, path, or directory is unsafe.
    pub async fn list_task_files(
        &self,
        task_id: &TaskId,
        path: &str,
    ) -> Result<Vec<TaskFileEntry>, TaskFileError> {
        let access = self.task_file_access(task_id).await?;
        let relative = access.resolve(path, true)?;
        verify_components(&access.boundary, &relative, false)?;
        let directory = access
            .boundary
            .open_dir(if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &relative
            })?;
        let mut entries = Vec::new();
        for entry in directory.entries()? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let entry_relative = relative.join(&name);
            entries.push(TaskFileEntry {
                path: access.task_path(&entry_relative)?,
                is_directory: metadata.is_dir(),
                size_bytes: metadata.is_file().then_some(metadata.len()),
            });
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    /// Read one bounded UTF-8 Task or project file.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task, path, file, or content is unsafe.
    pub async fn read_task_file(
        &self,
        task_id: &TaskId,
        path: &str,
    ) -> Result<String, TaskFileError> {
        let access = self.task_file_access(task_id).await?;
        let relative = access.resolve(path, false)?;
        verify_components(&access.boundary, &relative, false)?;
        let metadata = access.boundary.symlink_metadata(&relative)?;
        if metadata.file_type().is_symlink() {
            return Err(TaskFileError::SymbolicLink);
        }
        if !metadata.is_file() {
            return Err(TaskFileError::InvalidFile);
        }
        if metadata.len() > TASK_FILE_TEXT_LIMIT as u64 {
            return Err(TaskFileError::TooLarge);
        }
        let file = access.boundary.open(&relative)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take((TASK_FILE_TEXT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > TASK_FILE_TEXT_LIMIT {
            return Err(TaskFileError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| TaskFileError::InvalidFile)
    }

    /// Atomically create or replace one bounded UTF-8 Task file.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task, path, or content is unsafe.
    pub async fn write_task_file(
        &self,
        task_id: &TaskId,
        path: &str,
        content: &str,
    ) -> Result<(), TaskFileError> {
        if content.len() > TASK_FILE_TEXT_LIMIT {
            return Err(TaskFileError::TooLarge);
        }
        let access = self.task_file_access(task_id).await?;
        let relative = access.resolve_task_owned(path)?;
        let parent = relative.parent().ok_or(TaskFileError::UnsafePath)?;
        ensure_directories(&access.boundary, parent)?;
        verify_components(&access.boundary, parent, false)?;
        if let Ok(metadata) = access.boundary.symlink_metadata(&relative) {
            if metadata.file_type().is_symlink() {
                return Err(TaskFileError::SymbolicLink);
            }
            if !metadata.is_file() {
                return Err(TaskFileError::InvalidFile);
            }
        }
        atomic_write(&access.boundary, &relative, content.as_bytes())
    }

    /// Delete one regular Task file while preserving required Task documents.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task or path is unsafe.
    pub async fn delete_task_file(
        &self,
        task_id: &TaskId,
        path: &str,
    ) -> Result<(), TaskFileError> {
        let access = self.task_file_access(task_id).await?;
        let relative = access.resolve_task_owned(path)?;
        if relative == access.task_relative.join(TASK_DOCUMENT)
            || relative == access.task_relative.join(TASK_RESULT)
        {
            return Err(TaskFileError::RequiredDocument);
        }
        verify_components(&access.boundary, &relative, false)?;
        let metadata = access.boundary.symlink_metadata(&relative)?;
        if metadata.file_type().is_symlink() {
            return Err(TaskFileError::SymbolicLink);
        }
        if !metadata.is_file() {
            return Err(TaskFileError::InvalidFile);
        }
        access.boundary.remove_file(&relative)?;
        Ok(())
    }

    /// Replace current Reviewer feedback without granting Reviewer file writes.
    ///
    /// # Errors
    /// Returns [`TaskFileError`] when the Task file context is unavailable.
    pub async fn replace_task_review(
        &self,
        task_id: &TaskId,
        content: &str,
    ) -> Result<(), TaskFileError> {
        self.write_task_file(task_id, TASK_REVIEW, content).await
    }

    async fn task_file_access(&self, task_id: &TaskId) -> Result<TaskFileAccess, TaskFileError> {
        let task_id = task_id.clone();
        let (cwd_override, task_directory, project_folder) = self
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT t.cwd_override, t.task_directory, p.folder, t.task_id FROM tasks t LEFT JOIN projects p ON p.project_id = t.project_id WHERE t.task_id = ?1",
                        [task_id.as_str()],
                        |row| {
                            let directory = row.get::<_, Option<String>>(1)?.unwrap_or_else(|| {
                                row.get::<_, String>(3)
                                    .unwrap_or_else(|_| "task:task".to_string())
                                    .trim_start_matches("task:")
                                    .replace(':', "-")
                            });
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                directory,
                                row.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .map_err(StoreError::from)
            })
            .await?;
        let task_root = cwd_override
            .map(|directory| PathBuf::from(directory).join(&task_directory))
            .or_else(|| {
                project_folder
                    .as_ref()
                    .map(|folder| PathBuf::from(folder).join(&task_directory))
            })
            .unwrap_or_else(|| self.home_root.join("tasks").join(&task_directory));
        let boundary_root = project_folder
            .map(PathBuf::from)
            .filter(|project| task_root.starts_with(project))
            .unwrap_or_else(|| task_root.clone());
        std::fs::create_dir_all(&task_root)?;
        let boundary_root = std::path::absolute(boundary_root)?;
        let task_root = std::path::absolute(task_root)?;
        let task_relative = task_root
            .strip_prefix(&boundary_root)
            .map_err(|_| TaskFileError::UnsafePath)?
            .to_path_buf();
        reject_ambient_symlink_components(&boundary_root)?;
        reject_ambient_symlink_components(&task_root)?;
        let boundary = Dir::open_ambient_dir(&boundary_root, ambient_authority())?;
        Ok(TaskFileAccess {
            task_root,
            boundary_root,
            task_relative,
            boundary,
        })
    }
}

impl TaskFileAccess {
    fn resolve(&self, supplied: &str, allow_empty: bool) -> Result<PathBuf, TaskFileError> {
        let supplied = Path::new(supplied.trim());
        if supplied.is_absolute() {
            return Err(TaskFileError::UnsafePath);
        }
        let mut resolved = self.task_relative.clone();
        let mut saw_normal = false;
        for component in supplied.components() {
            match component {
                Component::Normal(value) => {
                    resolved.push(value);
                    saw_normal = true;
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if !resolved.pop() {
                        return Err(TaskFileError::UnsafePath);
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(TaskFileError::UnsafePath);
                }
            }
        }
        if !allow_empty && !saw_normal {
            return Err(TaskFileError::UnsafePath);
        }
        Ok(resolved)
    }

    fn resolve_task_owned(&self, supplied: &str) -> Result<PathBuf, TaskFileError> {
        let relative = self.resolve(supplied, false)?;
        if !relative.starts_with(&self.task_relative) {
            return Err(TaskFileError::UnsafePath);
        }
        Ok(relative)
    }

    fn task_path(&self, boundary_relative: &Path) -> Result<String, TaskFileError> {
        let absolute = self.boundary_root.join(boundary_relative);
        let relative = pathdiff(&self.task_root, &absolute)?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}

fn pathdiff(from: &Path, to: &Path) -> Result<PathBuf, TaskFileError> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return Err(TaskFileError::UnsafePath);
    }
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &to[common..] {
        result.push(component.as_os_str());
    }
    Ok(result)
}

fn reject_ambient_symlink_components(path: &Path) -> Result<(), TaskFileError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(TaskFileError::SymbolicLink);
        }
    }
    Ok(())
}

fn verify_components(root: &Dir, path: &Path, allow_missing: bool) -> Result<(), TaskFileError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(TaskFileError::UnsafePath);
        };
        current.push(value);
        match root.symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TaskFileError::SymbolicLink);
            }
            Ok(_) => {}
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_directories(root: &Dir, path: &Path) -> Result<(), TaskFileError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(TaskFileError::UnsafePath);
        };
        current.push(value);
        match root.symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TaskFileError::SymbolicLink);
            }
            Ok(metadata) if !metadata.is_dir() => return Err(TaskFileError::InvalidFile),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                root.create_dir(&current)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn atomic_write(root: &Dir, path: &Path, content: &[u8]) -> Result<(), TaskFileError> {
    let parent = path.parent().ok_or(TaskFileError::UnsafePath)?;
    let name = path.file_name().ok_or(TaskFileError::UnsafePath)?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary_name = format!(
        ".{}.noema-{}-{sequence}",
        name.to_string_lossy(),
        std::process::id()
    );
    let temporary = parent.join(OsStr::new(&temporary_name));
    let mut options = cap_std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = root.open_with(&temporary, &options)?;
    let result = (|| {
        file.write_all(content)?;
        file.sync_all()?;
        root.rename(&temporary, root, path)?;
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = root.remove_file(&temporary);
    }
    result.map_err(TaskFileError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoreConfig;

    #[test]
    fn relative_paths_can_reach_project_but_not_escape_it() {
        let boundary_root = PathBuf::from("/project");
        let task_root = boundary_root.join("task");
        let access = TaskFileAccess {
            task_relative: PathBuf::from("task"),
            boundary: Dir::open_ambient_dir("/", ambient_authority()).expect("root"),
            task_root,
            boundary_root,
        };

        assert_eq!(
            access.resolve("../shared.md", false).unwrap(),
            Path::new("shared.md")
        );
        assert!(matches!(
            access.resolve("../../outside", false),
            Err(TaskFileError::UnsafePath)
        ));
        assert!(matches!(
            access.resolve_task_owned("../shared.md"),
            Err(TaskFileError::UnsafePath)
        ));
    }

    #[test]
    fn task_slug_is_human_readable_and_bounded() {
        assert_eq!(task_slug(" Check 100 Websites "), "check-100-websites");
        assert_eq!(task_slug("日本語 の Task"), "日本語-の-task");
        assert_eq!(task_slug("---"), "task");
        assert!(task_slug(&"a".repeat(100)).len() <= 64);
    }

    #[tokio::test]
    async fn task_files_use_the_task_directory_and_project_boundary() {
        let home = tempfile::tempdir().expect("home");
        let project = home.path().join("project");
        std::fs::create_dir(&project).expect("project");
        std::fs::write(project.join("shared.md"), "shared").expect("shared file");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("db/noema.sqlite3")))
            .await
            .expect("store");
        let project_text = project.to_string_lossy().into_owned();
        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO projects (project_id, workspace_id, name, description, folder) VALUES ('project:files', 'workspace:personal', 'Files', '', ?1)",
                    [&project_text],
                )?;
                connection.execute(
                    "INSERT INTO tasks (task_id, workspace_id, project_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, task_directory, source_kind, created_by_actor_id) VALUES ('task:files', 'workspace:personal', 'project:files', 'workflow:personal:default', 'stage:personal:queue', 'Long Task', 'Keep durable notes.', 'agent:system:task-executor', 'long-task', 'system', 'actor:system')",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("rows");
        let task_id = TaskId::new("task:files").expect("task id");

        let collision = store
            .with_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let candidate = allocate_task_directory_tx(
                    &transaction,
                    &WorkspaceId::new("workspace:personal").expect("workspace id"),
                    Some(&ProjectId::new("project:files").expect("project id")),
                    "Long Task",
                )?;
                transaction.rollback()?;
                Ok(candidate)
            })
            .await
            .expect("allocate colliding directory");
        assert_eq!(collision, "long-task-2");

        store
            .ensure_task_document(&task_id)
            .await
            .expect("Task document");
        assert_eq!(
            store.read_task_file(&task_id, TASK_DOCUMENT).await.unwrap(),
            "# Long Task\n\nKeep durable notes.\n"
        );
        assert_eq!(
            store
                .read_task_file(&task_id, "../shared.md")
                .await
                .unwrap(),
            "shared"
        );
        assert!(matches!(
            store
                .write_task_file(&task_id, "../shared.md", "changed")
                .await,
            Err(TaskFileError::UnsafePath)
        ));
        store
            .write_task_file(&task_id, "notes/result.md", "first")
            .await
            .expect("first write");
        store
            .write_task_file(&task_id, "notes/result.md", "second")
            .await
            .expect("replacement");
        assert_eq!(
            store
                .read_task_file(&task_id, "notes/result.md")
                .await
                .unwrap(),
            "second"
        );
        assert!(matches!(
            store.delete_task_file(&task_id, TASK_DOCUMENT).await,
            Err(TaskFileError::RequiredDocument)
        ));

        store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE tasks SET title = 'Renamed Task' WHERE task_id = 'task:files'",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("rename Task");
        assert_eq!(
            store.task_working_directory(&task_id).await.unwrap(),
            project.join("long-task")
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = home.path().join("outside.md");
            std::fs::write(&outside, "outside").expect("outside");
            symlink(&outside, project.join("long-task/link.md")).expect("link");
            assert!(matches!(
                store.read_task_file(&task_id, "link.md").await,
                Err(TaskFileError::SymbolicLink)
            ));
        }
    }

    #[tokio::test]
    async fn standalone_task_lists_its_root_directory() {
        let home = tempfile::tempdir().expect("home");
        let store = NoemaStore::open(&StoreConfig::new(home.path().join("db/noema.sqlite3")))
            .await
            .expect("store");
        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, task_directory, source_kind, created_by_actor_id) VALUES ('task:standalone-files', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Standalone', '', 'agent:system:task-executor', 'standalone', 'system', 'actor:system')",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("Task");
        let task_id = TaskId::new("task:standalone-files").expect("Task id");
        store.ensure_task_document(&task_id).await.expect("TASK.md");

        let entries = store.list_task_files(&task_id, ".").await.expect("list");

        assert!(entries.iter().any(|entry| entry.path == TASK_DOCUMENT));
    }
}
