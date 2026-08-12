//! Filesystem authority for adapter connections and credential generations.

use crate::{
    AdapterConnectionAuthenticationV1, AdapterConnectionV4, AdapterCredentialGenerationV2,
    AdapterCredentialMaterial, AuthenticationMode, CompiledAdapterDefinition, ConnectionSlug,
    DefinitionInstall,
    digest::canonical_json_bytes,
    private_fs::{
        PrivateFsError, create_private_dir, random_hex, read_bounded_regular_file,
        require_directory_no_symlink, require_exact_entries, require_regular_directory,
        sync_directory, write_new_file,
    },
};
use noema_capabilities::{
    CapabilityConnectionPolicy, CapabilityDataSharingPolicy, CapabilityUnsafeActionPolicy,
};
use noema_home::NoemaPaths;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use thiserror::Error;

const CONNECTION_FILE: &str = "connection.json";
const CREDENTIALS_DIR: &str = "credentials";
const STAGING_DIR: &str = ".staging";
const REPLACEMENT_PREFIX: &str = ".connection-replace-";
const MAX_CONNECTION_BYTES: u64 = 128 * 1024;
const MAX_CREDENTIAL_BYTES: u64 = 128 * 1024;
const MAX_CONNECTIONS: usize = 1_024;

/// Canonical connection store rooted in one `NOEMA_HOME`.
#[derive(Debug, Clone)]
pub struct AdapterConnectionStore {
    paths: NoemaPaths,
}

/// Successful filesystem-owned connection discovered during a scan.
#[derive(Debug, Clone)]
pub struct ConnectionInstall {
    /// Exact non-secret canonical descriptor.
    pub descriptor: AdapterConnectionV4,
    /// Rebuildable body-free SQLite projection.
    pub projection: ConnectionProjection,
}

/// Safe filesystem-derived adapter connection index row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProjection {
    /// Stable filesystem connection identity.
    pub connection_id: String,
    /// Stable tool namespace slug, absent for blocked objects.
    pub connection_slug: Option<String>,
    /// Exact definition digest, absent for blocked objects.
    pub semantic_digest: Option<String>,
    /// Human-visible connection label when configured or discovered.
    pub connection_label: Option<String>,
    /// `active`, `suspended`, `authentication_required`, or `blocked`.
    pub status: &'static str,
    /// Descriptor/lifecycle revision.
    pub connection_revision: Option<u64>,
    /// Direct credential revision.
    pub credential_revision: Option<u64>,
    /// Reviewed policy revision.
    pub policy_revision: Option<u64>,
    /// Current direct credential generation identity without its bytes.
    pub credential_generation: Option<String>,
    /// Reusable OAuth grant identity when selected.
    pub grant_id: Option<String>,
    /// Reviewed operation identities.
    pub allowed_operations: Vec<String>,
    /// Safe descriptor path relative to `NOEMA_HOME`.
    pub descriptor_relative_path: String,
    /// Safe secret path reference relative to `NOEMA_HOME`.
    pub credential_relative_path: Option<String>,
    /// Safe blocked-object diagnostic category.
    pub diagnostic_code: Option<&'static str>,
}

/// Deterministic connection scan with invalid objects isolated as diagnostics.
#[derive(Debug)]
pub struct ConnectionScan {
    /// Valid connection descriptors.
    pub connections: Vec<ConnectionInstall>,
    /// Invalid objects that did not abort unrelated discovery.
    pub diagnostics: Vec<ConnectionScanDiagnostic>,
}

impl ConnectionScan {
    /// Return every filesystem-owned projection in connection-id order.
    #[must_use]
    pub fn projections(&self) -> Vec<ConnectionProjection> {
        let mut rows = self
            .connections
            .iter()
            .map(|connection| connection.projection.clone())
            .chain(
                self.diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.projection.clone()),
            )
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.connection_id.cmp(&right.connection_id));
        rows
    }
}

/// Safe diagnostic for one invalid connection directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionScanDiagnostic {
    /// Stable non-secret failure category.
    pub code: &'static str,
    /// Blocked projection when the directory has a valid connection identity.
    pub projection: Option<ConnectionProjection>,
}

