//! Shared private filesystem primitives for canonical adapter objects.

use ring::rand::{SecureRandom, SystemRandom};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrivateFsError {
    #[error("private adapter filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("private adapter filesystem invariant failed: {0}")]
    Integrity(&'static str),
}

#[doc = "Create or validate a private, non-symlink directory.\n\n# Errors\nReturns an I/O or integrity error when the path is unsafe or unavailable."]
pub fn create_private_dir(path: &Path) -> Result<(), PrivateFsError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(PrivateFsError::Integrity("object_directory"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[doc = "Require an existing private, non-symlink directory.\n\n# Errors\nReturns an I/O or integrity error when the directory is unsafe or unavailable."]
pub fn require_regular_directory(path: &Path) -> Result<(), PrivateFsError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || !has_private_permissions(&metadata)
    {
        return Err(PrivateFsError::Integrity("object_directory"));
    }
    Ok(())
}

pub(crate) fn require_directory_no_symlink(path: &Path) -> Result<(), PrivateFsError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PrivateFsError::Integrity("noema_home"));
    }
    Ok(())
}

pub(crate) fn require_exact_entries(path: &Path, expected: &[&str]) -> Result<(), PrivateFsError> {
    let mut names = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    let mut expected = expected.iter().map(OsString::from).collect::<Vec<_>>();
    expected.sort();
    if names != expected {
        return Err(PrivateFsError::Integrity("object_entries"));
    }
    Ok(())
}

#[doc = "Read a bounded private regular file without following its final symlink.\n\n# Errors\nReturns an I/O or integrity error when the file is unsafe, unavailable, or oversized."]
pub fn read_bounded_regular_file(path: &Path, max: u64) -> Result<Vec<u8>, PrivateFsError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > max
        || !has_private_permissions(&metadata)
    {
        return Err(PrivateFsError::Integrity("object_file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(PrivateFsError::Integrity("object_changed"));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(PrivateFsError::Integrity("object_oversized"));
    }
    Ok(bytes)
}

#[doc = "Create and synchronize a private regular file without replacing an entry.\n\n# Errors\nReturns an I/O or integrity error when creation or synchronization fails."]
pub fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), PrivateFsError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[doc = "Generate a cryptographically random lowercase hexadecimal value.\n\n# Errors\nReturns an integrity error when secure randomness is unavailable."]
pub fn random_hex(bytes: usize) -> Result<String, PrivateFsError> {
    let mut value = vec![0_u8; bytes];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| PrivateFsError::Integrity("randomness"))?;
    Ok(value.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[doc = "Synchronize directory metadata after an atomic filesystem mutation.\n\n# Errors\nReturns an I/O error when the directory cannot be opened or synchronized."]
pub fn sync_directory(path: &Path) -> Result<(), PrivateFsError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn has_private_permissions(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn has_private_permissions(_metadata: &fs::Metadata) -> bool {
    true
}
