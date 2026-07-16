//! Bounded cleanup for governed operation-private staging entries.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use cap_std::fs::{Dir, Metadata};

use crate::{ArtifactOperationError, safe_artifact_filename};

use super::{
    fs::{filesystem_error, open_verified_cap_dir, relative_path},
    storage::operation_id_is_valid,
};

const STAGING_DIR: &str = ".artifact-staging";
const MAX_STALE_STAGING_INSPECTIONS: usize = 128;
const MAX_STALE_STAGING_REMOVALS: usize = 32;

pub(super) fn cleanup_stale_staging(
    root_dir: &Dir,
    root: &Path,
    active_operations: &Arc<Mutex<std::collections::HashSet<String>>>,
    stale_age: Duration,
) -> Result<(), ArtifactOperationError> {
    let staging_root = root.join(STAGING_DIR);
    let relative_staging_root = relative_path(root, &staging_root)?;
    let metadata = match root_dir.symlink_metadata(&relative_staging_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(filesystem_error(
                "cleanup_staging",
                &staging_root,
                "staging directory could not be inspected",
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(filesystem_error(
            "cleanup_staging",
            &staging_root,
            "staging directory is not a real directory",
        ));
    }
    let staging_dir = open_verified_cap_dir(root_dir, root, &staging_root).map_err(|_| {
        filesystem_error(
            "cleanup_staging",
            &staging_root,
            "staging directory could not be verified",
        )
    })?;
    let mut removed = 0;
    let entries = staging_dir.entries().map_err(|_| {
        filesystem_error(
            "cleanup_staging",
            &staging_root,
            "staging directory could not be listed",
        )
    })?;
    for (inspected, entry) in entries.enumerate() {
        if inspected >= MAX_STALE_STAGING_INSPECTIONS || removed >= MAX_STALE_STAGING_REMOVALS {
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let Some(operation_id) = name.to_str() else {
            continue;
        };
        if !operation_id_is_valid(operation_id)
            || active_operations
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(operation_id)
        {
            continue;
        }
        let metadata = match staging_dir.symlink_metadata(Path::new(operation_id)) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !is_stale(&metadata, stale_age)
        {
            continue;
        }
        let operation_path = staging_root.join(operation_id);
        let operation_dir = match open_verified_cap_dir(root_dir, root, &operation_path) {
            Ok(directory) => directory,
            Err(_) => continue,
        };
        let mut children = match operation_dir.entries() {
            Ok(children) => children,
            Err(_) => continue,
        };
        let first = children.next();
        let second = children.next();
        match (first, second) {
            (None, None) => {}
            (Some(Ok(child)), None) => {
                let child_name: OsString = child.file_name();
                let Some(child_name) = child_name.to_str() else {
                    continue;
                };
                if safe_artifact_filename(child_name).is_err() {
                    continue;
                }
                let child_metadata = match operation_dir.symlink_metadata(Path::new(child_name)) {
                    Ok(metadata) => metadata,
                    Err(_) => continue,
                };
                if child_metadata.file_type().is_symlink() || !child_metadata.is_file() {
                    continue;
                }
                if operation_dir.remove_file(Path::new(child_name)).is_err() {
                    continue;
                }
            }
            _ => continue,
        }
        if staging_dir.remove_dir(Path::new(operation_id)).is_ok() {
            removed += 1;
        }
    }
    Ok(())
}

pub(super) struct CreatedDirectoryGuard {
    parent: Dir,
    name: PathBuf,
    armed: bool,
}

impl CreatedDirectoryGuard {
    pub(super) fn new(parent: Dir, name: impl Into<PathBuf>) -> Self {
        Self {
            parent,
            name: name.into(),
            armed: true,
        }
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CreatedDirectoryGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.parent.remove_dir(&self.name);
        }
    }
}

fn is_stale(metadata: &Metadata, stale_age: Duration) -> bool {
    metadata.modified().ok().is_some_and(|modified| {
        SystemTime::now()
            .duration_since(modified.into_std())
            .ok()
            .is_some_and(|age| age >= stale_age)
    })
}