/// Connection filesystem failure with no secret-bearing display fields.
#[derive(Debug, Error)]
pub enum ConnectionStoreError {
    /// Filesystem operation failed.
    #[error("adapter connection filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// A derived path component was invalid.
    #[error("adapter connection path is invalid: {0}")]
    Path(#[from] noema_home::NoemaPathError),
    /// Canonical JSON could not be parsed or encoded.
    #[error("adapter connection JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Filesystem content violated a stable connection invariant.
    #[error("adapter connection invariant failed: {0}")]
    Integrity(&'static str),
}

impl From<PrivateFsError> for ConnectionStoreError {
    fn from(error: PrivateFsError) -> Self {
        match error {
            PrivateFsError::Io(error) => Self::Io(error),
            PrivateFsError::Integrity(code) => Self::Integrity(code),
        }
    }
}

impl AdapterConnectionStore {
    /// Quarantine connection descriptors from the replaced ownership model.
    pub(crate) fn quarantine_legacy_descriptors(
        &self,
    ) -> Result<BTreeSet<String>, ConnectionStoreError> {
        self.prepare_roots()?;
        let mut quarantined = BTreeSet::new();
        let mut entries =
            fs::read_dir(self.paths.adapter_connections_dir())?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Some(connection_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !valid_hex_id(&connection_id) {
                continue;
            }
            let target = entry.path();
            let bytes =
                read_bounded_regular_file(&target.join(CONNECTION_FILE), MAX_CONNECTION_BYTES)?;
            let descriptor: serde_json::Value = serde_json::from_slice(&bytes)?;
            if canonical_json_bytes(&descriptor)? != bytes
                || descriptor
                    .get("connection_id")
                    .and_then(serde_json::Value::as_str)
                    != Some(connection_id.as_str())
            {
                return Err(ConnectionStoreError::Integrity("connection_identity"));
            }
            let schema_version = descriptor
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
                .ok_or(ConnectionStoreError::Integrity("connection_schema"))?;
            if schema_version == 4 {
                continue;
            }
            self.quarantine(&connection_id)?;
            quarantined.insert(connection_id);
        }
        Ok(quarantined)
    }

    /// Create a filesystem store handle. Directories are created lazily.
    #[must_use]
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    /// Remove abandoned connection-install staging directories before startup
    /// discovery. Staging is never authoritative, so a crash cannot publish a
    /// partial descriptor or leave its extracted secret indefinitely.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for unsafe staging metadata, boundedness
    /// failures, or a failed cleanup/sync operation.
    pub fn recover(&self) -> Result<(), ConnectionStoreError> {
        self.prepare_roots()?;
        self.recover_staging()?;
        self.recover_credential_replacements()
    }

    fn recover_staging(&self) -> Result<(), ConnectionStoreError> {
        let staging_root = self.paths.adapter_connections_dir().join(STAGING_DIR);
        let metadata = match fs::symlink_metadata(&staging_root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ConnectionStoreError::Integrity("staging_directory"));
        }
        let entries = fs::read_dir(&staging_root)?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() > 64 {
            return Err(ConnectionStoreError::Integrity("staging_oversized"));
        }
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || metadata.is_file() {
                fs::remove_file(path)?;
            } else if metadata.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                return Err(ConnectionStoreError::Integrity("staging_entry"));
            }
        }
        sync_directory(&staging_root)?;
        sync_directory(&self.paths.adapter_connections_dir())?;
        Ok(())
    }

    fn recover_credential_replacements(&self) -> Result<(), ConnectionStoreError> {
        let mut entries =
            fs::read_dir(self.paths.adapter_connections_dir())?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() > MAX_CONNECTIONS + 1 {
            return Err(ConnectionStoreError::Integrity("connections_oversized"));
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Some(connection_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !valid_hex_id(&connection_id) {
                continue;
            }
            let path = entry.path();
            let Ok(descriptor) = Self::read_canonical_descriptor(&path, &connection_id) else {
                continue;
            };
            let credentials = path.join(CREDENTIALS_DIR);
            if require_regular_directory(&credentials).is_err() {
                continue;
            }
            let credential_entries = fs::read_dir(&credentials)?.collect::<Result<Vec<_>, _>>()?;
            if credential_entries.len() > 64 {
                continue;
            }
            let mut changed = false;
            for credential_entry in credential_entries {
                let Ok(name) = credential_entry.file_name().into_string() else {
                    continue;
                };
                let is_replacement = name
                    .strip_prefix(REPLACEMENT_PREFIX)
                    .is_some_and(valid_hex_id);
                let unreferenced_generation = name
                    .strip_suffix(".json")
                    .filter(|generation| valid_hex_id(generation))
                    .is_some_and(|generation| {
                        direct_credential_generation(&descriptor) != Some(generation)
                    });
                if (is_replacement || unreferenced_generation)
                    && read_bounded_regular_file(
                        &credential_entry.path(),
                        if is_replacement {
                            MAX_CONNECTION_BYTES
                        } else {
                            MAX_CREDENTIAL_BYTES
                        },
                    )
                    .is_ok()
                {
                    fs::remove_file(credential_entry.path())?;
                    changed = true;
                }
            }
            if changed {
                sync_directory(&credentials)?;
            }
        }
        Ok(())
    }

    /// Atomically install one new connection and optional immutable credential.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for invalid authority, conflicting immutable
    /// bytes, unsafe filesystem state, or failed durable publication.
    pub fn install(
        &self,
        descriptor: &AdapterConnectionV4,
        credential: Option<&AdapterCredentialGenerationV2>,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        validate_connection(descriptor, credential, definition)?;
        let descriptor_bytes = canonical_json_bytes(&serde_json::to_value(descriptor)?)?;
        if descriptor_bytes.len() as u64 > MAX_CONNECTION_BYTES {
            return Err(ConnectionStoreError::Integrity("connection_oversized"));
        }
        let credential_bytes = credential
            .map(|credential| canonical_json_bytes(&serde_json::to_value(credential)?))
            .transpose()?;
        if credential_bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() as u64 > MAX_CREDENTIAL_BYTES)
        {
            return Err(ConnectionStoreError::Integrity("credential_oversized"));
        }
        self.prepare_roots()?;
        let target = self
            .paths
            .adapter_connection_dir(&descriptor.connection_id)?;
        if target.exists() {
            let installed =
                self.read_connection_dir(&target, &descriptor.connection_id, definition)?;
            if installed.descriptor != *descriptor {
                return Err(ConnectionStoreError::Integrity("connection_conflict"));
            }
            if let (Some(generation), Some(expected)) = (
                direct_credential_generation(descriptor),
                credential_bytes.as_ref(),
            ) {
                let existing = read_bounded_regular_file(
                    &credential_path(&target.join(CREDENTIALS_DIR), generation)?,
                    MAX_CREDENTIAL_BYTES,
                )?;
                if existing != *expected {
                    return Err(ConnectionStoreError::Integrity("credential_conflict"));
                }
            }
            return Ok(installed);
        }
        let staging_root = self.paths.adapter_connections_dir().join(STAGING_DIR);
        create_private_dir(&staging_root)?;
        let staging = staging_root.join(random_hex(12)?);
        create_private_dir(&staging)?;
        let result = (|| {
            write_new_file(&staging.join(CONNECTION_FILE), &descriptor_bytes)?;
            let credentials = staging.join(CREDENTIALS_DIR);
            create_private_dir(&credentials)?;
            if let (Some(credential), Some(bytes)) = (credential, credential_bytes.as_ref()) {
                write_new_file(
                    &credential_path(&credentials, &credential.generation_id)?,
                    bytes,
                )?;
            }
            sync_directory(&credentials)?;
            sync_directory(&staging)?;
            fs::rename(&staging, &target)?;
            sync_directory(&self.paths.adapter_connections_dir())?;
            self.read_connection_dir(&target, &descriptor.connection_id, definition)
        })();
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    /// Atomically replace only non-secret management state under an exact
    /// descriptor fence while preserving the current credential generation.
    pub(crate) fn replace_management_descriptor(
        &self,
        expected: &AdapterConnectionV4,
        replacement: &AdapterConnectionV4,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_connection_dir(&expected.connection_id)?;
        let (current, credential) = Self::read_descriptor(&target, &expected.connection_id)?;
        if current != *expected
            || replacement.connection_id != current.connection_id
            || replacement.semantic_digest != current.semantic_digest
            || replacement.authentication != current.authentication
        {
            return Err(ConnectionStoreError::Integrity("management_transition"));
        }
        validate_connection(replacement, credential.as_ref(), definition)?;
        let bytes = canonical_json_bytes(&serde_json::to_value(replacement)?)?;
        if bytes.len() as u64 > MAX_CONNECTION_BYTES {
            return Err(ConnectionStoreError::Integrity("connection_oversized"));
        }
        let credentials = target.join(CREDENTIALS_DIR);
        let temporary = credentials.join(format!("{REPLACEMENT_PREFIX}{}", random_hex(16)?));
        write_new_file(&temporary, &bytes)?;
        sync_directory(&credentials)?;
        if let Err(error) = fs::rename(&temporary, target.join(CONNECTION_FILE)) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        sync_directory(&target)?;
        self.read_connection_dir(&target, &replacement.connection_id, definition)
    }

    /// Move one connection to a reviewed revision while preserving valid policy.
    pub(crate) fn rebind_definition_descriptor(
        &self,
        expected: &AdapterConnectionV4,
        current_definition: &CompiledAdapterDefinition,
        replacement_definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_connection_dir(&expected.connection_id)?;
        let (current, credential) = Self::read_descriptor(&target, &expected.connection_id)?;
        validate_connection(&current, credential.as_ref(), current_definition)?;
        let mut permitted = current.clone();
        permitted.semantic_digest = replacement_definition.semantic_digest.to_string();
        permitted.connection_revision = permitted
            .connection_revision
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("connection_revision"))?;
        permitted.allowed_operations.retain(|operation_id| {
            replacement_definition
                .operations
                .iter()
                .any(|operation| operation.operation_id == *operation_id)
        });
        let previous_override_count = permitted.tool_overrides.len();
        permitted.tool_overrides.retain_mut(|policy| {
            let Some(current_operation) = current_definition
                .operations
                .iter()
                .find(|operation| operation.operation_id == policy.tool_id)
            else {
                return false;
            };
            let Some(replacement_operation) = replacement_definition
                .operations
                .iter()
                .find(|operation| operation.operation_id == policy.tool_id)
            else {
                return false;
            };
            if policy.source_revision != current_operation.operation_digest.as_str()
                || !same_operation_contract(current_operation, replacement_operation)
            {
                return false;
            }
            policy.source_revision = replacement_operation.operation_digest.to_string();
            true
        });
        if previous_override_count != permitted.tool_overrides.len()
            || !permitted.tool_overrides.is_empty()
        {
            permitted.policy_revision = permitted
                .policy_revision
                .checked_add(1)
                .ok_or(ConnectionStoreError::Integrity("policy_revision"))?;
            if let Some(policy) = permitted.policy.as_mut() {
                policy.revision = permitted.policy_revision;
            }
        }
        if current != *expected {
            return Err(ConnectionStoreError::Integrity("definition_transition"));
        }
        validate_connection(&permitted, credential.as_ref(), replacement_definition)?;
        let bytes = canonical_json_bytes(&serde_json::to_value(&permitted)?)?;
        if bytes.len() as u64 > MAX_CONNECTION_BYTES {
            return Err(ConnectionStoreError::Integrity("connection_oversized"));
        }
        let credentials = target.join(CREDENTIALS_DIR);
        let temporary = credentials.join(format!("{REPLACEMENT_PREFIX}{}", random_hex(16)?));
        write_new_file(&temporary, &bytes)?;
        sync_directory(&credentials)?;
        if let Err(error) = fs::rename(&temporary, target.join(CONNECTION_FILE)) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        sync_directory(&target)?;
        self.read_connection_dir(&target, &permitted.connection_id, replacement_definition)
    }

    /// Stop one connection until a breaking authentication revision is completed.
    pub(crate) fn require_new_authentication(
        &self,
        expected: &AdapterConnectionV4,
        current_definition: &CompiledAdapterDefinition,
        replacement_definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_connection_dir(&expected.connection_id)?;
        let (current, credential) = Self::read_descriptor(&target, &expected.connection_id)?;
        validate_connection(&current, credential.as_ref(), current_definition)?;
        let mut replacement = expected.clone();
        replacement.semantic_digest = replacement_definition.semantic_digest.to_string();
        replacement.status = crate::AdapterConnectionStatus::AuthenticationRequired;
        replacement.authentication = AdapterConnectionAuthenticationV1::Pending;
        replacement.allowed_operations.clear();
        replacement.tool_overrides.clear();
        replacement.connection_revision = replacement
            .connection_revision
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("connection_revision"))?;
        replacement.policy_revision = replacement
            .policy_revision
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("policy_revision"))?;
        if let Some(policy) = replacement.policy.as_mut() {
            policy.revision = replacement.policy_revision;
        }
        if current != *expected {
            return Err(ConnectionStoreError::Integrity("definition_transition"));
        }
        validate_connection(&replacement, credential.as_ref(), replacement_definition)?;
        let bytes = canonical_json_bytes(&serde_json::to_value(&replacement)?)?;
        atomic_replace_connection(&target, &bytes)?;
        self.read_connection_dir(&target, &replacement.connection_id, replacement_definition)
    }

    /// Merge two OAuth connections already bound to one reviewed definition.
    pub(crate) fn merge_oauth_connections(
        &self,
        survivor_id: &str,
        redundant_id: &str,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let survivor_path = self.paths.adapter_connection_dir(survivor_id)?;
        let redundant_path = self.paths.adapter_connection_dir(redundant_id)?;
        let (mut survivor, survivor_credential) =
            Self::read_descriptor(&survivor_path, survivor_id)?;
        let (redundant, redundant_credential) =
            Self::read_descriptor(&redundant_path, redundant_id)?;
        validate_connection(&survivor, survivor_credential.as_ref(), definition)?;
        validate_connection(&redundant, redundant_credential.as_ref(), definition)?;
        let same_grant = matches!(
            (&survivor.authentication, &redundant.authentication),
            (
                AdapterConnectionAuthenticationV1::OauthGrant { grant_id: left },
                AdapterConnectionAuthenticationV1::OauthGrant { grant_id: right }
            ) if left == right
        );
        if !same_grant {
            return Err(ConnectionStoreError::Integrity("connection_merge_binding"));
        }
        survivor
            .allowed_operations
            .extend(redundant.allowed_operations.iter().cloned());
        survivor.allowed_operations.sort();
        survivor.allowed_operations.dedup();
        survivor.policy = stricter_policy(survivor.policy, redundant.policy);
        survivor.status = safer_status(survivor.status, redundant.status);
        survivor.tool_overrides.retain(|policy| {
            redundant
                .tool_overrides
                .iter()
                .find(|candidate| candidate.tool_id == policy.tool_id)
                .is_none_or(|candidate| candidate == policy)
        });
        for policy in redundant.tool_overrides {
            if !survivor
                .tool_overrides
                .iter()
                .any(|candidate| candidate.tool_id == policy.tool_id)
            {
                survivor.tool_overrides.push(policy);
            }
        }
        survivor
            .tool_overrides
            .sort_by(|left, right| left.tool_id.cmp(&right.tool_id));
        survivor.connection_revision = survivor
            .connection_revision
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("connection_revision"))?;
        survivor.policy_revision = survivor
            .policy_revision
            .max(redundant.policy_revision)
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("policy_revision"))?;
        if let Some(policy) = survivor.policy.as_mut() {
            policy.revision = survivor.policy_revision;
        }
        validate_connection(&survivor, survivor_credential.as_ref(), definition)?;
        let bytes = canonical_json_bytes(&serde_json::to_value(&survivor)?)?;
        atomic_replace_connection(&survivor_path, &bytes)?;
        self.read_connection_dir(&survivor_path, survivor_id, definition)
    }

    /// Replace authentication on one suspended connection without changing its identity.
    pub(crate) fn replace_authentication(
        &self,
        expected: &AdapterConnectionV4,
        authentication: AdapterConnectionAuthenticationV1,
        credential: Option<&AdapterCredentialGenerationV2>,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_connection_dir(&expected.connection_id)?;
        let (current, _) = Self::read_descriptor(&target, &expected.connection_id)?;
        if current != *expected
            || current.status != crate::AdapterConnectionStatus::AuthenticationRequired
            || current.semantic_digest != definition.semantic_digest.as_str()
        {
            return Err(ConnectionStoreError::Integrity("authentication_transition"));
        }
        let mut replacement = current;
        replacement.authentication = authentication;
        replacement.status = crate::AdapterConnectionStatus::Active;
        replacement.allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect();
        replacement.allowed_operations.sort();
        replacement.connection_revision = replacement
            .connection_revision
            .checked_add(1)
            .ok_or(ConnectionStoreError::Integrity("connection_revision"))?;
        validate_connection(&replacement, credential, definition)?;
        if let Some(credential) = credential {
            let credential_bytes = canonical_json_bytes(&serde_json::to_value(credential)?)?;
            if credential_bytes.len() as u64 > MAX_CREDENTIAL_BYTES {
                return Err(ConnectionStoreError::Integrity("credential_oversized"));
            }
            write_new_file(
                &credential_path(&target.join(CREDENTIALS_DIR), &credential.generation_id)?,
                &credential_bytes,
            )?;
            sync_directory(&target.join(CREDENTIALS_DIR))?;
        }
        let bytes = canonical_json_bytes(&serde_json::to_value(&replacement)?)?;
        atomic_replace_connection(&target, &bytes)?;
        self.read_connection_dir(&target, &replacement.connection_id, definition)
    }

    /// Scan active connection objects against the exact compiled definitions.
    ///
    /// # Errors
    ///
    /// Returns a redacted error if the authoritative root cannot be read.
    pub fn scan(
        &self,
        definitions: &[DefinitionInstall],
    ) -> Result<ConnectionScan, ConnectionStoreError> {
        self.prepare_roots()?;
        if definitions.len() > MAX_CONNECTIONS {
            return Err(ConnectionStoreError::Integrity("definitions_oversized"));
        }
        let by_digest = definitions
            .iter()
            .map(|definition| {
                (
                    definition.compiled.semantic_digest.as_str(),
                    &definition.compiled,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut entries =
            fs::read_dir(self.paths.adapter_connections_dir())?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() > MAX_CONNECTIONS {
            return Err(ConnectionStoreError::Integrity("connections_oversized"));
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut connections = Vec::new();
        let mut diagnostics = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                diagnostics.push(ConnectionScanDiagnostic {
                    code: "connection_name",
                    projection: None,
                });
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if !valid_hex_id(name) {
                diagnostics.push(ConnectionScanDiagnostic {
                    code: "connection_name",
                    projection: None,
                });
                continue;
            }
            match Self::read_descriptor(&entry.path(), name) {
                Ok((descriptor, credential)) => {
                    let Some(definition) = by_digest.get(descriptor.semantic_digest.as_str())
                    else {
                        diagnostics.push(ConnectionScanDiagnostic {
                            code: "definition_unavailable",
                            projection: Some(blocked_projection(
                                &self.paths,
                                name,
                                "definition_unavailable",
                            )),
                        });
                        continue;
                    };
                    match validate_connection(&descriptor, credential.as_ref(), definition) {
                        Ok(()) => connections.push(ConnectionInstall {
                            projection: projection(&self.paths, &descriptor),
                            descriptor,
                        }),
                        Err(error) => diagnostics.push(ConnectionScanDiagnostic {
                            code: diagnostic_code(&error),
                            projection: Some(blocked_projection(
                                &self.paths,
                                name,
                                diagnostic_code(&error),
                            )),
                        }),
                    }
                }
                Err(error) => diagnostics.push(ConnectionScanDiagnostic {
                    code: diagnostic_code(&error),
                    projection: Some(blocked_projection(
                        &self.paths,
                        name,
                        diagnostic_code(&error),
                    )),
                }),
            }
        }
        Ok(ConnectionScan {
            connections,
            diagnostics,
        })
    }

    /// Move a connection out of the discovery root before later cleanup.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for an invalid id, conflicting quarantine
    /// object, unsafe filesystem state, or failed durable rename.
    pub fn quarantine(&self, connection_id: &str) -> Result<(), ConnectionStoreError> {
        self.prepare_roots()?;
        let source = self.paths.adapter_connection_dir(connection_id)?;
        let target = self
            .paths
            .quarantined_adapter_connection_dir(connection_id)?;
        if !source.exists() {
            return Ok(());
        }
        if target.exists() {
            return Err(ConnectionStoreError::Integrity("quarantine_conflict"));
        }
        fs::rename(source, target)?;
        sync_directory(&self.paths.adapter_connections_dir())?;
        sync_directory(&self.paths.adapter_connection_quarantine_dir())?;
        Ok(())
    }

    /// Quarantine every connection bound to a legacy definition digest.
    pub(crate) fn quarantine_referencing(
        &self,
        digests: &BTreeSet<String>,
    ) -> Result<(), ConnectionStoreError> {
        self.prepare_roots()?;
        let mut entries =
            fs::read_dir(self.paths.adapter_connections_dir())?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Some(connection_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if connection_id.starts_with('.') || !valid_hex_id(&connection_id) {
                continue;
            }
            let descriptor = Self::read_canonical_descriptor(&entry.path(), &connection_id)?;
            if digests.contains(&descriptor.semantic_digest) {
                self.quarantine(&connection_id)?;
            }
        }
        Ok(())
    }

    pub(crate) fn load_for_invocation(
        &self,
        connection_id: &str,
        definition: &CompiledAdapterDefinition,
    ) -> Result<(AdapterConnectionV4, Option<AdapterCredentialGenerationV2>), ConnectionStoreError>
    {
        self.prepare_roots()?;
        let path = self.paths.adapter_connection_dir(connection_id)?;
        let (descriptor, credential) = Self::read_descriptor(&path, connection_id)?;
        validate_connection(&descriptor, credential.as_ref(), definition)?;
        Ok((descriptor, credential))
    }

    fn prepare_roots(&self) -> Result<(), ConnectionStoreError> {
        require_directory_no_symlink(self.paths.root())?;
        create_private_dir(&self.paths.adapters_dir())?;
        create_private_dir(&self.paths.adapter_connections_dir())?;
        create_private_dir(&self.paths.adapter_quarantine_dir())?;
        create_private_dir(&self.paths.adapter_connection_quarantine_dir())?;
        Ok(())
    }

    fn read_connection_dir(
        &self,
        path: &Path,
        expected_id: &str,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        let (descriptor, credential) = Self::read_descriptor(path, expected_id)?;
        validate_connection(&descriptor, credential.as_ref(), definition)?;
        Ok(ConnectionInstall {
            projection: projection(&self.paths, &descriptor),
            descriptor,
        })
    }

    fn read_descriptor(
        path: &Path,
        expected_id: &str,
    ) -> Result<(AdapterConnectionV4, Option<AdapterCredentialGenerationV2>), ConnectionStoreError>
    {
        let descriptor = Self::read_canonical_descriptor(path, expected_id)?;
        let credentials = path.join(CREDENTIALS_DIR);
        require_regular_directory(&credentials)?;
        let credentials_by_generation = read_credential_generations(&credentials)?;
        let credential = match direct_credential_generation(&descriptor) {
            Some(generation) => {
                // A descriptor replacement is published before its previous
                // immutable generation is removed. If cleanup is interrupted,
                // the descriptor still selects the sole authority and startup
                // recovery removes the one unreferenced predecessor.
                if credentials_by_generation.is_empty() || credentials_by_generation.len() > 2 {
                    return Err(ConnectionStoreError::Integrity("credential_unreferenced"));
                }
                Some(
                    credentials_by_generation
                        .into_iter()
                        .find(|credential| credential.generation_id == generation)
                        .ok_or(ConnectionStoreError::Integrity("credential_missing"))?,
                )
            }
            None => {
                if !credentials_by_generation.is_empty() {
                    return Err(ConnectionStoreError::Integrity("credential_unreferenced"));
                }
                None
            }
        };
        Ok((descriptor, credential))
    }

    fn read_canonical_descriptor(
        path: &Path,
        expected_id: &str,
    ) -> Result<AdapterConnectionV4, ConnectionStoreError> {
        require_regular_directory(path)?;
        require_exact_entries(path, &[CONNECTION_FILE, CREDENTIALS_DIR])?;
        let bytes = read_bounded_regular_file(&path.join(CONNECTION_FILE), MAX_CONNECTION_BYTES)?;
        let descriptor: AdapterConnectionV4 = serde_json::from_slice(&bytes)?;
        if descriptor.connection_id != expected_id
            || canonical_json_bytes(&serde_json::to_value(&descriptor)?)? != bytes
        {
            return Err(ConnectionStoreError::Integrity("connection_identity"));
        }
        Ok(descriptor)
    }
}

fn validate_connection(
    descriptor: &AdapterConnectionV4,
    credential: Option<&AdapterCredentialGenerationV2>,
    definition: &CompiledAdapterDefinition,
) -> Result<(), ConnectionStoreError> {
    if descriptor.schema_version != 4
        || !valid_hex_id(&descriptor.connection_id)
        || ConnectionSlug::new(descriptor.connection_slug.clone()).is_err()
        || descriptor.semantic_digest != definition.semantic_digest.as_str()
        || !definition.reviewed
        || descriptor
            .connection_label
            .as_deref()
            .is_some_and(|value| !valid_label(value))
        || descriptor.connection_revision == 0
        || descriptor.policy_revision == 0
        || !sorted_unique_components(&descriptor.allowed_operations)
        || descriptor
            .policy
            .is_some_and(|policy| policy.revision != descriptor.policy_revision)
        || descriptor.tool_overrides.iter().any(|policy| {
            definition
                .operations
                .iter()
                .find(|operation| operation.operation_id == policy.tool_id)
                .is_none_or(|operation| {
                    operation.operation_digest.as_str() != policy.source_revision
                })
        })
        || descriptor.allowed_operations.iter().any(|operation| {
            !definition
                .operations
                .iter()
                .any(|candidate| candidate.operation_id == *operation)
        })
    {
        return Err(ConnectionStoreError::Integrity("connection_policy"));
    }
    if descriptor.status == crate::AdapterConnectionStatus::AuthenticationRequired {
        return if authentication_reference_valid(&descriptor.authentication) {
            Ok(())
        } else {
            Err(ConnectionStoreError::Integrity("credential_binding"))
        };
    }
    match (
        &descriptor.authentication,
        definition.authentication.mode(),
        credential,
    ) {
        (AdapterConnectionAuthenticationV1::None, AuthenticationMode::None, None) => {}
        (
            AdapterConnectionAuthenticationV1::Credential {
                generation_id,
                revision,
            },
            AuthenticationMode::Credential,
            Some(credential),
        ) if *revision > 0 && credential_matches(generation_id, credential, definition) => {}
        (
            AdapterConnectionAuthenticationV1::OauthGrant { grant_id },
            AuthenticationMode::Oauth2AuthorizationCodePkce,
            None,
        ) if valid_hex_id(grant_id) => {}
        _ => return Err(ConnectionStoreError::Integrity("credential_binding")),
    }
    Ok(())
}

fn authentication_reference_valid(authentication: &AdapterConnectionAuthenticationV1) -> bool {
    match authentication {
        AdapterConnectionAuthenticationV1::Pending | AdapterConnectionAuthenticationV1::None => {
            true
        }
        AdapterConnectionAuthenticationV1::Credential {
            generation_id,
            revision,
        } => *revision > 0 && valid_hex_id(generation_id),
        AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => valid_hex_id(grant_id),
    }
}

fn same_operation_contract(
    current: &crate::CompiledOperation,
    replacement: &crate::CompiledOperation,
) -> bool {
    let mut current_arguments = current.arguments.clone();
    let mut replacement_arguments = replacement.arguments.clone();
    for argument in &mut current_arguments {
        argument.description.clear();
    }
    for argument in &mut replacement_arguments {
        argument.description.clear();
    }
    current.operation_id == replacement.operation_id
        && current.method == replacement.method
        && current.path == replacement.path
        && current.authorization == replacement.authorization
        && current.fixed_headers == replacement.fixed_headers
        && current.fixed_query == replacement.fixed_query
        && current_arguments == replacement_arguments
        && current.json_body_template == replacement.json_body_template
        && current.behavior == replacement.behavior
        && current.retry == replacement.retry
        && current.pagination == replacement.pagination
        && current.response == replacement.response
}

fn credential_matches(
    generation: &str,
    credential: &AdapterCredentialGenerationV2,
    definition: &CompiledAdapterDefinition,
) -> bool {
    credential.schema_version == 2
        && credential.generation_id == generation
        && valid_hex_id(generation)
        && match &credential.material {
            AdapterCredentialMaterial::Credential { fields } => definition
                .authentication
                .credential()
                .is_some_and(|config| credential_fields_match(fields, config.setup.input.fields())),
        }
}

fn credential_fields_match(
    values: &std::collections::BTreeMap<String, String>,
    fields: &[crate::CredentialField],
) -> bool {
    values.len() == fields.len()
        && fields.iter().all(|field| {
            values
                .get(&field.id)
                .is_some_and(|value| valid_secret(value))
        })
}

fn read_credential_generations(
    path: &Path,
) -> Result<Vec<AdapterCredentialGenerationV2>, ConnectionStoreError> {
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    if entries.len() > 64 {
        return Err(ConnectionStoreError::Integrity("credentials_oversized"));
    }
    entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ConnectionStoreError::Integrity("credential_name"))?;
            let generation = name
                .strip_suffix(".json")
                .filter(|generation| valid_hex_id(generation))
                .ok_or(ConnectionStoreError::Integrity("credential_name"))?;
            let bytes = read_bounded_regular_file(&entry.path(), MAX_CREDENTIAL_BYTES)?;
            let credential: AdapterCredentialGenerationV2 = serde_json::from_slice(&bytes)?;
            if credential.generation_id != generation
                || canonical_json_bytes(&serde_json::to_value(&credential)?)? != bytes
            {
                return Err(ConnectionStoreError::Integrity("credential_canonical"));
            }
            Ok(credential)
        })
        .collect()
}

