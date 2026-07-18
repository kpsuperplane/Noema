//! Dev-only provisioning for the managed Mnemosyne sidecar.

use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::process::Command;

use super::DevError;

pub(super) const SIDECAR_COMMAND_ENV: &str = "NOEMA_MNEMOSYNE_SIDECAR_COMMAND";
const SIDECAR_PORT_ENV: &str = "NOEMA_MNEMOSYNE_PORT";
const SIDECAR_SOURCE_DIR: &str = "crates/noema-memory/mnemosyne-sidecar";
const SIDECAR_VENV_DIR: &str = "crates/noema-memory/target/mnemosyne-sidecar-venv";
const INSTALL_STAMP_FILE: &str = ".noema-install.stamp";

pub(super) async fn ensure_dev_sidecar(repo_root: &Path) -> Result<Option<String>, DevError> {
    if env::var_os(SIDECAR_COMMAND_ENV).is_some() {
        eprintln!("using {SIDECAR_COMMAND_ENV} override for managed Mnemosyne");
        return Ok(None);
    }

    let python = venv_python(repo_root);
    if !executable_exists(&python) {
        let base_python = base_python().await?;
        eprintln!(
            "creating dev Mnemosyne sidecar environment at {}",
            venv_dir(repo_root).display()
        );
        let status = venv_create_command(repo_root, &base_python)
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if !status.success() {
            return Err(DevError::MnemosyneInstallerExited { status });
        }
    }

    if install_is_current(repo_root)? {
        eprintln!(
            "reusing dev Mnemosyne sidecar package in {}",
            venv_dir(repo_root).display()
        );
    } else {
        eprintln!(
            "installing dev Mnemosyne sidecar package into {}",
            venv_dir(repo_root).display()
        );
        let status = install_command(repo_root)
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if !status.success() {
            return Err(DevError::MnemosyneInstallerExited { status });
        }
        mark_install_current(repo_root)?;
    }
    let command = sidecar_uvicorn_command(&python);
    eprintln!("dev Mnemosyne sidecar: {}", python.display());
    Ok(Some(command))
}

fn sidecar_uvicorn_command(python: &Path) -> String {
    format!(
        "{} -m uvicorn --factory noema_mnemosyne_sidecar.app:create_app --host 127.0.0.1 --port \"${SIDECAR_PORT_ENV}\"",
        shell_quote(python)
    )
}

fn install_stamp_path(repo_root: &Path) -> PathBuf {
    venv_dir(repo_root).join(INSTALL_STAMP_FILE)
}

fn install_is_current(repo_root: &Path) -> Result<bool, DevError> {
    let Ok(stamp) = std::fs::metadata(install_stamp_path(repo_root)) else {
        return Ok(false);
    };
    let manifest = std::fs::metadata(source_dir(repo_root).join("pyproject.toml"))
        .map_err(|source| DevError::InstallMnemosyne { source })?;
    let stamp_modified = stamp
        .modified()
        .map_err(|source| DevError::InstallMnemosyne { source })?;
    let manifest_modified = manifest
        .modified()
        .map_err(|source| DevError::InstallMnemosyne { source })?;
    Ok(stamp_modified >= manifest_modified)
}

fn mark_install_current(repo_root: &Path) -> Result<(), DevError> {
    std::fs::write(install_stamp_path(repo_root), b"editable")
        .map_err(|source| DevError::InstallMnemosyne { source })
}

async fn base_python() -> Result<PathBuf, DevError> {
    for candidate in python_candidates() {
        if !executable_exists(&candidate) {
            continue;
        }
        let status = Command::new(&candidate)
            .arg("-c")
            .arg("import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|source| DevError::InstallMnemosyne { source })?;
        if status.success() {
            return Ok(candidate);
        }
    }
    Err(DevError::MissingMnemosynePython)
}

fn python_candidates() -> Vec<PathBuf> {
    [
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "/opt/homebrew/bin/python3.13",
        "/opt/homebrew/bin/python3.12",
        "/opt/homebrew/bin/python3.11",
        "/opt/homebrew/bin/python3.10",
        "/usr/local/bin/python3.13",
        "/usr/local/bin/python3.12",
        "/usr/local/bin/python3.11",
        "/usr/local/bin/python3.10",
        "python3",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn venv_create_command(repo_root: &Path, base_python: &Path) -> Command {
    let mut command = Command::new(base_python);
    command
        .arg("-m")
        .arg("venv")
        .arg(venv_dir(repo_root))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn install_command(repo_root: &Path) -> Command {
    let mut command = Command::new(venv_python(repo_root));
    command
        .args(["-m", "pip", "install", "--editable"])
        .arg(source_dir(repo_root))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

fn source_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(SIDECAR_SOURCE_DIR)
}

fn venv_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(SIDECAR_VENV_DIR)
}

fn venv_python(repo_root: &Path) -> PathBuf {
    if cfg!(windows) {
        venv_dir(repo_root).join("Scripts/python.exe")
    } else {
        venv_dir(repo_root).join("bin/python")
    }
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn executable_exists(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemosyne_dev_paths_target_generated_venv() {
        let root = Path::new("/workspace");
        assert_eq!(
            source_dir(root),
            PathBuf::from("/workspace/crates/noema-memory/mnemosyne-sidecar")
        );
        assert_eq!(
            venv_python(root),
            PathBuf::from(
                "/workspace/crates/noema-memory/target/mnemosyne-sidecar-venv/bin/python"
            )
        );
        assert_eq!(
            install_stamp_path(root),
            PathBuf::from(
                "/workspace/crates/noema-memory/target/mnemosyne-sidecar-venv/.noema-install.stamp"
            )
        );
    }

    #[test]
    fn mnemosyne_dev_sidecar_command_uses_asgi_factory() {
        assert_eq!(SIDECAR_COMMAND_ENV, "NOEMA_MNEMOSYNE_SIDECAR_COMMAND");
        assert_eq!(
            sidecar_uvicorn_command(Path::new("/workspace/.venv/bin/python")),
            "'/workspace/.venv/bin/python' -m uvicorn --factory noema_mnemosyne_sidecar.app:create_app --host 127.0.0.1 --port \"$NOEMA_MNEMOSYNE_PORT\""
        );
    }

    #[test]
    fn mnemosyne_dev_install_uses_editable_source_package() {
        let command = install_command(Path::new("/workspace"));
        assert_eq!(
            command.as_std().get_program(),
            "/workspace/crates/noema-memory/target/mnemosyne-sidecar-venv/bin/python"
        );
        assert_eq!(
            command
                .as_std()
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "-m",
                "pip",
                "install",
                "--editable",
                "/workspace/crates/noema-memory/mnemosyne-sidecar"
            ]
        );
    }
}
