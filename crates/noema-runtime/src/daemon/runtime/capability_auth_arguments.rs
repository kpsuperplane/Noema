//! Protected transient arguments for durable capability-authentication pauses.

use std::{
    collections::HashSet,
    io::{self, Read, Write},
    path::Path,
};

#[cfg(unix)]
use cap_std::fs::{MetadataExt as CapMetadataExt, OpenOptionsExt, PermissionsExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata, OpenOptions, Permissions},
};
use noema_home::NoemaPaths;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::Value;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as StdMetadataExt;

const RUN_DIR: &str = "run";
const ARGUMENT_DIR: &str = "capability-auth";
const MAX_ARGUMENT_BYTES: usize = 1_048_576;

/// Opaque durable reference and digest for one exact argument payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProtectedCapabilityArguments {
    pub(super) reference: String,
    pub(super) sha256: String,
}

/// Private filesystem authority for transient resumable arguments.
#[derive(Clone)]
pub(super) struct CapabilityAuthArgumentStore {
    paths: NoemaPaths,
}

impl std::fmt::Debug for CapabilityAuthArgumentStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapabilityAuthArgumentStore")
            .finish_non_exhaustive()
    }
}

impl CapabilityAuthArgumentStore {
    pub(super) const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    pub(super) fn persist(
        &self,
        arguments: &Value,
    ) -> Result<ProtectedCapabilityArguments, CapabilityAuthArgumentStoreError> {
        let bytes = serde_json::to_vec(arguments).map_err(|_| error())?;
        if bytes.len() > MAX_ARGUMENT_BYTES {
            return Err(error());
        }
        let dir = self.open_dir(true)?.ok_or_else(error)?;
        for _ in 0..8 {
            let reference = random_reference()?;
            let temporary = format!("{reference}.tmp");
            let target = format!("{reference}.json");
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = match dir.open_with(&temporary, &options) {
                Ok(file) => file,
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(error()),
            };
            let result = (|| {
                file.write_all(&bytes)?;
                file.sync_all()?;
                drop(file);
                dir.rename(&temporary, &dir, &target)?;
                Ok::<_, std::io::Error>(())
            })();
            if result.is_err() {
                let _ = dir.remove_file(&temporary);
                return Err(error());
            }
            sync_dir(&self.paths, &dir)?;
            return Ok(ProtectedCapabilityArguments {
                reference,
                sha256: sha256_hex(&bytes),
            });
        }
        Err(error())
    }