fn credential_path(
    root: &Path,
    generation: &str,
) -> Result<std::path::PathBuf, ConnectionStoreError> {
    if !valid_hex_id(generation) {
        return Err(ConnectionStoreError::Integrity("credential_generation"));
    }
    Ok(root.join(format!("{generation}.json")))
}

fn direct_credential_generation(descriptor: &AdapterConnectionV4) -> Option<&str> {
    match &descriptor.authentication {
        AdapterConnectionAuthenticationV1::Credential { generation_id, .. } => Some(generation_id),
        AdapterConnectionAuthenticationV1::Pending
        | AdapterConnectionAuthenticationV1::None
        | AdapterConnectionAuthenticationV1::OauthGrant { .. } => None,
    }
}

fn direct_credential_revision(descriptor: &AdapterConnectionV4) -> Option<u64> {
    match descriptor.authentication {
        AdapterConnectionAuthenticationV1::Credential { revision, .. } => Some(revision),
        AdapterConnectionAuthenticationV1::Pending
        | AdapterConnectionAuthenticationV1::None
        | AdapterConnectionAuthenticationV1::OauthGrant { .. } => None,
    }
}

fn oauth_grant_id(descriptor: &AdapterConnectionV4) -> Option<&str> {
    match &descriptor.authentication {
        AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => Some(grant_id),
        AdapterConnectionAuthenticationV1::Pending
        | AdapterConnectionAuthenticationV1::None
        | AdapterConnectionAuthenticationV1::Credential { .. } => None,
    }
}

