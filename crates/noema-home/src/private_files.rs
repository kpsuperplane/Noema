//! Private atomic file replacement for Noema-owned secrets.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(any(windows, test))]
use std::ffi::OsStr;
#[cfg(windows)]
use std::{
    env,
    process::{Command, Stdio},
};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);
const TEMP_FILE_ATTEMPTS: usize = 32;

/// Atomically replace one private file from a temporary file in the same directory.
///
/// # Errors
///
/// Returns an I/O error when directory preparation, writing, replacement, or sync fails.
pub fn atomic_write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "private file path must have a parent directory",
        )
    })?;
    ensure_private_dir(parent)?;

    let (temporary_path, mut temporary_file) = create_temporary_file(path)?;
    let write_result = (|| {
        temporary_file.write_all(bytes)?;
        temporary_file.sync_all()?;
        set_private_file_permissions(&temporary_path)?;
        drop(temporary_file);
        fs::rename(&temporary_path, path)?;
        set_private_file_permissions(path)?;
        sync_parent(path)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

/// Create a Noema-owned private directory and restrict its permissions.
///
/// # Errors
///
/// Returns an I/O error when directory creation or permission changes fail.
pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    create_private_dir_all(path)
}

/// Restrict an existing regular file to the current operating-system account.
///
/// # Errors
///
/// Returns an I/O error when the path is not a regular file or protection fails.
pub fn ensure_private_file(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private file path must be a regular file",
        ));
    }
    set_private_file_permissions(path)
}

fn create_temporary_file(path: &Path) -> io::Result<(PathBuf, fs::File)> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "private file path must have a parent directory",
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "private file path must have a file name",
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
        "could not allocate private temporary file",
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

#[cfg(windows)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    set_private_windows_acl(path, false)
}

#[cfg(not(any(unix, windows)))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(any(unix, windows))]
fn missing_private_directories(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut missing = Vec::new();
    let mut current = Some(path);
    while let Some(candidate) = current {
        match fs::metadata(candidate) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "private directory path is not a directory",
                ));
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                missing.push(candidate.to_path_buf());
                current = candidate.parent();
            }
            Err(source) => return Err(source),
        }
    }
    Ok(missing)
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let missing = missing_private_directories(path)?;
    fs::create_dir_all(path)?;
    for directory in missing.iter().rev() {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let missing = missing_private_directories(path)?;
    fs::create_dir_all(path)?;
    for directory in missing.iter().rev() {
        set_private_windows_acl(directory, true)?;
    }
    set_private_windows_acl(path, true)
}

#[cfg(not(any(unix, windows)))]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(windows)]
fn set_private_windows_acl(path: &Path, directory: bool) -> io::Result<()> {
    let system_root = env::var_os("SystemRoot")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other("SystemRoot is unavailable"))?;
    let system_root = PathBuf::from(system_root);
    if !system_root.is_absolute() {
        return Err(io::Error::other("SystemRoot must be an absolute path"));
    }
    let account = current_windows_account()?;
    let status = Command::new(system_root.join("System32/icacls.exe"))
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(windows_acl_grant(&account, directory))
        .arg(windows_acl_grant(OsStr::new("*S-1-5-18"), directory))
        .arg("/q")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| io::Error::other(format!("icacls failed with status {status}")))
}

#[cfg(windows)]
fn current_windows_account() -> io::Result<OsString> {
    let mut account = env::var_os("USERDOMAIN")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other("USERDOMAIN is unavailable"))?;
    account.push("\\");
    account.push(
        env::var_os("USERNAME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| io::Error::other("USERNAME is unavailable"))?,
    );
    Ok(account)
}

#[cfg(any(windows, test))]
fn windows_acl_grant(account: &OsStr, directory: bool) -> OsString {
    let mut grant = account.to_os_string();
    grant.push(if directory { ":(OI)(CI)F" } else { ":F" });
    grant
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
    fn atomic_replacement_is_private_and_leaves_no_temporary_file() {
        let root = tempfile::tempdir().expect("root");
        let directory = root.path().join("private");
        let path = directory.join("secret");
        atomic_write_private(&path, b"first").expect("first write");
        atomic_write_private(&path, b"second").expect("second write");
        assert_eq!(fs::read(&path).expect("read"), b"second");
        assert_eq!(fs::read_dir(&directory).expect("entries").count(), 1);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(directory)
                    .expect("directory")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&path).expect("file").permissions().mode() & 0o777,
                0o600
            );
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("loosen file");
            ensure_private_file(&path).expect("restore private file");
            assert_eq!(
                fs::metadata(&path).expect("file").permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn windows_acl_grants_distinguish_files_and_directories() {
        let account = OsStr::new("DOMAIN\\user");

        assert_eq!(windows_acl_grant(account, false), "DOMAIN\\user:F");
        assert_eq!(windows_acl_grant(account, true), "DOMAIN\\user:(OI)(CI)F");
    }
}
