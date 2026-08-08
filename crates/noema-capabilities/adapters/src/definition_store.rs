//! Immutable content-addressed definition and source storage.

use crate::{
    AdapterCompileError, AdapterCompiler, AdapterManifest, CompiledAdapterDefinition,
    SemanticDigest, SourceDigest,
    digest::{canonical_json_bytes, semantic_manifest_value},
    private_fs::{
        PrivateFsError, create_private_dir, random_hex, read_bounded_regular_file,
        require_directory_no_symlink, require_exact_entries, require_regular_directory,
        sync_directory, write_new_file,
    },
};
use noema_home::NoemaPaths;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};
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
    /// Exact earlier definition revisions replaced by this immutable object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replaces_semantic_digests: Vec<String>,
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

/// Exact canonical definition data used by trusted setup and review surfaces.
#[derive(Debug, Clone)]
pub struct StoredAdapterDefinition {
    /// Canonical manifest bytes parsed into the closed v7 vocabulary.
    pub manifest: AdapterManifest,
    /// Canonical source provenance stored beside the manifest.
    pub provenance: DefinitionProvenance,
    /// Exact retained source snapshot and extension, when one was installed.
    pub source: Option<(Vec<u8>, String)>,
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

impl From<PrivateFsError> for DefinitionStoreError {
    fn from(error: PrivateFsError) -> Self {
        match error {
            PrivateFsError::Io(error) => Self::Io(error),
            PrivateFsError::Integrity(code) => Self::Integrity(code),
        }
    }
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
        manifest: &AdapterManifest,
        source_reference: &str,
        imported_at: Option<&str>,
        source: Option<(&[u8], &str)>,
    ) -> Result<DefinitionInstall, DefinitionStoreError> {
        validate_provenance_text(source_reference, 4_096)?;
        if let Some(imported_at) = imported_at {
            validate_provenance_text(imported_at, 128)?;
        }
        let provenance = DefinitionProvenance {
            source_digest: None,
            source_extension: None,
            source_reference: source_reference.to_string(),
            imported_at: imported_at.map(str::to_string),
            replaces_semantic_digests: Vec::new(),
        };
        self.install_with_provenance(manifest, provenance, source)
    }