fn projection(paths: &NoemaPaths, descriptor: &AdapterConnectionV4) -> ConnectionProjection {
    let credential_relative_path = direct_credential_generation(descriptor).map(|generation| {
        format!(
            "adapters/connections/{}/credentials/{generation}.json",
            descriptor.connection_id
        )
    });
    ConnectionProjection {
        connection_id: descriptor.connection_id.clone(),
        connection_slug: Some(descriptor.connection_slug.clone()),
        semantic_digest: Some(descriptor.semantic_digest.clone()),
        connection_label: descriptor.connection_label.clone(),
        status: descriptor.status.as_str(),
        connection_revision: Some(descriptor.connection_revision),
        credential_revision: direct_credential_revision(descriptor),
        policy_revision: Some(descriptor.policy_revision),
        credential_generation: direct_credential_generation(descriptor).map(str::to_string),
        grant_id: oauth_grant_id(descriptor).map(str::to_string),
        allowed_operations: descriptor.allowed_operations.clone(),
        descriptor_relative_path: relative_descriptor_path(paths, &descriptor.connection_id),
        credential_relative_path,
        diagnostic_code: None,
    }
}

fn blocked_projection(
    paths: &NoemaPaths,
    connection_id: &str,
    code: &'static str,
) -> ConnectionProjection {
    ConnectionProjection {
        connection_id: connection_id.to_string(),
        connection_slug: None,
        semantic_digest: None,
        connection_label: None,
        status: "blocked",
        connection_revision: None,
        credential_revision: None,
        policy_revision: None,
        credential_generation: None,
        grant_id: None,
        allowed_operations: Vec::new(),
        descriptor_relative_path: relative_descriptor_path(paths, connection_id),
        credential_relative_path: None,
        diagnostic_code: Some(code),
    }
}

