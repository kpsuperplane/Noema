//! Capability-rooted filesystem primitives for MCP secret persistence.

use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

#[cfg(unix)]
use cap_std::fs::{MetadataExt as CapMetadataExt, OpenOptionsExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, File, Metadata, OpenOptions, Permissions},
};
use noema_home::sanitize_path_segment;
use ring::rand::{SecureRandom, SystemRandom};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as StdMetadataExt, PermissionsExt};

use super::{BACKUP_PREFIX, RESTORE_PREFIX, SECRET_FILE, STAGING_DIR};

const MCP_DIR: &str = "mcp";

pub(super) struct SecretRoot {
    path: PathBuf,
    dir: Dir,
}

impl SecretRoot {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let before = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir(path)?;
                std::fs::symlink_metadata(path)?
            }
            Err(error) => return Err(error),
        };
        validate_root_metadata(&before)?;
        let dir = Dir::open_ambient_dir(path, ambient_authority())?;
        let opened = dir.dir_metadata()?;
        let after = std::fs::symlink_metadata(path)?;
        validate_root_metadata(&after)?;
        if !same_std_cap_metadata(&before, &opened) || !same_std_cap_metadata(&after, &opened) {
            return Err(invalid("MCP secret root changed while opening"));
        }
        Ok(Self {
            path: path.to_path_buf(),
            dir,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn read_secret(&self, mcp_server_id: &str) -> io::Result<Option<Vec<u8>>> {
        let Some(server) = self.open_server(mcp_server_id, false)? else {
            return Ok(None);
        };
        read_verified_file(&server, Path::new(SECRET_FILE))
    }

    pub(super) fn write_stage(&self, filename: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let staging = self
            .open_staging(true)?
            .ok_or_else(|| invalid("MCP staging directory is unavailable"))?;
        write_new_private_file(&staging, Path::new(filename), bytes)?;
        Ok(self.path.join(MCP_DIR).join(STAGING_DIR).join(filename))
    }

    pub(super) fn commit_stage(
        &self,
        stage_path: &Path,
        mcp_server_id: &str,
        backup_filename: &str,
    ) -> io::Result<(PathBuf, Option<PathBuf>)> {
        self.verify()?;
        let stage_name = self.stage_filename(stage_path)?;
        let staging = self
            .open_staging(false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "MCP staging is missing"))?;
        require_regular_file(&staging, &stage_name)?;
        let server = self
            .open_server(mcp_server_id, true)?
            .ok_or_else(|| invalid("MCP server directory is unavailable"))?;

        let target_name = Path::new(SECRET_FILE);
        let target_exists = file_metadata(&server, target_name)?.is_some();
        let backup_name = Path::new(backup_filename);
        let backup = if target_exists {
            copy_verified_file(&server, target_name, &server, backup_name)?;
            Some(self.server_path(mcp_server_id).join(backup_filename))
        } else {
            None
        };

        if let Err(source) = replace_file(&staging, &stage_name, &server, target_name) {
            let target_missing = file_metadata(&server, target_name)
                .map(|metadata| metadata.is_none())
                .unwrap_or(false);
            if target_missing
                && backup.is_some()
                && let Err(recovery) = restore_backup(&server, backup_name, target_name)
            {
                return Err(combined_io_error(source, recovery));
            }
            return Err(source);
        }
        if let Err(source) = set_named_file_private(&server, target_name) {
            let recovery = if backup.is_some() {
                restore_backup(&server, backup_name, target_name)
            } else {
                remove_file_if_present(&server, target_name)
            };
            if let Err(recovery) = recovery {
                return Err(combined_io_error(source, recovery));
            }
            return Err(source);
        }
        self.verify()?;
        Ok((self.server_path(mcp_server_id).join(SECRET_FILE), backup))
    }

    pub(super) fn rollback(&self, target: &Path, backup: Option<&Path>) -> io::Result<()> {
        let (server_name, target_name) = self.server_file(target)?;
        if target_name != OsStr::new(SECRET_FILE) {
            return Err(invalid("invalid MCP secret target"));
        }
        let server = self
            .open_server_segment(&server_name, false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "MCP server is missing"))?;
        if let Some(backup) = backup {
            let (backup_server, backup_name) = self.server_file(backup)?;
            if backup_server != server_name
                || !backup_name
                    .to_str()
                    .is_some_and(|name| name.starts_with(BACKUP_PREFIX))
            {
                return Err(invalid("invalid MCP secret backup"));
            }
            restore_backup(&server, Path::new(&backup_name), Path::new(SECRET_FILE))?;
            remove_file_if_present(&server, Path::new(&backup_name))?;
        } else {
            remove_file_if_present(&server, Path::new(SECRET_FILE))?;
        }
        self.verify()
    }

    pub(super) fn finalize(&self, backup: &Path) -> io::Result<()> {
        let (server_name, backup_name) = self.server_file(backup)?;
        if !backup_name
            .to_str()
            .is_some_and(|name| name.starts_with(BACKUP_PREFIX))
        {
            return Err(invalid("invalid MCP secret backup"));
        }
        let server = self
            .open_server_segment(&server_name, false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "MCP server is missing"))?;
        remove_file_if_present(&server, Path::new(&backup_name))?;
        self.verify()
    }

    pub(super) fn discard(&self, stage_path: &Path) -> io::Result<()> {
        let stage_name = self.stage_filename(stage_path)?;
        let staging = self
            .open_staging(false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "MCP staging is missing"))?;
        remove_file_if_present(&staging, &stage_name)?;
        self.verify()
    }

    pub(super) fn remove_server(&self, mcp_server_id: &str) -> io::Result<()> {
        let Some(mcp) = self.open_mcp(false)? else {
            return Ok(());
        };
        let server_name = sanitize_path_segment(mcp_server_id);
        let Some(server) = open_child_dir(&mcp, Path::new(&server_name), false)? else {
            return Ok(());
        };
        server.remove_open_dir_all()?;
        self.verify()
    }

    pub(super) fn cleanup_abandoned_staging(&self) -> io::Result<()> {
        self.recover_abandoned_backups()?;
        if let Some(staging) = self.open_staging(false)? {
            staging.remove_open_dir_all()?;
        }
        self.open_staging(true)?
            .ok_or_else(|| invalid("MCP staging directory is unavailable"))?;
        self.verify()
    }

    fn recover_abandoned_backups(&self) -> io::Result<()> {
        let Some(mcp) = self.open_mcp(false)? else {
            return Ok(());
        };
        for entry in mcp.entries()? {
            let entry = entry?;
            let server_name = entry.file_name();
            if server_name == OsStr::new(STAGING_DIR) {
                continue;
            }
            let metadata = mcp.symlink_metadata(Path::new(&server_name))?;
            if metadata.file_type().is_symlink() {
                return Err(invalid("MCP server path contains a symbolic link"));
            }
            if !metadata.is_dir() {
                continue;
            }
            let server = open_child_dir(&mcp, Path::new(&server_name), false)?
                .ok_or_else(|| invalid("MCP server directory changed during cleanup"))?;
            recover_server_backups(&server)?;
        }
        self.verify()
    }

    fn open_mcp(&self, create: bool) -> io::Result<Option<Dir>> {
        self.verify()?;
        open_child_dir(&self.dir, Path::new(MCP_DIR), create)
    }

    fn open_staging(&self, create: bool) -> io::Result<Option<Dir>> {
        let Some(mcp) = self.open_mcp(create)? else {
            return Ok(None);
        };
        open_child_dir(&mcp, Path::new(STAGING_DIR), create)
    }

    fn open_server(&self, mcp_server_id: &str, create: bool) -> io::Result<Option<Dir>> {
        let server_name = sanitize_path_segment(mcp_server_id);
        self.open_server_segment(OsStr::new(&server_name), create)
    }

    fn open_server_segment(&self, server_name: &OsStr, create: bool) -> io::Result<Option<Dir>> {
        let Some(mcp) = self.open_mcp(create)? else {
            return Ok(None);
        };
        open_child_dir(&mcp, Path::new(server_name), create)
    }

    fn stage_filename(&self, path: &Path) -> io::Result<PathBuf> {
        let staging = self.path.join(MCP_DIR).join(STAGING_DIR);
        single_relative_component(
            path.strip_prefix(staging)
                .map_err(|_| invalid("staged MCP secret does not belong to the configured root"))?,
        )
    }

    fn server_file(&self, path: &Path) -> io::Result<(OsString, OsString)> {
        let relative = path
            .strip_prefix(self.path.join(MCP_DIR))
            .map_err(|_| invalid("MCP secret file does not belong to the configured root"))?;
        let mut components = relative.components();
        let Some(Component::Normal(server)) = components.next() else {
            return Err(invalid("invalid MCP server file path"));
        };
        let Some(Component::Normal(filename)) = components.next() else {
            return Err(invalid("invalid MCP server file path"));
        };
        if components.next().is_some() {
            return Err(invalid("invalid MCP server file path"));
        }
        Ok((server.to_os_string(), filename.to_os_string()))
    }

    fn server_path(&self, mcp_server_id: &str) -> PathBuf {
        self.path
            .join(MCP_DIR)
            .join(sanitize_path_segment(mcp_server_id))
    }

    fn verify(&self) -> io::Result<()> {
        let ambient = std::fs::symlink_metadata(&self.path)?;
        validate_root_metadata(&ambient)?;
        let retained = self.dir.dir_metadata()?;
        if !same_std_cap_metadata(&ambient, &retained) {
            return Err(invalid("MCP secret root changed after initialization"));
        }
        Ok(())
    }
}

