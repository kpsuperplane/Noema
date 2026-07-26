//! Immutable content-addressed definition and source storage.

use crate::{
    AdapterCompileError, AdapterCompiler, AdapterManifestV1, CompiledAdapterDefinition,
    SemanticDigest, SourceDigest,
    digest::{canonical_json_bytes, semantic_manifest_value},
};
use noema_home::NoemaPaths;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use thiserror::Error;

const MANIFEST_FILE: &str = "manifest.json";
const PROVENANCE_FILE: &str = "provenance.json";
const STAGING_DIR: &str = ".staging";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_PROVENANCE_BYTES: u64 = 64 * 1024;
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Non-authoritative source provenance stored beside a canonical manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionProvenance {
    /// Digest of retained exact source bytes, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<SourceDigest>,
    /// Closed source document extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_extension: Option<String>,
    /// Human-auditable source reference. It is never policy authority.
    pub source_reference: String,
    /// Optional volatile import timestamp, excluded from semantic identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<String>,
}

/// Canonical definition store rooted in one `NOEMA_HOME`.
#[derive(Debug, Clone)]
pub struct AdapterDefinitionStore {
    paths: NoemaPaths,
}

/// Successful immutable installation.
#[derive(Debug, Clone)]
pub struct DefinitionInstall {
    /// Compiled immutable operation plans.
    pub compiled: CompiledAdapterDefinition,
    /// Rebuildable SQLite projection row.
    pub projection: DefinitionProjection,
}

/// Safe filesystem-derived definition index row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionProjection {
    /// Directory content address.
    pub semantic_digest: String,
    /// Stable definition identity when parsing succeeded.
    pub definition_id: Option<String>,
    /// Stable adapter family when parsing succeeded.
    pub adapter_id: Option<String>,
    /// Retained exact-source digest.
    pub source_digest: Option<String>,
    /// Safe path relative to `NOEMA_HOME`.
    pub manifest_relative_path: String,
    /// Safe path relative to `NOEMA_HOME`.
    pub provenance_relative_path: String,
    /// `compiled` or `blocked`.
    pub compile_status: &'static str,
    /// `reviewed`, `pending`, or `unknown`.
    pub review_status: &'static str,
    /// Bounded stable diagnostic category.
    pub diagnostic_code: Option<&'static str>,
    /// Number of compiled operations.
    pub operation_count: usize,
    /// Stable compiler identity.
    pub compiler_version: &'static str,
}

/// Deterministic scan result, including blocked objects.
#[derive(Debug)]
pub struct DefinitionScan {
    /// Valid compiled definitions.
    pub definitions: Vec<DefinitionInstall>,
    /// Safe diagnostics for blocked content-addressed objects.
    pub diagnostics: Vec<DefinitionScanDiagnostic>,
}

impl DefinitionScan {
    /// Return every filesystem-owned projection row in semantic-digest order.
    #[must_use]
    pub fn projections(&self) -> Vec<DefinitionProjection> {
        let mut rows = self
            .definitions
            .iter()
            .map(|definition| definition.projection.clone())
            .chain(
                self.diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.projection.clone()),
            )
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.semantic_digest.cmp(&right.semantic_digest));
        rows
    }
}

/// Safe scan diagnostic for an invalid canonical object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionScanDiagnostic {
    /// Stable, non-secret failure category.
    pub code: &'static str,
    /// Blocked projection retained when the directory has a valid content address.
    pub projection: Option<DefinitionProjection>,
}