fn relative_descriptor_path(paths: &NoemaPaths, connection_id: &str) -> String {
    paths
        .adapter_connection_dir(connection_id)
        .expect("validated id")
        .join(CONNECTION_FILE)
        .strip_prefix(paths.root())
        .expect("adapter path")
        .to_string_lossy()
        .into_owned()
}

fn valid_hex_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_component(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16 * 1024
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn sorted_unique_components(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| valid_component(value, 256))
}

fn atomic_replace_connection(path: &Path, bytes: &[u8]) -> Result<(), ConnectionStoreError> {
    if bytes.len() as u64 > MAX_CONNECTION_BYTES {
        return Err(ConnectionStoreError::Integrity("connection_oversized"));
    }
    let credentials = path.join(CREDENTIALS_DIR);
    let temporary = credentials.join(format!("{REPLACEMENT_PREFIX}{}", random_hex(16)?));
    write_new_file(&temporary, bytes)?;
    sync_directory(&credentials)?;
    if let Err(error) = fs::rename(&temporary, path.join(CONNECTION_FILE)) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    sync_directory(path)?;
    Ok(())
}

fn stricter_policy(
    left: Option<CapabilityConnectionPolicy>,
    right: Option<CapabilityConnectionPolicy>,
) -> Option<CapabilityConnectionPolicy> {
    match (left, right) {
        (None, policy) | (policy, None) => policy,
        (Some(left), Some(right)) => Some(CapabilityConnectionPolicy {
            data_sharing: if matches!(
                (left.data_sharing, right.data_sharing),
                (
                    CapabilityDataSharingPolicy::ReviewEveryCall,
                    CapabilityDataSharingPolicy::ReviewEveryCall
                        | CapabilityDataSharingPolicy::AllowAutomatically
                ) | (
                    CapabilityDataSharingPolicy::AllowAutomatically,
                    CapabilityDataSharingPolicy::ReviewEveryCall
                )
            ) {
                CapabilityDataSharingPolicy::ReviewEveryCall
            } else {
                CapabilityDataSharingPolicy::AllowAutomatically
            },
            unsafe_actions: match (left.unsafe_actions, right.unsafe_actions) {
                (CapabilityUnsafeActionPolicy::AlwaysAsk, _)
                | (_, CapabilityUnsafeActionPolicy::AlwaysAsk) => {
                    CapabilityUnsafeActionPolicy::AlwaysAsk
                }
                (CapabilityUnsafeActionPolicy::ReviewerMayApprove, _)
                | (_, CapabilityUnsafeActionPolicy::ReviewerMayApprove) => {
                    CapabilityUnsafeActionPolicy::ReviewerMayApprove
                }
                _ => CapabilityUnsafeActionPolicy::NeverAsk,
            },
            revision: left.revision.max(right.revision),
        }),
    }
}

