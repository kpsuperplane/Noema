//! Supermemory server binary resolution.

use std::{env, path::PathBuf};

use thiserror::Error;

/// Environment variable that overrides the managed Supermemory server binary.
pub const NOEMA_SUPERMEMORY_SERVER_ENV: &str = "NOEMA_SUPERMEMORY_SERVER";

/// Resolved Supermemory server executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupermemoryServerBinary {
    /// Executable path to launch.
    pub path: PathBuf,
    /// Source that produced this executable path.
    pub source: SupermemoryBinarySource,
}

/// Origin for a resolved Supermemory server executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupermemoryBinarySource {
    /// Explicit `NOEMA_SUPERMEMORY_SERVER` override.
    Environment,
    /// Bundled Noema resource candidate.
    Bundled,
    /// Development fallback found on `PATH`.
    Path,
}

/// Resolver for the managed Supermemory server executable.
#[derive(Debug, Clone)]
pub struct SupermemoryBinaryResolver {
    bundled_root: PathBuf,
    path_lookup_enabled: bool,
}

impl SupermemoryBinaryResolver {
    /// Create a resolver with a bundled resource root and optional `PATH` fallback.
    #[must_use]
    pub fn new(bundled_root: PathBuf, path_lookup_enabled: bool) -> Self {
        Self {
            bundled_root,
            path_lookup_enabled,
        }
    }

    /// Create the default resolver for a Noema installation.
    #[must_use]
    pub fn default_for_paths(paths: &crate::NoemaPaths) -> Self {
        Self::new(default_bundled_root(paths), true)
    }

    /// Resolve the executable using the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryBinaryError`] when no candidate exists or the
    /// selected candidate cannot be inspected or executed.
    pub fn resolve(&self) -> Result<SupermemoryServerBinary, SupermemoryBinaryError> {
        let env_path = env::var_os(NOEMA_SUPERMEMORY_SERVER_ENV).map(PathBuf::from);
        self.resolve_with_env(env_path)
    }

    /// Resolve the executable with an explicit environment override value.
    ///
    /// # Errors
    ///
    /// Returns [`SupermemoryBinaryError`] when no candidate exists or the
    /// selected candidate cannot be inspected or executed.
    pub fn resolve_with_env(
        &self,
        env_path: Option<PathBuf>,
    ) -> Result<SupermemoryServerBinary, SupermemoryBinaryError> {
        if let Some(path) = env_path {
            return executable_candidate(path, SupermemoryBinarySource::Environment);
        }

        let bundled = self.bundled_root.join(server_filename());
        if bundled.exists() {
            return executable_candidate(bundled, SupermemoryBinarySource::Bundled);
        }

        if self.path_lookup_enabled
            && let Some(path) = lookup_path(server_filename())
        {
            return executable_candidate(path, SupermemoryBinarySource::Path);
        }

        Err(SupermemoryBinaryError::Missing)
    }
}

/// Errors returned while resolving the managed Supermemory server executable.
#[derive(Debug, Error)]
pub enum SupermemoryBinaryError {
    /// No executable candidate was found.
    #[error("supermemory-server executable was not found")]
    Missing,
    /// A candidate exists but is not executable.
    #[error("supermemory-server candidate is not executable: {path}")]
    NotExecutable {
        /// Candidate path.
        path: PathBuf,
    },
    /// A candidate could not be inspected.
    #[error("supermemory-server candidate could not be inspected at {path}: {message}")]
    Io {
        /// Candidate path.
        path: PathBuf,
        /// Sanitized inspection failure.
        message: String,
    },
}

impl SupermemoryBinaryError {
    /// Stable status code for persisted service status.
    #[must_use]
    pub const fn sanitized_code(&self) -> &'static str {
        match self {
            Self::Missing => "supermemory_server_missing",
            Self::NotExecutable { .. } => "supermemory_server_not_executable",
            Self::Io { .. } => "supermemory_start_failed",
        }
    }

    /// Stable status message for persisted service status.
    #[must_use]
    pub const fn sanitized_message(&self) -> &'static str {
        match self {
            Self::Missing => "supermemory-server executable was not found",
            Self::NotExecutable { .. } => "supermemory-server executable is not runnable",
            Self::Io { .. } => "supermemory-server executable could not be inspected",
        }
    }
}

/// Platform-specific Supermemory server executable file name.
#[must_use]
pub fn server_filename() -> &'static str {
    if cfg!(windows) {
        "supermemory-server.exe"
    } else {
        "supermemory-server"
    }
}

fn default_bundled_root(_paths: &crate::NoemaPaths) -> PathBuf {
    PathBuf::from(env!("NOEMA_BUNDLED_SUPERMEMORY_DIR"))
}

fn executable_candidate(
    path: PathBuf,
    source: SupermemoryBinarySource,
) -> Result<SupermemoryServerBinary, SupermemoryBinaryError> {
    let metadata = std::fs::metadata(&path).map_err(|error| SupermemoryBinaryError::Io {
        path: path.clone(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(SupermemoryBinaryError::NotExecutable { path });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(SupermemoryBinaryError::NotExecutable { path });
        }
    }
    Ok(SupermemoryServerBinary { path, source })
}

fn lookup_path(program: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|entry| entry.join(program))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    #[test]
    fn resolver_prefers_env_override() {
        let temp = TempDir::new().expect("temp");
        let env_binary = temp.path().join("env-supermemory-server");
        write_executable(&env_binary);
        let bundled = temp.path().join("bundle");
        fs::create_dir_all(&bundled).expect("bundle dir");
        write_executable(&bundled.join(super::server_filename()));

        let resolver = super::SupermemoryBinaryResolver::new(bundled, false);
        let resolved = resolver
            .resolve_with_env(Some(env_binary.clone()))
            .expect("resolve");

        assert_eq!(resolved.path, env_binary);
        assert_eq!(resolved.source, super::SupermemoryBinarySource::Environment);
    }

    #[test]
    fn resolver_uses_bundled_candidate_before_path_lookup() {
        let temp = TempDir::new().expect("temp");
        let bundled = temp.path().join("bundle");
        fs::create_dir_all(&bundled).expect("bundle dir");
        let bundled_binary = bundled.join(super::server_filename());
        write_executable(&bundled_binary);

        let resolver = super::SupermemoryBinaryResolver::new(bundled, false);
        let resolved = resolver.resolve_with_env(None).expect("resolve");

        assert_eq!(resolved.path, bundled_binary);
        assert_eq!(resolved.source, super::SupermemoryBinarySource::Bundled);
    }

    #[test]
    fn resolver_reports_missing_when_no_candidate_exists() {
        let temp = TempDir::new().expect("temp");
        let resolver = super::SupermemoryBinaryResolver::new(temp.path().join("missing"), false);

        let error = resolver.resolve_with_env(None).expect_err("missing");

        assert!(matches!(error, super::SupermemoryBinaryError::Missing));
    }

    fn write_executable(path: &PathBuf) {
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod");
        }
    }
}