fn open_child_dir(parent: &Dir, name: &Path, create: bool) -> io::Result<Option<Dir>> {
    validate_single_component(name)?;
    let before = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match parent.create_dir(name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            parent.symlink_metadata(name)?
        }
        Err(error) => return Err(error),
    };
    validate_directory_metadata(&before)?;
    let dir = parent.open_dir(name)?;
    let opened = dir.dir_metadata()?;
    let after = parent.symlink_metadata(name)?;
    validate_directory_metadata(&after)?;
    if !same_cap_metadata(&before, &opened) || !same_cap_metadata(&after, &opened) {
        return Err(invalid("MCP secret directory changed while opening"));
    }
    set_private_dir_permissions(&dir)?;
    Ok(Some(dir))
}

fn read_verified_file(directory: &Dir, filename: &Path) -> io::Result<Option<Vec<u8>>> {
    let Some(mut file) = open_verified_file(directory, filename)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn open_verified_file(directory: &Dir, filename: &Path) -> io::Result<Option<File>> {
    validate_single_component(filename)?;
    let Some(before) = file_metadata(directory, filename)? else {
        return Ok(None);
    };
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = directory.open_with(filename, &options)?;
    let opened = file.metadata()?;
    let after = directory.symlink_metadata(filename)?;
    validate_file_metadata(&after)?;
    if !same_cap_metadata(&before, &opened) || !same_cap_metadata(&after, &opened) {
        return Err(invalid("MCP secret file changed while opening"));
    }
    Ok(Some(file))
}

fn require_regular_file(directory: &Dir, filename: &Path) -> io::Result<()> {
    if file_metadata(directory, filename)?.is_some() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "MCP secret file is missing",
        ))
    }
}

