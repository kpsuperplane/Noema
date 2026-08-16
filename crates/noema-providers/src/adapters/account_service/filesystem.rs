//! Hardened filesystem primitives for provider credentials.

use std::{fmt, fs, io, path::Path};

pub(crate) use noema_home::atomic_write_private;

/// Byte-exact snapshot of one credential file.
#[derive(Clone, PartialEq, Eq)]
pub enum FileSnapshot {
    /// The file did not exist.
    Missing,
    /// The file existed with these exact bytes.
    Present(Vec<u8>),
}

impl fmt::Debug for FileSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("FileSnapshot::Missing"),
            Self::Present(bytes) => formatter
                .debug_tuple("FileSnapshot::Present")
                .field(&format_args!("[REDACTED; {} bytes]", bytes.len()))
                .finish(),
        }
    }
}

/// Capture one file without interpreting its contents.
pub(crate) fn snapshot_file(path: &Path) -> io::Result<FileSnapshot> {
    match fs::read(path) {
        Ok(bytes) => Ok(FileSnapshot::Present(bytes)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(FileSnapshot::Missing),
        Err(source) => Err(source),
    }
}

/// Restore one file to a byte-exact prior snapshot.
pub(crate) fn restore_file(path: &Path, snapshot: &FileSnapshot) -> io::Result<()> {
    match snapshot {
        FileSnapshot::Missing => remove_file_if_exists(path),
        FileSnapshot::Present(bytes) => atomic_write_private(path, bytes),
    }
}

/// Remove one file while treating an absent file as success.
pub(crate) fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(())
        }
        Err(source) => Err(source),
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    match fs::File::open(parent).and_then(|directory| directory.sync_all()) {
        Ok(()) => Ok(()),
        #[cfg(windows)]
        Err(source) if source.kind() == io::ErrorKind::PermissionDenied => Ok(()),
        Err(source) => Err(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_and_restore_preserve_exact_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("provider/account/credential.json");
        let original = b"{not valid json}\0\xff";
        atomic_write_private(&path, original).expect("write original");
        let snapshot = snapshot_file(&path).expect("snapshot");
        atomic_write_private(&path, b"replacement").expect("replace");

        restore_file(&path, &snapshot).expect("restore");

        assert_eq!(fs::read(&path).expect("read restored"), original);
    }

    #[test]
    fn missing_snapshot_removes_replacement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("provider/account/credential.json");
        let snapshot = snapshot_file(&path).expect("missing snapshot");
        atomic_write_private(&path, b"new credential").expect("write");

        restore_file(&path, &snapshot).expect("restore missing");

        assert!(!path.exists());
    }

    #[test]
    fn file_snapshot_debug_redacts_present_bytes() {
        let snapshot = FileSnapshot::Present(b"credential-secret".to_vec());
        let debug = format!("{snapshot:?}");

        assert!(!debug.contains("credential-secret"));
        assert!(debug.contains("[REDACTED"));
    }
}
