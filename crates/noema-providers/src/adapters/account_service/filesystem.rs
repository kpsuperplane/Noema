//! Hardened filesystem primitives for provider credentials.

use std::{
    ffi::OsString,
    fmt, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);
const TEMP_FILE_ATTEMPTS: usize = 32;

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

/// Atomically replace one credential file from a temporary file in the same
/// directory.
pub(crate) fn atomic_write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential path must have a parent directory",
        )
    })?;
    ensure_private_dir(parent)?;

    let (temporary_path, mut temporary_file) = create_temporary_file(path)?;
    let write_result = (|| {
        temporary_file.write_all(bytes)?;
        temporary_file.sync_all()?;
        set_private_file_permissions(&temporary_path)?;
        drop(temporary_file);
        replace_file(&temporary_path, path)?;
        set_private_file_permissions(path)?;
        sync_parent(path)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

/// Create an account credential directory with private permissions.
pub(crate) fn ensure_private_dir(path: &Path) -> io::Result<()> {
    create_private_dir_all(path)
}

fn create_temporary_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential path must have a parent directory",
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential path must have a file name",
        )
    })?;

    for _ in 0..TEMP_FILE_ATTEMPTS {
        let sequence = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".tmp-{}-{sequence}", std::process::id()));
        let temporary_path = parent.join(temporary_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        configure_private_file_creation(&mut options);
        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(source),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate credential temporary file",
    ))
}

#[cfg(unix)]
fn configure_private_file_creation(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

#[cfg(not(unix))]
fn configure_private_file_creation(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut missing = Vec::new();
    let mut current = Some(path);
    while let Some(candidate) = current {
        match fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "credential directory path is not a directory",
                ));
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                missing.push(candidate.to_path_buf());
                current = candidate.parent();
            }
            Err(source) => return Err(source),
        }
    }

    fs::create_dir_all(path)?;
    for directory in missing.iter().rev() {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
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

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are live, immutable, NUL-terminated UTF-16 paths
    // for the duration of the call, and the flags contain no pointer-bearing
    // options.
    let replaced = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
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
    fn atomic_replacement_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let account_home = dir.path().join("provider/account");
        let path = account_home.join("credential.json");
        atomic_write_private(&path, b"first").expect("first write");
        atomic_write_private(&path, b"second").expect("second write");

        assert_eq!(fs::read(&path).expect("read"), b"second");
        let entries = fs::read_dir(account_home)
            .expect("read account home")
            .collect::<Result<Vec<_>, _>>()
            .expect("entries");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn file_snapshot_debug_redacts_present_bytes() {
        let snapshot = FileSnapshot::Present(b"credential-secret".to_vec());
        let debug = format!("{snapshot:?}");

        assert!(!debug.contains("credential-secret"));
        assert!(debug.contains("[REDACTED"));
    }

    #[cfg(unix)]
    #[test]
    fn credentials_use_private_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let account_home = dir.path().join("provider/account");
        let path = account_home.join("credential.json");
        atomic_write_private(&path, b"secret").expect("write");

        let directory_mode = fs::metadata(&account_home)
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = fs::metadata(path)
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }
}
