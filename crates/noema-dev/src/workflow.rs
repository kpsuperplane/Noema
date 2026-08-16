use std::{
    ffi::OsString,
    fs, io,
    path::{Component, Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};

use thiserror::Error;
use tokio::process::Command;

use crate::{cargo_exe, strip_cargo_run_env};

const GIB: u64 = 1024 * 1024 * 1024;
const CACHE_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const CACHE_CHECK_SENTINEL: &str = ".noema-cache-budget-checked";
const CACHE_TEMP_DIRECTORY: &str = "tmp";
const CARGO_CACHE_TAG: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55\n\
# This file is a cache directory tag created by cargo.\n\
# For information about cache directory tags see https://bford.info/cachedir/\n";
const DEV_CACHE: CacheTarget = CacheTarget::new("development server", "noema-dev", 30 * GIB);
const VALIDATION_CACHE: CacheTarget =
    CacheTarget::new("Rust validation", "noema-validation", 20 * GIB);

#[derive(Clone, Copy)]
struct CacheTarget {
    label: &'static str,
    directory: &'static str,
    max_bytes: u64,
}

impl CacheTarget {
    const fn new(label: &'static str, directory: &'static str, max_bytes: u64) -> Self {
        Self {
            label,
            directory,
            max_bytes,
        }
    }

    fn path(self, repo_root: &Path) -> PathBuf {
        repo_root.join("target").join(self.directory)
    }

    fn temp_path(self, repo_root: &Path) -> PathBuf {
        self.path(repo_root).join(CACHE_TEMP_DIRECTORY)
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkflowError {
    #[error("validation requires a Cargo command")]
    MissingValidationCommand,

    #[error("refusing to clean unsafe Cargo target: {path}")]
    UnsafeCacheTarget { path: PathBuf },

    #[error("failed to inspect {label} Cargo cache: {source}")]
    InspectCache {
        label: &'static str,
        source: io::Error,
    },

    #[error("failed to prepare {label} Cargo cache: {source}")]
    PrepareCache {
        label: &'static str,
        source: io::Error,
    },

    #[error("failed to start Cargo: {source}")]
    SpawnCargo { source: io::Error },

    #[error("Cargo exited with status {status}")]
    CargoExited { status: ExitStatus },
}

pub(crate) async fn run_validation(cargo_args: Vec<OsString>) -> Result<(), WorkflowError> {
    if cargo_args.is_empty() {
        return Err(WorkflowError::MissingValidationCommand);
    }

    let repo_root = crate::repo_root();
    enforce_cache_budget(&repo_root, VALIDATION_CACHE).await?;

    let mut command = Command::new(cargo_exe());
    command
        .args(cargo_args)
        .current_dir(&repo_root)
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "1")
        .env("CARGO_TARGET_DIR", VALIDATION_CACHE.path(&repo_root))
        .env("TMPDIR", VALIDATION_CACHE.temp_path(&repo_root))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    strip_cargo_run_env(&mut command);

    let status = command
        .status()
        .await
        .map_err(|source| WorkflowError::SpawnCargo { source })?;
    if status.success() {
        Ok(())
    } else {
        Err(WorkflowError::CargoExited { status })
    }
}

pub(crate) async fn run_development_server() -> Result<(), WorkflowError> {
    let repo_root = crate::repo_root();
    enforce_cache_budget(&repo_root, DEV_CACHE).await?;

    let mut command = Command::new(cargo_exe());
    command
        .args(["run", "-p", "noema-server", "--bin", "noema_web"])
        .current_dir(&repo_root)
        .env("CARGO_TARGET_DIR", DEV_CACHE.path(&repo_root))
        .env("TMPDIR", DEV_CACHE.temp_path(&repo_root))
        .env("NOEMA_WEB__HOST", "0.0.0.0")
        .env("NOEMA_WEB__DEV_NO_AUTH", "true")
        .env("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "true")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    strip_cargo_run_env(&mut command);

    let status = command
        .status()
        .await
        .map_err(|source| WorkflowError::SpawnCargo { source })?;
    if status.success() {
        Ok(())
    } else {
        Err(WorkflowError::CargoExited { status })
    }
}

async fn enforce_cache_budget(repo_root: &Path, target: CacheTarget) -> Result<(), WorkflowError> {
    let target_path = target.path(repo_root);
    if !cache_target_is_safe(repo_root, &target_path) {
        return Err(WorkflowError::UnsafeCacheTarget { path: target_path });
    }
    prepare_cache_target(&target_path).map_err(|source| WorkflowError::PrepareCache {
        label: target.label,
        source,
    })?;
    if cache_check_is_current(&target_path) {
        return Ok(());
    }

    let size_bytes =
        directory_size_bytes(&target_path)
            .await
            .map_err(|source| WorkflowError::InspectCache {
                label: target.label,
                source,
            })?;
    if should_clean_cache(size_bytes, target.max_bytes) {
        eprintln!(
            "{} Cargo cache is {:.1} GiB (limit {} GiB); rebuilding the disposable cache",
            target.label,
            size_bytes as f64 / GIB as f64,
            target.max_bytes / GIB
        );
        clean_cache(repo_root, &target_path).await?;
        prepare_cache_target(&target_path).map_err(|source| WorkflowError::PrepareCache {
            label: target.label,
            source,
        })?;
    }

    fs::write(target_path.join(CACHE_CHECK_SENTINEL), b"").map_err(|source| {
        WorkflowError::PrepareCache {
            label: target.label,
            source,
        }
    })
}

fn prepare_cache_target(target_path: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(target_path)?;
    fs::create_dir_all(target_path.join(CACHE_TEMP_DIRECTORY))?;
    let tag_path = target_path.join("CACHEDIR.TAG");
    if !fs::read(&tag_path).is_ok_and(|contents| contents == CARGO_CACHE_TAG) {
        fs::write(tag_path, CARGO_CACHE_TAG)?;
    }
    Ok(())
}

fn cache_check_is_current(target_path: &Path) -> bool {
    fs::metadata(target_path.join(CACHE_CHECK_SENTINEL))
        .and_then(|metadata| metadata.modified())
        .and_then(|modified| modified.elapsed().map_err(io::Error::other))
        .is_ok_and(|elapsed| elapsed < CACHE_CHECK_INTERVAL)
}

#[cfg(unix)]
async fn directory_size_bytes(target_path: &Path) -> Result<u64, io::Error> {
    if !target_path.exists() {
        return Ok(0);
    }

    let output = Command::new("du")
        .arg("-sk")
        .arg(target_path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(io::Error::other("du failed to measure target directory"));
    }

    let kibibytes = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| io::Error::other("du returned an invalid size"))?;
    Ok(kibibytes * 1024)
}

#[cfg(not(unix))]
async fn directory_size_bytes(target_path: &Path) -> Result<u64, io::Error> {
    let mut total = 0;
    let mut pending = vec![target_path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                total += metadata.len();
            }
        }
    }
    Ok(total)
}