const fn safer_status(
    left: crate::AdapterConnectionStatus,
    right: crate::AdapterConnectionStatus,
) -> crate::AdapterConnectionStatus {
    use crate::AdapterConnectionStatus::{Active, AuthenticationRequired, Suspended};
    match (left, right) {
        (AuthenticationRequired, _) | (_, AuthenticationRequired) => AuthenticationRequired,
        (Suspended, _) | (_, Suspended) => Suspended,
        (Active, Active) => Active,
    }
}

fn diagnostic_code(error: &ConnectionStoreError) -> &'static str {
    match error {
        ConnectionStoreError::Io(_) => "filesystem",
        ConnectionStoreError::Path(_) => "path",
        ConnectionStoreError::Json(_) => "json",
        ConnectionStoreError::Integrity(code) => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterCompiler, AdapterConnectionStatus, AdapterManifest};
    use serde_json::json;

    #[test]
    fn oauth_merge_preserves_union_and_stricter_policy() {
        let directory = tempfile::tempdir().expect("temporary home");
        let paths = NoemaPaths::from_noema_home(directory.path()).expect("Noema paths");
        let manifest: AdapterManifest = serde_json::from_value(json!({
            "schema_version": 9,
            "definition_id": "definition:merge_fixture",
            "adapter_id": "merge_fixture",
            "display_name": "Merge fixture",
            "definition_revision": "v1",
            "reviewed": true,
            "origin": "https://api.example.test/",
            "authentication": {
                "kind": "oauth2_authorization_code_pkce",
                "profile_digest": crate::reviewed_google_oauth_profile_digest()
            },
            "operations": [
                {
                    "operation_id": "get_item",
                    "description": "Get one item.",
                    "method": "GET",
                    "path": "/item",
                    "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["scope.read"]]},
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {"kind": "none"},
                    "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
                },
                {
                    "operation_id": "list_items",
                    "description": "List items.",
                    "method": "GET",
                    "path": "/items",
                    "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["scope.read"]]},
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {"kind": "none"},
                    "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
                }
            ]
        }))
        .expect("manifest");
        let definition = AdapterCompiler::compile(&manifest).expect("definition");
        let store = AdapterConnectionStore::new(paths.clone());
        let grant_id = "c".repeat(32);
        let first = AdapterConnectionV4 {
            schema_version: 4,
            connection_id: "a".repeat(32),
            connection_slug: "first".to_string(),
            semantic_digest: definition.semantic_digest.to_string(),
            connection_label: Some("Primary".to_string()),
            status: AdapterConnectionStatus::Active,
            connection_revision: 1,
            policy_revision: 1,
            authentication: AdapterConnectionAuthenticationV1::OauthGrant {
                grant_id: grant_id.clone(),
            },
            allowed_operations: vec!["list_items".to_string()],
            policy: Some(CapabilityConnectionPolicy {
                data_sharing: CapabilityDataSharingPolicy::AllowAutomatically,
                unsafe_actions: CapabilityUnsafeActionPolicy::NeverAsk,
                revision: 1,
            }),
            tool_overrides: Vec::new(),
        };
        let mut second = first.clone();
        second.connection_id = "b".repeat(32);
        second.connection_slug = "second".to_string();
        second.connection_label = None;
        second.allowed_operations = vec!["get_item".to_string()];
        second.policy = Some(CapabilityConnectionPolicy {
            data_sharing: CapabilityDataSharingPolicy::ReviewEveryCall,
            unsafe_actions: CapabilityUnsafeActionPolicy::AlwaysAsk,
            revision: 1,
        });
        store
            .install(&first, None, &definition)
            .expect("first connection");
        store
            .install(&second, None, &definition)
            .expect("second connection");

        let merged = store
            .merge_oauth_connections(&first.connection_id, &second.connection_id, &definition)
            .expect("merge connections");
        assert_eq!(
            merged.descriptor.allowed_operations,
            ["get_item", "list_items"]
        );
        assert_eq!(
            merged.descriptor.connection_label.as_deref(),
            Some("Primary")
        );
        let policy = merged.descriptor.policy.expect("merged policy");
        assert_eq!(
            policy.data_sharing,
            CapabilityDataSharingPolicy::ReviewEveryCall
        );
        assert_eq!(
            policy.unsafe_actions,
            CapabilityUnsafeActionPolicy::AlwaysAsk
        );
        store
            .quarantine(&second.connection_id)
            .expect("quarantine duplicate");
        assert!(
            !paths
                .adapter_connection_dir(&second.connection_id)
                .expect("active path")
                .exists()
        );
        assert!(
            paths
                .quarantined_adapter_connection_dir(&second.connection_id)
                .expect("quarantine path")
                .exists()
        );
    }
}
