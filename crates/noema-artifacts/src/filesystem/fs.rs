//! Capability-rooted filesystem primitives.

use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
use cap_std::fs::{MetadataExt as CapMetadataExt, OpenOptionsExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata, OpenOptions},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as StdMetadataExt;

use crate::{ArtifactDomainError, ArtifactOperationError};

pub(super) fn open_root(root: &Path) -> Result<Dir, ArtifactOperationError> {
    let before = std::fs::symlink_metadata(root)
        .map_err(|_| filesystem_error("open_root", root, "artifact root is unavailable"))?;
    validate_ambient_root_metadata(root, &before)?;

    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|_| filesystem_error("open_root", root, "artifact root could not be opened"))?;
    let opened = directory
        .dir_metadata()
        .map_err(|_| filesystem_error("open_root", root, "artifact root could not be verified"))?;
    let after = std::fs::symlink_metadata(root)
        .map_err(|_| filesystem_error("open_root", root, "artifact root changed while opening"))?;
    validate_ambient_root_metadata(root, &after)?;
    if !same_std_cap_metadata(&before, &opened) || !same_std_cap_metadata(&after, &opened) {
        return Err(filesystem_error(
            "open_root",
            root,
            "artifact root changed while opening",
        ));
    }
    Ok(directory)
}

pub(super) fn duplicate_dir(directory: &Dir, path: &Path) -> Result<Dir, ArtifactOperationError> {
    directory.try_clone().map_err(|_| {
        filesystem_error(
            "open_directory",
            path,
            "artifact directory handle could not be retained",
        )
    })
}

pub(super) fn ensure_verified_dir(
    root_dir: &Dir,
    root: &Path,
    directory: &Path,
) -> Result<Dir, ArtifactOperationError> {
    verify_root_identity(root_dir, root).map_err(|_| {
        filesystem_error(
            "create_directory",
            directory,
            "artifact root changed after initialization",
        )
    })?;
    reject_symlink_path_components(root, directory, true).map_err(|_| {
        filesystem_error(
            "create_directory",
            directory,
            "artifact directory path is unsafe",
        )
    })?;
    let relative = relative_path(root, directory)?;
    root_dir.create_dir_all(&relative).map_err(|_| {
        filesystem_error(
            "create_directory",
            directory,
            "artifact directory could not be created",
        )
    })?;
    open_verified_cap_dir(root_dir, root, directory).map_err(|_| {
        filesystem_error(
            "create_directory",
            directory,
            "artifact directory could not be verified",
        )
    })
}

pub(super) fn open_verified_cap_dir(
    root_dir: &Dir,
    root: &Path,
    directory: &Path,
) -> Result<Dir, std::io::Error> {
    verify_root_identity(root_dir, root)?;
    reject_symlink_path_components(root, directory, false)?;
    let relative = relative_path(root, directory).map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
    })?;
    let dir = root_dir.open_dir(&relative)?;
    verify_root_identity(root_dir, root)?;
    reject_symlink_path_components(root, directory, false)?;
    let path_metadata = root_dir.symlink_metadata(&relative)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact directory path is not a real directory",
        ));
    }
    let dir_metadata = dir.dir_metadata()?;
    if !same_cap_metadata(&dir_metadata, &path_metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact directory changed while opening",
        ));
    }
    Ok(dir)
}

pub(super) fn read_verified_file(
    directory: &Dir,
    filename: &Path,
    absolute_path: &Path,
) -> Result<(Vec<u8>, Metadata), ArtifactOperationError> {
    let before = directory
        .symlink_metadata(filename)
        .map_err(|_| filesystem_error("read", absolute_path, "artifact file is unavailable"))?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(filesystem_error(
            "read",
            absolute_path,
            "artifact file is not a regular file",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = directory.open_with(filename, &options).map_err(|_| {
        filesystem_error("read", absolute_path, "artifact file could not be opened")
    })?;
    let opened = file.metadata().map_err(|_| {
        filesystem_error(
            "read",
            absolute_path,
            "artifact file metadata is unavailable",
        )
    })?;
    let after = directory.symlink_metadata(filename).map_err(|_| {
        filesystem_error("read", absolute_path, "artifact file changed while opening")
    })?;
    if !same_cap_metadata(&opened, &before) || !same_cap_metadata(&opened, &after) {
        return Err(ArtifactOperationError::Integrity {
            path: absolute_path.to_path_buf(),
        });
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| filesystem_error("read", absolute_path, "artifact file could not be read"))?;
    Ok((bytes, opened))
}

pub(super) fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, ArtifactDomainError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ArtifactDomainError::UnsafeFilename {
            value: path.display().to_string(),
        })?;
    if !relative_path_components_are_safe(relative) {
        return Err(ArtifactDomainError::UnsafeFilename {
            value: relative.display().to_string(),
        });
    }
    Ok(relative.to_path_buf())
}

pub(super) fn relative_path_components_are_safe(path: &Path) -> bool {
    let mut saw_component = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => saw_component = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    saw_component
}

pub(super) fn cleanup_empty_dirs<'a>(
    root_dir: &Dir,
    directories: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<(), ArtifactOperationError> {
    for directory in directories {
        match root_dir.remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(_) => {
                return Err(filesystem_error(
                    "cleanup",
                    directory,
                    "private artifact directory could not be removed",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_no_follow(options: &mut OpenOptions) {
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
pub(super) fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
pub(super) fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
pub(super) fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

pub(super) fn filesystem_error(
    operation: &'static str,
    path: &Path,
    message: &'static str,
) -> ArtifactOperationError {
    ArtifactOperationError::Filesystem {
        operation,
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}

fn verify_root_identity(root_dir: &Dir, root: &Path) -> Result<(), std::io::Error> {
    let ambient = std::fs::symlink_metadata(root)?;
    if ambient.file_type().is_symlink() || !ambient.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact root is no longer a real directory",
        ));
    }
    let retained = root_dir.dir_metadata()?;
    if !same_std_cap_metadata(&ambient, &retained) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact root changed after initialization",
        ));
    }
    Ok(())
}

fn validate_ambient_root_metadata(
    root: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), ArtifactOperationError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(filesystem_error(
            "open_root",
            root,
            "artifact root must be a real directory",
        ));
    }
    Ok(())
}

fn reject_symlink_path_components(
    root: &Path,
    path: &Path,
    allow_missing: bool,
) -> Result<(), std::io::Error> {
    let relative = path.strip_prefix(root).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact path is outside the configured root",
        )
    })?;
    let mut current = root.to_path_buf();
    reject_symlink_component(&current)?;
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "artifact path contains unsafe components",
            ));
        };
        current.push(segment);
        match reject_symlink_component(&current) {
            Ok(()) => {}
            Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn reject_symlink_component(path: &Path) -> Result<(), std::io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact path contains a symbolic link",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn same_std_cap_metadata(left: &std::fs::Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_std_cap_metadata(left: &std::fs::Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok().map(Into::into) == right.modified().ok()
}
