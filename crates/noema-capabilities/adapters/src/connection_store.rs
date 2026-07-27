//! Filesystem authority for adapter connections and credential generations.

use crate::{
    AdapterConnectionV1, AdapterCredentialGenerationV1, AdapterCredentialMaterial,
    AuthenticationMode, CompiledAdapterDefinition, ConnectionSlug, DefinitionInstall,
    digest::canonical_json_bytes,
    private_fs::{
        PrivateFsError, create_private_dir, random_hex, read_bounded_regular_file,
        require_directory_no_symlink, require_exact_entries, require_regular_directory,
        sync_directory, write_new_file,
    },
};
use noema_home::NoemaPaths;
use std::{collections::BTreeMap, fs, path::Path};
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
    pub descriptor: AdapterConnectionV1,
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
    /// Stable external account identity when known.
    pub account_id: Option<String>,
    /// Reviewed account surface.
    pub account_kind: Option<String>,
    /// `active`, `suspended`, `authentication_required`, or `blocked`.
    pub status: &'static str,
    /// Descriptor/lifecycle revision.
    pub connection_revision: Option<u64>,
    /// Credential generation revision.
    pub credential_revision: Option<u64>,
    /// Provider grant revision.
    pub grant_revision: Option<u64>,
    /// Reviewed policy revision.
    pub policy_revision: Option<u64>,
    /// Current secret generation identity without its bytes.
    pub credential_generation: Option<String>,
    /// Exact non-secret grant scope subset.
    pub granted_scopes: Vec<String>,
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
                        descriptor.credential_generation.as_deref() != Some(generation)
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
        descriptor: &AdapterConnectionV1,
        credential: Option<&AdapterCredentialGenerationV1>,
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
                descriptor.credential_generation.as_deref(),
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

    /// Replace pre-authorization OAuth client metadata with one active token
    /// generation. The exact current descriptor is an optimistic revision
    /// fence; callers must also hold the connection lifecycle write lock.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when current authority changed, the requested
    /// transition is not an exact OAuth promotion, or durable publication
    /// cannot complete.
    #[allow(dead_code)] // Consumed by the next bounded token-exchange slice.
    pub(crate) fn promote_oauth_credential(
        &self,
        expected: &AdapterConnectionV1,
        replacement: &AdapterConnectionV1,
        credential: &AdapterCredentialGenerationV1,
        definition: &CompiledAdapterDefinition,
    ) -> Result<ConnectionInstall, ConnectionStoreError> {
        self.prepare_roots()?;
        let target = self.paths.adapter_connection_dir(&expected.connection_id)?;
        let (current, current_credential) =
            Self::read_descriptor(&target, &expected.connection_id)?;
        validate_connection(&current, current_credential.as_ref(), definition)?;
        validate_connection(replacement, Some(credential), definition)?;
        if current != *expected
            || !valid_oauth_promotion(
                &current,
                current_credential.as_ref(),
                replacement,
                credential,
            )
        {
            return Err(ConnectionStoreError::Integrity("credential_transition"));
        }

        let descriptor_bytes = canonical_json_bytes(&serde_json::to_value(replacement)?)?;
        let credential_bytes = canonical_json_bytes(&serde_json::to_value(credential)?)?;
        if descriptor_bytes.len() as u64 > MAX_CONNECTION_BYTES {
            return Err(ConnectionStoreError::Integrity("connection_oversized"));
        }
        if credential_bytes.len() as u64 > MAX_CREDENTIAL_BYTES {
            return Err(ConnectionStoreError::Integrity("credential_oversized"));
        }
        let credentials = target.join(CREDENTIALS_DIR);
        let new_credential_path = credential_path(&credentials, &credential.generation_id)?;
        let old_generation = current
            .credential_generation
            .as_deref()
            .ok_or(ConnectionStoreError::Integrity("credential_transition"))?;
        let old_credential_path = credential_path(&credentials, old_generation)?;
        let temporary_descriptor =
            credentials.join(format!("{REPLACEMENT_PREFIX}{}", random_hex(16)?));
        write_new_file(&new_credential_path, &credential_bytes)?;
        let mut descriptor_published = false;
        let result = (|| {
            write_new_file(&temporary_descriptor, &descriptor_bytes)?;
            sync_directory(&credentials)?;
            fs::rename(&temporary_descriptor, target.join(CONNECTION_FILE))?;
            descriptor_published = true;
            sync_directory(&target)?;
            fs::remove_file(&old_credential_path)?;
            sync_directory(&credentials)?;
            self.read_connection_dir(&target, &replacement.connection_id, definition)
        })();
        if !descriptor_published {
            let _ = fs::remove_file(&temporary_descriptor);
            let _ = fs::remove_file(&new_credential_path);
            let _ = sync_directory(&credentials);
        }
        result
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

    pub(crate) fn load_for_invocation(
        &self,
        connection_id: &str,
        definition: &CompiledAdapterDefinition,
    ) -> Result<(AdapterConnectionV1, Option<AdapterCredentialGenerationV1>), ConnectionStoreError>
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
    ) -> Result<(AdapterConnectionV1, Option<AdapterCredentialGenerationV1>), ConnectionStoreError>
    {
        let descriptor = Self::read_canonical_descriptor(path, expected_id)?;
        let credentials = path.join(CREDENTIALS_DIR);
        require_regular_directory(&credentials)?;
        let credentials_by_generation = read_credential_generations(&credentials)?;
        let credential = match descriptor.credential_generation.as_deref() {
            Some(generation) => {
                if credentials_by_generation.len() != 1 {
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
    ) -> Result<AdapterConnectionV1, ConnectionStoreError> {
        require_regular_directory(path)?;
        require_exact_entries(path, &[CONNECTION_FILE, CREDENTIALS_DIR])?;
        let bytes = read_bounded_regular_file(&path.join(CONNECTION_FILE), MAX_CONNECTION_BYTES)?;
        let descriptor: AdapterConnectionV1 = serde_json::from_slice(&bytes)?;
        if descriptor.connection_id != expected_id
            || canonical_json_bytes(&serde_json::to_value(&descriptor)?)? != bytes
        {
            return Err(ConnectionStoreError::Integrity("connection_identity"));
        }
        Ok(descriptor)
    }
}

#[allow(dead_code)] // Consumed through `promote_oauth_credential` in M2D2.
fn valid_oauth_promotion(
    current: &AdapterConnectionV1,
    current_credential: Option<&AdapterCredentialGenerationV1>,
    replacement: &AdapterConnectionV1,
    credential: &AdapterCredentialGenerationV1,
) -> bool {
    let Some(AdapterCredentialGenerationV1 {
        material:
            AdapterCredentialMaterial::Oauth2ClientMetadata {
                client_id,
                client_secret,
            },
        ..
    }) = current_credential
    else {
        return false;
    };
    let AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
        client_id: replacement_client_id,
        client_secret: replacement_client_secret,
        ..
    } = &credential.material
    else {
        return false;
    };
    current.status == crate::AdapterConnectionStatus::AuthenticationRequired
        && replacement.status == crate::AdapterConnectionStatus::Active
        && replacement.schema_version == current.schema_version
        && replacement.connection_id == current.connection_id
        && replacement.connection_slug == current.connection_slug
        && replacement.semantic_digest == current.semantic_digest
        && replacement.account_id == current.account_id
        && replacement.account_kind == current.account_kind
        && replacement.allowed_operations == current.allowed_operations
        && replacement.revisions.connection
            == current.revisions.connection.checked_add(1).unwrap_or(0)
        && replacement.revisions.credential
            == current.revisions.credential.checked_add(1).unwrap_or(0)
        && replacement.revisions.grant == current.revisions.grant.checked_add(1).unwrap_or(0)
        && replacement.revisions.policy == current.revisions.policy
        && replacement.credential_generation.as_deref() == Some(&credential.generation_id)
        && replacement.credential_generation != current.credential_generation
        && replacement_client_id == client_id
        && replacement_client_secret == client_secret
}

fn validate_connection(
    descriptor: &AdapterConnectionV1,
    credential: Option<&AdapterCredentialGenerationV1>,
    definition: &CompiledAdapterDefinition,
) -> Result<(), ConnectionStoreError> {
    if descriptor.schema_version != 1
        || !valid_hex_id(&descriptor.connection_id)
        || ConnectionSlug::new(descriptor.connection_slug.clone()).is_err()
        || descriptor.semantic_digest != definition.semantic_digest.as_str()
        || !definition.reviewed
        || !valid_component(&descriptor.account_kind, 96)
        || descriptor
            .account_id
            .as_deref()
            .is_some_and(|value| !valid_component(value, 256))
        || [
            descriptor.revisions.connection,
            descriptor.revisions.grant,
            descriptor.revisions.policy,
        ]
        .contains(&0)
        || !sorted_unique_text(&descriptor.granted_scopes, 256)
        || !sorted_unique_components(&descriptor.allowed_operations)
        || descriptor.allowed_operations.is_empty()
        || (!matches!(
            descriptor.status,
            crate::AdapterConnectionStatus::AuthenticationRequired
        ) && descriptor.granted_scopes.len() != definition.authentication.scopes.len())
        || (matches!(
            descriptor.status,
            crate::AdapterConnectionStatus::AuthenticationRequired
        ) && descriptor.granted_scopes.len() > definition.authentication.scopes.len())
        || descriptor
            .granted_scopes
            .iter()
            .any(|scope| !definition.authentication.scopes.contains(scope))
        || descriptor.allowed_operations.iter().any(|operation| {
            !definition
                .operations
                .iter()
                .any(|candidate| candidate.operation_id == *operation)
        })
    {
        return Err(ConnectionStoreError::Integrity("connection_policy"));
    }
    match (
        definition.authentication.mode,
        descriptor.status,
        descriptor.credential_generation.as_deref(),
        credential,
    ) {
        (AuthenticationMode::None, status, None, None)
            if status != crate::AdapterConnectionStatus::AuthenticationRequired
                && descriptor.revisions.credential == 0 => {}
        (AuthenticationMode::StaticBearer, status, Some(generation), Some(credential))
            if descriptor.revisions.credential > 0
                && credential_matches(
                    generation,
                    credential,
                    AuthenticationMode::StaticBearer,
                    definition,
                    status,
                ) => {}
        (
            AuthenticationMode::Oauth2AuthorizationCodePkce,
            status,
            Some(generation),
            Some(credential),
        ) if descriptor.revisions.credential > 0
            && credential_matches(
                generation,
                credential,
                AuthenticationMode::Oauth2AuthorizationCodePkce,
                definition,
                status,
            ) => {}
        (AuthenticationMode::Oauth2AuthorizationCodePkce, status, None, None)
            if status == crate::AdapterConnectionStatus::AuthenticationRequired
                && descriptor.revisions.credential == 0 => {}
        _ => return Err(ConnectionStoreError::Integrity("credential_binding")),
    }
    Ok(())
}

fn credential_matches(
    generation: &str,
    credential: &AdapterCredentialGenerationV1,
    mode: AuthenticationMode,
    definition: &CompiledAdapterDefinition,
    status: crate::AdapterConnectionStatus,
) -> bool {
    credential.schema_version == 1
        && credential.generation_id == generation
        && valid_hex_id(generation)
        && match (&credential.material, mode) {
            (
                AdapterCredentialMaterial::StaticBearer { token },
                AuthenticationMode::StaticBearer,
            ) => valid_secret(token),
            (
                AdapterCredentialMaterial::Oauth2ClientMetadata {
                    client_id,
                    client_secret,
                },
                AuthenticationMode::Oauth2AuthorizationCodePkce,
            ) => {
                status == crate::AdapterConnectionStatus::AuthenticationRequired
                    && definition
                        .authentication
                        .credential_import
                        .as_ref()
                        .is_some_and(|schema| {
                            schema.kind == crate::CredentialImportKind::OauthClientJson
                                && crate::credential_import::validate_import_schema(schema).is_ok()
                        })
                    && valid_secret(client_id)
                    && client_secret.as_deref().is_none_or(valid_secret)
            }
            (
                AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                    client_id,
                    client_secret,
                    access_token,
                    refresh_token,
                    ..
                },
                AuthenticationMode::Oauth2AuthorizationCodePkce,
            ) => {
                valid_secret(client_id)
                    && client_secret.as_deref().is_none_or(valid_secret)
                    && valid_secret(access_token)
                    && refresh_token.as_deref().is_none_or(valid_secret)
            }
            _ => false,
        }
}