fn file_metadata(directory: &Dir, filename: &Path) -> io::Result<Option<Metadata>> {
    validate_single_component(filename)?;
    match directory.symlink_metadata(filename) {
        Ok(metadata) => {
            validate_file_metadata(&metadata)?;
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_new_private_file(directory: &Dir, filename: &Path, bytes: &[u8]) -> io::Result<()> {
    validate_single_component(filename)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    set_no_follow(&mut options);
    set_create_mode(&mut options);
    let mut file = directory.open_with(filename, &options)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    set_private_file_permissions(&file)
}

fn copy_verified_file(
    source_dir: &Dir,
    source: &Path,
    target_dir: &Dir,
    target: &Path,
) -> io::Result<()> {
    let bytes = read_verified_file(source_dir, source)?.ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "MCP secret source file is missing")
    })?;
    write_new_private_file(target_dir, target, &bytes)
}

fn restore_backup(directory: &Dir, backup: &Path, target: &Path) -> io::Result<()> {
    let restore_name = format!("{RESTORE_PREFIX}{}", random_hex_io()?);
    let restore = Path::new(&restore_name);
    copy_verified_file(directory, backup, directory, restore)?;
    match replace_file(directory, restore, directory, target) {
        Ok(()) => set_named_file_private(directory, target),
        Err(error) => {
            let _ = remove_file_if_present(directory, restore);
            Err(error)
        }
    }
}