/// Definition filesystem failure.
#[derive(Debug, Error)]
pub enum DefinitionStoreError {
    /// Filesystem operation failed.
    #[error("adapter definition filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// Path validation failed.
    #[error("adapter definition path is invalid: {0}")]
    Path(#[from] noema_home::NoemaPathError),
    /// Canonical JSON operation failed.
    #[error("adapter definition JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Definition compilation failed.
    #[error("adapter definition compilation failed: {0}")]
    Compile(#[from] AdapterCompileError),
    /// Source bytes or provenance disagreed with its content address.
    #[error("adapter definition content-address invariant failed: {0}")]
    Integrity(&'static str),
}

impl AdapterDefinitionStore {
    /// Create a filesystem store handle. Directories are created lazily.
    #[must_use]
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    /// Install one canonical manifest and optional exact source snapshot.
    ///
    /// The operation is immutable and idempotent by semantic digest. A crash
    /// can leave only an ignored staging directory, never a partial canonical
    /// object.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionStoreError`] for invalid content, unsafe paths,
    /// digest conflicts, or filesystem failures.
    pub fn install(
        &self,
        manifest: &AdapterManifestV1,
        source_reference: &str,
        imported_at: Option<&str>,
        source: Option<(&[u8], &str)>,
    ) -> Result<DefinitionInstall, DefinitionStoreError> {
        validate_provenance_text(source_reference, 4_096)?;
        if let Some(imported_at) = imported_at {
            validate_provenance_text(imported_at, 128)?;
        }
        let compiled = AdapterCompiler::compile(manifest)?;
        let manifest_value = semantic_manifest_value(manifest)?;
        let manifest_bytes = canonical_json_bytes(&manifest_value)?;
        if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(DefinitionStoreError::Integrity("manifest_oversized"));
        }
        let (source_digest, source_extension) = if let Some((bytes, extension)) = source {
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err(DefinitionStoreError::Integrity("source_oversized"));
            }
            let digest = SourceDigest::compute(bytes);
            self.install_source(&digest, extension, bytes)?;
            (Some(digest), Some(extension.to_string()))
        } else {
            (None, None)
        };
        let provenance = DefinitionProvenance {
            source_digest,
            source_extension,
            source_reference: source_reference.to_string(),
            imported_at: imported_at.map(str::to_string),
        };
        let provenance_bytes = canonical_json_bytes(&serde_json::to_value(&provenance)?)?;
        let definition_dir = self
            .paths
            .adapter_definition_dir(compiled.semantic_digest.as_str())?;
        self.prepare_roots()?;
        if definition_dir.exists() {
            let installed =
                self.read_definition_dir(&definition_dir, compiled.semantic_digest.as_str())?;
            if installed.compiled.semantic_digest != compiled.semantic_digest {
                return Err(DefinitionStoreError::Integrity("definition_conflict"));
            }
            return Ok(installed);
        }
        let staging_root = self.paths.adapter_definitions_dir().join(STAGING_DIR);
        create_private_dir(&staging_root)?;
        let staging = staging_root.join(random_stage_name()?);
        create_private_dir(&staging)?;
        let install_result = (|| {
            write_new_file(&staging.join(MANIFEST_FILE), &manifest_bytes)?;
            write_new_file(&staging.join(PROVENANCE_FILE), &provenance_bytes)?;
            File::open(&staging)?.sync_all()?;
            match fs::rename(&staging, &definition_dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            File::open(self.paths.adapter_definitions_dir())?.sync_all()?;
            self.read_definition_dir(&definition_dir, compiled.semantic_digest.as_str())
        })();
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        install_result
    }

    /// Scan canonical definition objects deterministically without network use.
    ///
    /// Invalid individual objects become blocked diagnostics. Failure to open
    /// the authoritative root remains a store error.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionStoreError`] when the root cannot be prepared or read.
    pub fn scan(&self) -> Result<DefinitionScan, DefinitionStoreError> {
        self.prepare_roots()?;
        let mut entries =
            fs::read_dir(self.paths.adapter_definitions_dir())?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut definitions = Vec::new();
        let mut diagnostics = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                diagnostics.push(DefinitionScanDiagnostic {
                    code: "definition_name",
                    projection: None,
                });
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if SemanticDigest::parse(name.to_string()).is_err() {
                diagnostics.push(DefinitionScanDiagnostic {
                    code: "definition_name",
                    projection: None,
                });
                continue;
            }
            let path = entry.path();
            match self.read_definition_dir(&path, name) {
                Ok(definition) => definitions.push(definition),
                Err(error) => {
                    let code = diagnostic_code(&error);
                    diagnostics.push(DefinitionScanDiagnostic {
                        code,
                        projection: Some(blocked_projection(&self.paths, name, code)),
                    });
                }
            }
        }
        Ok(DefinitionScan {
            definitions,
            diagnostics,
        })
    }

    fn prepare_roots(&self) -> Result<(), DefinitionStoreError> {
        require_directory_no_symlink(self.paths.root())?;
        create_private_dir(&self.paths.adapters_dir())?;
        create_private_dir(&self.paths.adapter_definitions_dir())?;
        create_private_dir(&self.paths.adapter_sources_dir())?;
        create_private_dir(&self.paths.adapter_quarantine_dir())?;
        Ok(())
    }

    fn install_source(
        &self,
        digest: &SourceDigest,
        extension: &str,
        bytes: &[u8],
    ) -> Result<(), DefinitionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_source_path(digest.as_str(), extension)?;
        if target.exists() {
            let existing = read_bounded_regular_file(&target, MAX_SOURCE_BYTES as u64)?;
            if existing != bytes || SourceDigest::compute(&existing) != *digest {
                return Err(DefinitionStoreError::Integrity("source_conflict"));
            }
            return Ok(());
        }
        let temporary = self.paths.adapter_sources_dir().join(format!(
            ".{}-{}",
            digest.as_str(),
            random_stage_name()?
        ));
        write_new_file(&temporary, bytes)?;
        match fs::rename(&temporary, &target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary);
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
        }
        let existing = read_bounded_regular_file(&target, MAX_SOURCE_BYTES as u64)?;
        if SourceDigest::compute(&existing) != *digest {
            return Err(DefinitionStoreError::Integrity("source_digest"));
        }
        File::open(self.paths.adapter_sources_dir())?.sync_all()?;
        Ok(())
    }

    fn read_definition_dir(
        &self,
        path: &Path,
        expected_digest: &str,
    ) -> Result<DefinitionInstall, DefinitionStoreError> {
        require_regular_directory(path)?;
        require_exact_definition_entries(path)?;
        let manifest_bytes =
            read_bounded_regular_file(&path.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
        let provenance_bytes =
            read_bounded_regular_file(&path.join(PROVENANCE_FILE), MAX_PROVENANCE_BYTES)?;
        let manifest: AdapterManifestV1 = serde_json::from_slice(&manifest_bytes)?;
        if canonical_json_bytes(&serde_json::to_value(&manifest)?)? != manifest_bytes {
            return Err(DefinitionStoreError::Integrity("manifest_not_canonical"));
        }
        let compiled = AdapterCompiler::compile(&manifest)?;
        if compiled.semantic_digest.as_str() != expected_digest {
            return Err(DefinitionStoreError::Integrity("semantic_digest"));
        }
        let provenance: DefinitionProvenance = serde_json::from_slice(&provenance_bytes)?;
        if canonical_json_bytes(&serde_json::to_value(&provenance)?)? != provenance_bytes {
            return Err(DefinitionStoreError::Integrity("provenance_not_canonical"));
        }
        self.verify_source(&provenance)?;
        Ok(DefinitionInstall {
            projection: compiled_projection(&self.paths, &compiled, &provenance),
            compiled,
        })
    }

    fn verify_source(&self, provenance: &DefinitionProvenance) -> Result<(), DefinitionStoreError> {
        match (&provenance.source_digest, &provenance.source_extension) {
            (None, None) => Ok(()),
            (Some(digest), Some(extension)) => {
                let path = self.paths.adapter_source_path(digest.as_str(), extension)?;
                let bytes = read_bounded_regular_file(&path, MAX_SOURCE_BYTES as u64)?;
                if SourceDigest::compute(&bytes) != *digest {
                    return Err(DefinitionStoreError::Integrity("source_digest"));
                }
                Ok(())
            }
            _ => Err(DefinitionStoreError::Integrity("source_provenance")),
        }
    }
}

