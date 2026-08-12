//! Atomic filesystem protocol for reusable OAuth authority objects.

use crate::{
    digest::canonical_json_bytes,
    oauth_authority_store::OauthAuthorityStoreError,
    private_fs::{create_private_dir, random_hex, sync_directory, write_new_file},
};
use std::{fs, path::Path};

pub(super) const STAGING_DIR: &str = ".staging";
const REPLACEMENT_PREFIX: &str = ".descriptor-replace-";
pub(super) const MAX_DESCRIPTOR_BYTES: u64 = 128 * 1024;
pub(super) const MAX_SECRET_BYTES: u64 = 128 * 1024;

pub(super) fn canonical_bytes<T: serde::Serialize>(
    value: &T,
) -> Result<Vec<u8>, OauthAuthorityStoreError> {
    let bytes = canonical_json_bytes(&serde_json::to_value(value)?)?;
    if bytes.len() as u64 > MAX_DESCRIPTOR_BYTES {
        return Err(OauthAuthorityStoreError::Integrity("object_oversized"));
    }
    Ok(bytes)
}

pub(super) fn install_directory(
    root: &Path,
    target: &Path,
    files: &[(&str, &[u8])],
    directories: &[&str],
    nested: &[(&str, String, &[u8])],
) -> Result<(), OauthAuthorityStoreError> {
    if target.exists() {
        return Ok(());
    }
    let staging_root = root.join(STAGING_DIR);
    create_private_dir(&staging_root)?;
    let staging = staging_root.join(random_hex(12)?);
    create_private_dir(&staging)?;
    let result = (|| {
        for (name, bytes) in files {
            write_new_file(&staging.join(name), bytes)?;
        }
        for directory in directories {
            create_private_dir(&staging.join(directory))?;
        }
        for (directory, name, bytes) in nested {
            if bytes.len() as u64 > MAX_SECRET_BYTES {
                return Err(OauthAuthorityStoreError::Integrity("secret_oversized"));
            }
            write_new_file(&staging.join(directory).join(name), bytes)?;
            sync_directory(&staging.join(directory))?;
        }
        sync_directory(&staging)?;
        fs::rename(&staging, target)?;
        sync_directory(root)?;
        Ok(())
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

pub(super) fn recover_staging(root: &Path) -> Result<(), OauthAuthorityStoreError> {
    let staging = root.join(STAGING_DIR);
    let metadata = match fs::symlink_metadata(&staging) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(OauthAuthorityStoreError::Integrity("staging_directory"));
    }
    let entries = fs::read_dir(&staging)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() > 64 {
        return Err(OauthAuthorityStoreError::Integrity("staging_oversized"));
    }
    for entry in entries {
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(entry.path())?;
        } else if metadata.is_file() || metadata.file_type().is_symlink() {
            fs::remove_file(entry.path())?;
        } else {
            return Err(OauthAuthorityStoreError::Integrity("staging_entry"));
        }
    }
    sync_directory(&staging)?;
    sync_directory(root)?;
    Ok(())
}

pub(super) fn publish_generation<D: serde::Serialize, S: serde::Serialize>(
    target: &Path,
    descriptor_name: &str,
    generations_name: &str,
    old_generation: &str,
    replacement: &D,
    new_generation: &str,
    secret: &S,
) -> Result<(), OauthAuthorityStoreError> {
    let descriptor_bytes = canonical_bytes(replacement)?;
    let secret_bytes = canonical_bytes(secret)?;
    if secret_bytes.len() as u64 > MAX_SECRET_BYTES {
        return Err(OauthAuthorityStoreError::Integrity("secret_oversized"));
    }
    let generations = target.join(generations_name);
    let new_path = generations.join(credential_file(new_generation));
    let old_path = generations.join(credential_file(old_generation));
    let temporary = generations.join(format!("{REPLACEMENT_PREFIX}{}", random_hex(16)?));
    write_new_file(&new_path, &secret_bytes)?;
    let mut published = false;
    let result = (|| {
        write_new_file(&temporary, &descriptor_bytes)?;
        sync_directory(&generations)?;
        fs::rename(&temporary, target.join(descriptor_name))?;
        published = true;
        sync_directory(target)?;
        fs::remove_file(old_path)?;
        sync_directory(&generations)?;
        Ok(())
    })();
    if !published {
        let _ = fs::remove_file(temporary);
        let _ = fs::remove_file(new_path);
        let _ = sync_directory(&generations);
    }
    result
}

pub(super) fn credential_file(generation: &str) -> String {
    format!("{generation}.json")
}