#[cfg(not(windows))]
fn replace_file(
    source_dir: &Dir,
    source: &Path,
    target_dir: &Dir,
    target: &Path,
) -> io::Result<()> {
    source_dir.rename(source, target_dir, target)
}

#[cfg(windows)]
fn replace_file(
    source_dir: &Dir,
    source: &Path,
    target_dir: &Dir,
    target: &Path,
) -> io::Result<()> {
    match source_dir.rename(source, target_dir, target) {
        Ok(()) => Ok(()),
        Err(error)
            if file_metadata(target_dir, target)?.is_some()
                && matches!(
                    error.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
                ) =>
        {
            target_dir.remove_file(target)?;
            source_dir.rename(source, target_dir, target)
        }
        Err(error) => Err(error),
    }
}

fn remove_file_if_present(directory: &Dir, filename: &Path) -> io::Result<()> {
    if file_metadata(directory, filename)?.is_none() {
        return Ok(());
    }
    match directory.remove_file(filename) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn recover_server_backups(server: &Dir) -> io::Result<()> {
    let mut backups = Vec::new();
    let mut restores = Vec::new();
    for entry in server.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(utf8_name) = name.to_str() else {
            continue;
        };
        if utf8_name.starts_with(BACKUP_PREFIX) || utf8_name.starts_with(RESTORE_PREFIX) {
            let metadata = server.symlink_metadata(Path::new(&name))?;
            validate_file_metadata(&metadata)?;
            if utf8_name.starts_with(BACKUP_PREFIX) {
                backups.push((
                    name,
                    metadata
                        .modified()
                        .ok()
                        .map(cap_std::time::SystemTime::into_std)
                        .unwrap_or(UNIX_EPOCH),
                ));
            } else {
                restores.push(name);
            }
        }
    }
    backups.sort_by_key(|(_, modified)| *modified);
    let target = Path::new(SECRET_FILE);
    let target_exists = file_metadata(server, target)?.is_some();
    if !target_exists && let Some((backup, _)) = backups.last() {
        restore_backup(server, Path::new(backup), target)?;
    }
    if file_metadata(server, target)?.is_some() {
        for (backup, _) in backups {
            remove_file_if_present(server, Path::new(&backup))?;
        }
    }
    for restore in restores {
        remove_file_if_present(server, Path::new(&restore))?;
    }
    Ok(())
}

fn set_named_file_private(directory: &Dir, filename: &Path) -> io::Result<()> {
    let file = open_verified_file(directory, filename)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "MCP secret file is missing"))?;
    set_private_file_permissions(&file)
}

fn validate_root_metadata(metadata: &std::fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid("MCP secret root must be a real directory"));
    }
    Ok(())
}

fn validate_directory_metadata(metadata: &Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid("MCP secret path component is not a real directory"));
    }
    Ok(())
}

fn validate_file_metadata(metadata: &Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("MCP secret path is not a regular file"));
    }
    Ok(())
}

fn validate_single_component(path: &Path) -> io::Result<()> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(invalid("MCP secret path contains unsafe components"));
    }
    Ok(())
}

fn single_relative_component(path: &Path) -> io::Result<PathBuf> {
    validate_single_component(path)?;
    Ok(path.to_path_buf())
}

fn combined_io_error(primary: io::Error, recovery: io::Error) -> io::Error {
    io::Error::new(
        primary.kind(),
        format!("{primary}; recovery also failed: {recovery}"),
    )
}

fn random_hex_io() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| io::Error::other("secure random generation failed"))?;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(unix)]
fn set_no_follow(options: &mut OpenOptions) {
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_create_mode(options: &mut OpenOptions) {
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_create_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_private_dir_permissions(directory: &Dir) -> io::Result<()> {
    directory.set_permissions(
        Path::new("."),
        Permissions::from_std(std::fs::Permissions::from_mode(0o700)),
    )
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_directory: &Dir) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &File) -> io::Result<()> {
    file.set_permissions(Permissions::from_std(std::fs::Permissions::from_mode(
        0o600,
    )))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_cap_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn same_std_cap_metadata(left: &std::fs::Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_std_cap_metadata(left: &std::fs::Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok().map(Into::into) == right.modified().ok()
}