fn compiled_projection(
    paths: &NoemaPaths,
    compiled: &CompiledAdapterDefinition,
    provenance: &DefinitionProvenance,
) -> DefinitionProjection {
    let digest = compiled.semantic_digest.as_str();
    DefinitionProjection {
        semantic_digest: digest.to_string(),
        definition_id: Some(compiled.definition_id.clone()),
        adapter_id: Some(compiled.adapter_id.clone()),
        source_digest: provenance.source_digest.as_ref().map(ToString::to_string),
        manifest_relative_path: relative_definition_path(paths, digest, MANIFEST_FILE),
        provenance_relative_path: relative_definition_path(paths, digest, PROVENANCE_FILE),
        compile_status: "compiled",
        review_status: if compiled.reviewed {
            "reviewed"
        } else {
            "pending"
        },
        diagnostic_code: None,
        operation_count: compiled.operations.len(),
        compiler_version: AdapterCompiler::version(),
    }
}

fn blocked_projection(
    paths: &NoemaPaths,
    digest: &str,
    code: &'static str,
) -> DefinitionProjection {
    DefinitionProjection {
        semantic_digest: digest.to_string(),
        definition_id: None,
        adapter_id: None,
        source_digest: None,
        manifest_relative_path: relative_definition_path(paths, digest, MANIFEST_FILE),
        provenance_relative_path: relative_definition_path(paths, digest, PROVENANCE_FILE),
        compile_status: "blocked",
        review_status: "unknown",
        diagnostic_code: Some(code),
        operation_count: 0,
        compiler_version: AdapterCompiler::version(),
    }
}