    pub(super) fn load(
        &self,
        reference: &str,
        expected_sha256: &str,
    ) -> Result<Value, CapabilityAuthArgumentStoreError> {
        validate_reference(reference)?;
        let dir = self.open_dir(false)?.ok_or_else(error)?;
        let filename = format!("{reference}.json");
        let metadata = verified_file_metadata(&dir, Path::new(&filename))?;
        if metadata.len() > MAX_ARGUMENT_BYTES as u64 {
            return Err(error());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW);
        let file = dir.open_with(&filename, &options).map_err(|_| error())?;
        let opened = file.metadata().map_err(|_| error())?;
        let after = verified_file_metadata(&dir, Path::new(&filename))?;
        if !same_cap_metadata(&metadata, &opened) || !same_cap_metadata(&after, &opened) {
            return Err(error());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take((MAX_ARGUMENT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| error())?;
        if bytes.len() > MAX_ARGUMENT_BYTES || sha256_hex(&bytes) != expected_sha256 {
            return Err(error());
        }
        serde_json::from_slice(&bytes).map_err(|_| error())
    }

    pub(super) fn remove(&self, reference: &str) -> Result<(), CapabilityAuthArgumentStoreError> {
        validate_reference(reference)?;
        let Some(dir) = self.open_dir(false)? else {
            return Ok(());
        };
        match dir.remove_file(format!("{reference}.json")) {
            Ok(()) => sync_dir(&self.paths, &dir),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(error()),
        }
    }

    pub(super) fn remove_unreferenced(
        &self,
        retained: &HashSet<String>,
    ) -> Result<(), CapabilityAuthArgumentStoreError> {
        let Some(dir) = self.open_dir(false)? else {
            return Ok(());
        };
        for entry in dir.entries().map_err(|_| error())? {
            let entry = entry.map_err(|_| error())?;
            let name = entry.file_name().into_string().map_err(|_| error())?;
            let remove = if let Some(reference) = name.strip_suffix(".json") {
                validate_reference(reference)?;
                !retained.contains(reference)
            } else {
                name.ends_with(".tmp")
            };
            if remove {
                dir.remove_file(&name).map_err(|_| error())?;
            }
        }
        sync_dir(&self.paths, &dir)
    }

    fn open_dir(&self, create: bool) -> Result<Option<Dir>, CapabilityAuthArgumentStoreError> {
        let before = std::fs::symlink_metadata(self.paths.root()).map_err(|_| error())?;
        validate_std_dir(&before)?;
        let root =
            Dir::open_ambient_dir(self.paths.root(), ambient_authority()).map_err(|_| error())?;
        let opened = root.dir_metadata().map_err(|_| error())?;
        let after = std::fs::symlink_metadata(self.paths.root()).map_err(|_| error())?;
        validate_std_dir(&after)?;
        if !same_std_cap_metadata(&before, &opened) || !same_std_cap_metadata(&after, &opened) {
            return Err(error());
        }
        let Some(run) = open_child_dir(&root, Path::new(RUN_DIR), create)? else {
            return Ok(None);
        };
        open_child_dir(&run, Path::new(ARGUMENT_DIR), create)
    }
}

fn open_child_dir(
    parent: &Dir,
    name: &Path,
    create: bool,
) -> Result<Option<Dir>, CapabilityAuthArgumentStoreError> {
    let before = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            match parent.create_dir(name) {
                Ok(()) => {}
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(error()),
            }
            parent.symlink_metadata(name).map_err(|_| error())?
        }
        Err(_) => return Err(error()),
    };
    validate_cap_dir(&before)?;
    let dir = parent.open_dir(name).map_err(|_| error())?;
    let opened = dir.dir_metadata().map_err(|_| error())?;
    let after = parent.symlink_metadata(name).map_err(|_| error())?;
    validate_cap_dir(&after)?;
    if !same_cap_metadata(&before, &opened) || !same_cap_metadata(&after, &opened) {
        return Err(error());
    }
    #[cfg(unix)]
    dir.set_permissions(Path::new("."), Permissions::from_mode(0o700))
        .map_err(|_| error())?;
    Ok(Some(dir))
}

fn verified_file_metadata(
    dir: &Dir,
    filename: &Path,
) -> Result<Metadata, CapabilityAuthArgumentStoreError> {
    let metadata = dir.symlink_metadata(filename).map_err(|_| error())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(error());
    }
    Ok(metadata)
}

fn validate_std_dir(metadata: &std::fs::Metadata) -> Result<(), CapabilityAuthArgumentStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(error());
    }
    Ok(())
}

fn validate_cap_dir(metadata: &Metadata) -> Result<(), CapabilityAuthArgumentStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(error());
    }
    Ok(())
}

fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    return left.dev() == right.dev() && left.ino() == right.ino();
    #[cfg(not(unix))]
    return left.len() == right.len() && left.modified().ok() == right.modified().ok();
}

fn same_std_cap_metadata(left: &std::fs::Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    return left.dev() == right.dev() && left.ino() == right.ino();
    #[cfg(not(unix))]
    return left.len() == right.len()
        && left.modified().ok().map(Into::into) == right.modified().ok();
}

fn sync_dir(paths: &NoemaPaths, dir: &Dir) -> Result<(), CapabilityAuthArgumentStoreError> {
    let before =
        std::fs::symlink_metadata(paths.capability_auth_arguments_dir()).map_err(|_| error())?;
    validate_std_dir(&before)?;
    let opened = dir.dir_metadata().map_err(|_| error())?;
    if !same_std_cap_metadata(&before, &opened) {
        return Err(error());
    }
    std::fs::File::open(paths.capability_auth_arguments_dir())
        .and_then(|directory| directory.sync_all())
        .map_err(|_| error())
}

/// Redacted protected-argument storage failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("protected capability arguments are unavailable")]
pub(super) struct CapabilityAuthArgumentStoreError;

fn random_reference() -> Result<String, CapabilityAuthArgumentStoreError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| error())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_reference(reference: &str) -> Result<(), CapabilityAuthArgumentStoreError> {
    if reference.len() == 32
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(error())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

const fn error() -> CapabilityAuthArgumentStoreError {
    CapabilityAuthArgumentStoreError
}

#[cfg(test)]
#[path = "capability_auth_arguments/tests.rs"]
mod tests;