    /// Install one immutable definition with trusted revision provenance.
    pub(crate) fn install_with_provenance(
        &self,
        manifest: &AdapterManifest,
        mut provenance: DefinitionProvenance,
        source: Option<(&[u8], &str)>,
    ) -> Result<DefinitionInstall, DefinitionStoreError> {
        validate_provenance_text(&provenance.source_reference, 4_096)?;
        if let Some(imported_at) = &provenance.imported_at {
            validate_provenance_text(imported_at, 128)?;
        }
        provenance.replaces_semantic_digests.sort();
        provenance.replaces_semantic_digests.dedup();
        for digest in &provenance.replaces_semantic_digests {
            SemanticDigest::parse(digest.clone())
                .map_err(|_| DefinitionStoreError::Integrity("replacement_digest"))?;
        }
        let compiled = AdapterCompiler::compile(manifest)?;
        if provenance
            .replaces_semantic_digests
            .iter()
            .any(|digest| digest == compiled.semantic_digest.as_str())
        {
            return Err(DefinitionStoreError::Integrity("replacement_cycle"));
        }
        let mut manifest_value = semantic_manifest_value(manifest)?;
        if let Some(display_name) = &manifest.display_name {
            manifest_value
                .as_object_mut()
                .ok_or(DefinitionStoreError::Integrity("manifest_shape"))?
                .insert(
                    "display_name".to_string(),
                    serde_json::Value::String(display_name.clone()),
                );
        }
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
        provenance.source_digest = source_digest;
        provenance.source_extension = source_extension;
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
            sync_directory(&staging)?;
            match fs::rename(&staging, &definition_dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            sync_directory(&self.paths.adapter_definitions_dir())?;
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

    /// Return pending revisions made non-actionable by a later proposal or approval.
    ///
    /// # Errors
    ///
    /// Returns a store error when any canonical definition or its lineage cannot be read.
    pub fn superseded_pending_digests(
        &self,
        scan: &DefinitionScan,
    ) -> Result<BTreeSet<String>, DefinitionStoreError> {
        let pending = scan
            .definitions
            .iter()
            .filter(|definition| !definition.compiled.reviewed)
            .map(|definition| definition.compiled.semantic_digest.to_string())
            .collect::<BTreeSet<_>>();
        let mut superseded = BTreeSet::new();
        for definition in &scan.definitions {
            let stored = self.load(definition.compiled.semantic_digest.as_str())?;
            superseded.extend(
                stored
                    .provenance
                    .replaces_semantic_digests
                    .iter()
                    .filter(|&digest| pending.contains(digest))
                    .cloned(),
            );
            if definition.compiled.reviewed {
                let mut draft = stored.manifest;
                draft.reviewed = false;
                let digest = AdapterCompiler::compile(&draft)?
                    .semantic_digest
                    .to_string();
                if pending.contains(&digest) {
                    superseded.insert(digest);
                }
            }
        }
        Ok(superseded)
    }

    /// Return exact earlier revisions replaced by reviewed definitions.
    ///
    /// # Errors
    ///
    /// Returns a store error when canonical reviewed provenance cannot be read.
    pub fn replaced_by_reviewed_digests(
        &self,
        scan: &DefinitionScan,
    ) -> Result<BTreeSet<String>, DefinitionStoreError> {
        let mut replaced = BTreeSet::new();
        for definition in &scan.definitions {
            let digest = definition.compiled.semantic_digest.as_str();
            let stored = self.load(digest)?;
            replaced.extend(stored.provenance.replaces_semantic_digests);
        }
        Ok(replaced)
    }

    /// Find retired pre-v7 definition objects without admitting them to discovery.
    pub(crate) fn legacy_definition_digests(
        &self,
    ) -> Result<BTreeSet<String>, DefinitionStoreError> {
        self.prepare_roots()?;
        let mut entries =
            fs::read_dir(self.paths.adapter_definitions_dir())?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut legacy = BTreeSet::new();
        for entry in entries {
            let Some(old_digest) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if SemanticDigest::parse(old_digest.clone()).is_err() {
                continue;
            }
            if Self::is_legacy_definition(&entry.path())? {
                legacy.insert(old_digest);
            }
        }
        Ok(legacy)
    }

    fn is_legacy_definition(path: &Path) -> Result<bool, DefinitionStoreError> {
        if require_regular_directory(path).is_err() {
            return Ok(false);
        }
        let Ok(manifest_bytes) =
            read_bounded_regular_file(&path.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)
        else {
            return Ok(false);
        };
        let Ok(manifest_value) = serde_json::from_slice::<serde_json::Value>(&manifest_bytes)
        else {
            return Ok(false);
        };
        Ok(matches!(
            manifest_value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(1..=6)
        ))
    }

    /// Move one exact definition out of discovery.
    pub(crate) fn quarantine(&self, digest: &str) -> Result<(), DefinitionStoreError> {
        SemanticDigest::parse(digest.to_string())
            .map_err(|_| DefinitionStoreError::Integrity("semantic_digest"))?;
        self.prepare_roots()?;
        let source = self.paths.adapter_definition_dir(digest)?;
        if !source.exists() {
            return Ok(());
        }
        require_regular_directory(&source)?;
        let quarantine = self.paths.adapter_quarantine_dir().join("definitions");
        create_private_dir(&quarantine)?;
        let target = quarantine.join(digest);
        if target.exists() {
            if !definition_directories_match(&source, &target)? {
                return Err(DefinitionStoreError::Integrity("quarantine_conflict"));
            }
            let duplicate =
                quarantine.join(format!(".duplicate-{digest}-{}", random_stage_name()?));
            fs::rename(&source, &duplicate)?;
            sync_directory(&self.paths.adapter_definitions_dir())?;
            sync_directory(&quarantine)?;
            fs::remove_dir_all(&duplicate)?;
            sync_directory(&quarantine)?;
            return Ok(());
        }
        fs::rename(source, target)?;
        sync_directory(&self.paths.adapter_definitions_dir())?;
        sync_directory(&quarantine)?;
        Ok(())
    }

    /// Load one exact canonical definition for a trusted review surface.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionStoreError`] when the digest, canonical files, or
    /// retained source snapshot fails the same checks used during discovery.
    pub fn load(
        &self,
        semantic_digest: &str,
    ) -> Result<StoredAdapterDefinition, DefinitionStoreError> {
        SemanticDigest::parse(semantic_digest.to_string())
            .map_err(|_| DefinitionStoreError::Integrity("semantic_digest"))?;
        self.prepare_roots()?;
        self.read_stored_definition(
            &self.paths.adapter_definition_dir(semantic_digest)?,
            semantic_digest,
        )
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
        sync_directory(&self.paths.adapter_sources_dir())?;
        Ok(())
    }

    fn read_definition_dir(
        &self,
        path: &Path,
        expected_digest: &str,
    ) -> Result<DefinitionInstall, DefinitionStoreError> {
        let (stored, compiled) = self.read_stored_and_compiled_definition(path, expected_digest)?;
        Ok(DefinitionInstall {
            projection: compiled_projection(&self.paths, &compiled, &stored.provenance),
            compiled,
        })
    }

    fn read_stored_definition(
        &self,
        path: &Path,
        expected_digest: &str,
    ) -> Result<StoredAdapterDefinition, DefinitionStoreError> {
        self.read_stored_and_compiled_definition(path, expected_digest)
            .map(|(stored, _)| stored)
    }

    fn read_stored_and_compiled_definition(
        &self,
        path: &Path,
        expected_digest: &str,
    ) -> Result<(StoredAdapterDefinition, CompiledAdapterDefinition), DefinitionStoreError> {
        require_regular_directory(path)?;
        require_exact_entries(path, &[MANIFEST_FILE, PROVENANCE_FILE])?;
        let manifest_bytes =
            read_bounded_regular_file(&path.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
        let provenance_bytes =
            read_bounded_regular_file(&path.join(PROVENANCE_FILE), MAX_PROVENANCE_BYTES)?;
        let manifest: AdapterManifest = serde_json::from_slice(&manifest_bytes)?;
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
        let source = match (&provenance.source_digest, &provenance.source_extension) {
            (Some(digest), Some(extension)) => {
                let bytes = read_bounded_regular_file(
                    &self.paths.adapter_source_path(digest.as_str(), extension)?,
                    MAX_SOURCE_BYTES as u64,
                )?;
                if SourceDigest::compute(&bytes) != *digest {
                    return Err(DefinitionStoreError::Integrity("source_digest"));
                }
                Some((bytes, extension.clone()))
            }
            (None, None) => None,
            _ => return Err(DefinitionStoreError::Integrity("source_provenance")),
        };
        Ok((
            StoredAdapterDefinition {
                manifest,
                provenance,
                source,
            },
            compiled,
        ))
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

fn definition_directories_match(
    source: &Path,
    target: &Path,
) -> Result<bool, DefinitionStoreError> {
    require_regular_directory(target)?;
    require_exact_entries(source, &[MANIFEST_FILE, PROVENANCE_FILE])?;
    require_exact_entries(target, &[MANIFEST_FILE, PROVENANCE_FILE])?;
    Ok(
        read_bounded_regular_file(&source.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?
            == read_bounded_regular_file(&target.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?
            && read_bounded_regular_file(&source.join(PROVENANCE_FILE), MAX_PROVENANCE_BYTES)?
                == read_bounded_regular_file(&target.join(PROVENANCE_FILE), MAX_PROVENANCE_BYTES)?,
    )
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

fn random_stage_name() -> Result<String, DefinitionStoreError> {
    random_hex(12).map_err(Into::into)
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

#[cfg(test)]
mod tests;