fn relative_definition_path(paths: &NoemaPaths, digest: &str, filename: &str) -> String {
    paths
        .adapter_definition_dir(digest)
        .expect("validated digest")
        .join(filename)
        .strip_prefix(paths.root())
        .expect("adapter path belongs to Noema home")
        .to_string_lossy()
        .into_owned()
}

fn create_private_dir(path: &Path) -> Result<(), std::io::Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(invalid_io("adapter directory is not a regular directory"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
        }
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn require_regular_directory(path: &Path) -> Result<(), DefinitionStoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || !has_private_permissions(&metadata)
    {
        return Err(DefinitionStoreError::Integrity("definition_directory"));
    }
    Ok(())
}

fn require_directory_no_symlink(path: &Path) -> Result<(), DefinitionStoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DefinitionStoreError::Integrity("noema_home"));
    }
    Ok(())
}

fn require_exact_definition_entries(path: &Path) -> Result<(), DefinitionStoreError> {
    let mut names = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    if names
        != [
            std::ffi::OsString::from(MANIFEST_FILE),
            std::ffi::OsString::from(PROVENANCE_FILE),
        ]
    {
        return Err(DefinitionStoreError::Integrity("definition_entries"));
    }
    Ok(())
}

fn read_bounded_regular_file(path: &Path, max: u64) -> Result<Vec<u8>, DefinitionStoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > max
        || !has_private_permissions(&metadata)
    {
        return Err(DefinitionStoreError::Integrity("object_file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != metadata.len() {
        return Err(DefinitionStoreError::Integrity("object_changed"));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(DefinitionStoreError::Integrity("object_oversized"));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn has_private_permissions(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn has_private_permissions(_metadata: &fs::Metadata) -> bool {
    true
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn random_stage_name() -> Result<String, DefinitionStoreError> {
    let mut bytes = [0_u8; 12];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| DefinitionStoreError::Integrity("randomness"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_provenance_text(value: &str, max: usize) -> Result<(), DefinitionStoreError> {
    if value.is_empty()
        || value.len() > max
        || value.trim() != value
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        Err(DefinitionStoreError::Integrity("provenance"))
    } else {
        Ok(())
    }
}

fn diagnostic_code(error: &DefinitionStoreError) -> &'static str {
    match error {
        DefinitionStoreError::Io(_) => "filesystem",
        DefinitionStoreError::Path(_) => "path",
        DefinitionStoreError::Json(_) => "json",
        DefinitionStoreError::Compile(_) => "compile",
        DefinitionStoreError::Integrity(code) => code,
    }
}

fn invalid_io(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests;