async fn clean_cache(repo_root: &Path, target_path: &Path) -> Result<(), WorkflowError> {
    let status = Command::new(cargo_exe())
        .arg("clean")
        .arg("--target-dir")
        .arg(target_path)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|source| WorkflowError::SpawnCargo { source })?;
    if status.success() {
        Ok(())
    } else {
        Err(WorkflowError::CargoExited { status })
    }
}

fn cache_target_is_safe(repo_root: &Path, target_path: &Path) -> bool {
    let Ok(relative) = target_path.strip_prefix(repo_root) else {
        return false;
    };
    let mut components = relative.components();
    matches!(components.next(), Some(Component::Normal(value)) if value == "target")
        && matches!(components.next(), Some(Component::Normal(value)) if matches!(value.to_str(), Some("noema-dev" | "noema-validation")))
        && components.next().is_none()
}

fn should_clean_cache(size_bytes: u64, max_bytes: u64) -> bool {
    size_bytes > max_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_named_noema_caches_below_repository_target() {
        let root = Path::new("/workspace/noema");

        assert!(cache_target_is_safe(
            root,
            &root.join("target/noema-validation")
        ));
        assert!(cache_target_is_safe(root, &root.join("target/noema-dev")));
        assert!(!cache_target_is_safe(root, &root.join("target")));
        assert!(!cache_target_is_safe(root, root));
        assert!(!cache_target_is_safe(
            root,
            Path::new("/tmp/noema-validation")
        ));
        assert!(!cache_target_is_safe(
            root,
            &root.join("target/noema-dev/nested")
        ));
        assert!(!cache_target_is_safe(
            root,
            &root.join("target/noema-other")
        ));
    }

    #[test]
    fn cleans_only_after_cache_exceeds_its_byte_budget() {
        assert!(!should_clean_cache(20, 20));
        assert!(should_clean_cache(21, 20));
    }

    #[test]
    fn prepares_cache_with_cargo_ownership_tag() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("cache");

        prepare_cache_target(&directory).unwrap();

        assert_eq!(
            fs::read(directory.join("CACHEDIR.TAG")).unwrap(),
            CARGO_CACHE_TAG
        );
        assert!(directory.join(CACHE_TEMP_DIRECTORY).is_dir());
    }

    #[test]
    fn repairs_invalid_cargo_ownership_tag() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path();
        fs::write(directory.join("CACHEDIR.TAG"), b"invalid").unwrap();

        prepare_cache_target(directory).unwrap();

        assert_eq!(
            fs::read(directory.join("CACHEDIR.TAG")).unwrap(),
            CARGO_CACHE_TAG
        );
    }
}