fn read_credential_generations(
    path: &Path,
) -> Result<Vec<AdapterCredentialGenerationV1>, ConnectionStoreError> {
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
            let credential: AdapterCredentialGenerationV1 = serde_json::from_slice(&bytes)?;
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

fn projection(paths: &NoemaPaths, descriptor: &AdapterConnectionV1) -> ConnectionProjection {
    let credential_relative_path = descriptor.credential_generation.as_ref().map(|generation| {
        format!(
            "adapters/connections/{}/credentials/{generation}.json",
            descriptor.connection_id
        )
    });
    ConnectionProjection {
        connection_id: descriptor.connection_id.clone(),
        connection_slug: Some(descriptor.connection_slug.clone()),
        semantic_digest: Some(descriptor.semantic_digest.clone()),
        account_id: descriptor.account_id.clone(),
        account_kind: Some(descriptor.account_kind.clone()),
        status: descriptor.status.as_str(),
        connection_revision: Some(descriptor.revisions.connection),
        credential_revision: Some(descriptor.revisions.credential),
        grant_revision: Some(descriptor.revisions.grant),
        policy_revision: Some(descriptor.revisions.policy),
        credential_generation: descriptor.credential_generation.clone(),
        granted_scopes: descriptor.granted_scopes.clone(),
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
        account_id: None,
        account_kind: None,
        status: "blocked",
        connection_revision: None,
        credential_revision: None,
        grant_revision: None,
        policy_revision: None,
        credential_generation: None,
        granted_scopes: Vec::new(),
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

fn sorted_unique_text(values: &[String], max: usize) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| {
            !value.is_empty()
                && value.len() <= max
                && value.trim() == value
                && !value.bytes().any(|byte| byte.is_ascii_control())
        })
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
mod tests;
