//! Root-contract adapter capability service.

use crate::{
    AdapterCatalogCompiler, AdapterCompiler, AdapterConnectionRevisions, AdapterConnectionStatus,
    AdapterConnectionStore, AdapterConnectionV3, AdapterCredentialGenerationV2,
    AdapterCredentialMaterial, AdapterDefinitionStore, AuthenticationMode,
    CompiledAdapterDefinition, Oauth2CallbackMode, Oauth2ClientAuthentication,
    credential_import::setup_credential,
    network::{
        AdapterBearerCredential, AdapterHttpExecutor, AdapterOAuthTokenRequest,
        ReqwestAdapterHttpExecutor,
    },
    oauth::{
        AdapterOAuthAttempt, AdapterOAuthAttemptRegistry, AdapterOAuthAuthorityV1,
        AdapterOAuthError,
    },
    private_fs::random_hex,
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogBuilder, CapabilityCatalogResult, CapabilityConnectionPolicy,
    CapabilityFuture, CapabilityInvokerRegistration, CapabilityToolPolicyOverride, InvokerKey,
    validate_capability_connection_policy,
};
use noema_home::NoemaPaths;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

/// Safe failure from the human-owned adapter connection setup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdapterConnectionSetupError {
    /// The exact reviewed definition is absent or invalid.
    #[error("adapter definition is unavailable")]
    DefinitionUnavailable,
    /// The transient credential document does not match the reviewed schema.
    #[error("adapter credential document is invalid")]
    InvalidCredential,
    /// Canonical setup state could not be published safely.
    #[error("adapter connection could not be created")]
    Unavailable,
}

/// Safe failure from the provider-neutral interactive OAuth setup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdapterOAuthSetupError {
    /// The connection or its reviewed OAuth authority is unavailable.
    #[error("adapter OAuth setup is unavailable")]
    Unavailable,
    /// The callback listener or request was invalid.
    #[error("adapter OAuth setup input is invalid")]
    Invalid,
    /// The initiating authority changed before completion.
    #[error("adapter OAuth setup was superseded")]
    Superseded,
    /// The short-lived browser attempt expired.
    #[error("adapter OAuth setup expired")]
    Expired,
    /// The authorization or token endpoint denied the request.
    #[error("adapter OAuth setup was denied")]
    Denied,
}

/// Safe failure from the non-secret adapter management boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdapterManagementError {
    /// The requested connection or exact operation is absent.
    #[error("adapter management target was not found")]
    NotFound,
    /// The submitted policy pair is invalid.
    #[error("adapter management input is invalid")]
    Invalid,
    /// A submitted revision no longer matches canonical state.
    #[error("adapter management state changed")]
    Conflict,
    /// Canonical management state could not be read or published.
    #[error("adapter management is unavailable")]
    Unavailable,
}

/// Filesystem-canonical definitions and connections for management reads.
#[derive(Debug)]
pub struct AdapterManagementSnapshot {
    /// Reviewed immutable API definitions.
    pub definitions: Vec<crate::DefinitionInstall>,
    /// Exact definition revisions replaced by each immutable revision.
    pub definition_replacements: BTreeMap<String, Vec<String>>,
    /// Valid concrete API connections.
    pub connections: Vec<crate::ConnectionInstall>,
}

/// Exact fence shared by one adapter management mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterManagementFence {
    /// Stable concrete connection identity.
    pub connection_id: String,
    /// Descriptor revision observed by the caller.
    pub expected_connection_revision: u64,
    /// Tool/policy revision observed by the caller.
    pub expected_policy_revision: u64,
}

enum AdapterManagementChange {
    ConnectionPolicy {
        data_sharing: noema_capabilities::CapabilityDataSharingPolicy,
        unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy,
    },
    ToolOverride(CapabilityToolPolicyOverride),
    ResetTool {
        tool_id: String,
        source_revision: String,
    },
    SetToolEnabled {
        tool_id: String,
        source_revision: String,
        enabled: bool,
    },
}

/// Safe failure from the one-time filesystem definition rewrite.
#[derive(Debug, thiserror::Error)]
pub enum AdapterMigrationError {
    /// The process-local rewrite coordinator is unavailable.
    #[error("adapter migration is unavailable")]
    Unavailable,
    /// A definition could not be validated, installed, or quarantined.
    #[error("adapter definition migration failed: {0}")]
    Definition(#[from] crate::DefinitionStoreError),
    /// A connection descriptor could not be rebound safely.
    #[error("adapter connection migration failed: {0}")]
    Connection(#[from] crate::ConnectionStoreError),
    /// A polling schedule could not be rebound safely.
    #[error("adapter schedule migration failed: {0}")]
    Schedule(#[from] crate::ScheduleError),
}

/// Opaque browser handoff for one process-local OAuth attempt.
#[derive(Clone, PartialEq, Eq)]
pub struct AdapterOAuthSetupStart {
    /// Random attempt identity safe for UI polling correlation.
    pub attempt_id: String,
    /// Authorization URL to open only in the human's browser.
    pub authorization_url: String,
    /// Absolute Unix expiry for the process-local attempt.
    pub expires_at_epoch_seconds: u64,
}

/// Completed OAuth publication and the attempt that authorized it.
#[derive(Debug)]
pub struct AdapterOAuthSetupCompletion {
    /// Process-local attempt identity used by paused capability calls.
    pub attempt_id: String,
    /// Canonical active connection after credential replacement.
    pub connection: crate::ConnectionInstall,
    /// Whether this callback activated a previously unauthenticated connection.
    pub newly_activated: bool,
}

impl std::fmt::Debug for AdapterOAuthSetupStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterOAuthSetupStart")
            .field("attempt_id", &self.attempt_id)
            .field("authorization_url", &"[REDACTED]")
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .finish()
    }
}

use crate::catalog::ADAPTER_INVOKER_KEY;
const OAUTH_ATTEMPT_TTL_SECONDS: u64 = 10 * 60;

pub(crate) struct AdapterCapabilityServiceInner {
    pub(crate) definitions: AdapterDefinitionStore,
    pub(crate) connections: AdapterConnectionStore,
    pub(crate) cursors: crate::DurableCursorStore,
    schedules: crate::ScheduleStore,
    pub(crate) oauth_callback_mode: Mutex<Option<Oauth2CallbackMode>>,
    migration_lock: Mutex<()>,
    pub(crate) definition_lock: Mutex<()>,
    connection_locks: Mutex<BTreeMap<String, Arc<RwLock<()>>>>,
    pub(crate) cursor_lock: tokio::sync::Mutex<()>,
    oauth_attempts: Mutex<AdapterOAuthAttemptRegistry>,
    pub(crate) http: Arc<dyn AdapterHttpExecutor>,
}

/// Filesystem-backed binding source and credentialed JSON invoker.
#[derive(Clone)]
pub struct AdapterCapabilityService {
    pub(crate) inner: Arc<AdapterCapabilityServiceInner>,
}

struct LoadedOAuthConnection {
    definition: CompiledAdapterDefinition,
    descriptor: AdapterConnectionV3,
    client_id: String,
    client_secret: Option<String>,
    callback_mode: Oauth2CallbackMode,
}

impl std::fmt::Debug for AdapterCapabilityService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterCapabilityService")
            .field("http", &"[CONFIGURED]")
            .finish_non_exhaustive()
    }
}

impl AdapterCapabilityService {
    /// Construct the production filesystem and HTTP capability service.
    #[must_use]
    pub fn new(paths: NoemaPaths) -> Self {
        Self::with_http(paths, Arc::new(ReqwestAdapterHttpExecutor))
    }

    fn with_http(paths: NoemaPaths, http: Arc<dyn AdapterHttpExecutor>) -> Self {
        Self {
            inner: Arc::new(AdapterCapabilityServiceInner {
                definitions: AdapterDefinitionStore::new(paths.clone()),
                connections: AdapterConnectionStore::new(paths.clone()),
                cursors: crate::DurableCursorStore::new(paths.clone()),
                schedules: crate::ScheduleStore::new(paths),
                oauth_callback_mode: Mutex::new(None),
                migration_lock: Mutex::new(()),
                definition_lock: Mutex::new(()),
                connection_locks: Mutex::new(BTreeMap::new()),
                cursor_lock: tokio::sync::Mutex::new(()),
                oauth_attempts: Mutex::new(AdapterOAuthAttemptRegistry::default()),
                http,
            }),
        }
    }

    /// Return the generic root binding-source handle.
    #[must_use]
    pub fn binding_source(&self) -> CapabilityBindingSourceHandle {
        Arc::new(self.clone())
    }

    /// Return the generic root invoker registration.
    #[must_use]
    pub fn invoker_registration(&self) -> CapabilityInvokerRegistration {
        CapabilityInvokerRegistration::new(
            InvokerKey::new(ADAPTER_INVOKER_KEY),
            Arc::new(self.clone()),
        )
    }

    /// Expose the callback mode owned by the serving shell to definition proposals.
    pub fn set_oauth_callback_mode(&self, mode: Oauth2CallbackMode) {
        if let Ok(mut configured) = self.inner.oauth_callback_mode.lock() {
            *configured = Some(mode);
        }
    }

    /// Read the current non-secret adapter management hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a safe category when canonical filesystem state is unavailable.
    pub fn management_snapshot(&self) -> Result<AdapterManagementSnapshot, AdapterManagementError> {
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let definition_replacements = definitions
            .definitions
            .iter()
            .map(|definition| {
                let digest = definition.compiled.semantic_digest.to_string();
                let stored = self
                    .inner
                    .definitions
                    .load(&digest)
                    .map_err(|_| AdapterManagementError::Unavailable)?;
                Ok((digest, stored.provenance.replaces_semantic_digests))
            })
            .collect::<Result<BTreeMap<_, _>, AdapterManagementError>>()?;
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(AdapterManagementSnapshot {
            definitions: definitions.definitions,
            definition_replacements,
            connections: connections.connections,
        })
    }

    /// Publish one exact current pending definition as a reviewed immutable revision.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the target is absent, stale, or cannot be published.
    pub fn review_definition(
        &self,
        semantic_digest: &str,
    ) -> Result<crate::DefinitionInstall, AdapterManagementError> {
        let _guard = self
            .inner
            .definition_lock
            .lock()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let scan = self
            .inner
            .definitions
            .scan()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let superseded = self
            .inner
            .definitions
            .superseded_pending_digests(&scan)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        if superseded.contains(semantic_digest) {
            return Err(AdapterManagementError::Conflict);
        }
        let stored = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterManagementError::NotFound)?;
        if stored.manifest.reviewed {
            return Err(AdapterManagementError::Conflict);
        }
        let mut reviewed = stored.manifest;
        reviewed.reviewed = true;
        let source = stored
            .source
            .as_ref()
            .map(|(bytes, extension)| (bytes.as_slice(), extension.as_str()));
        self.inner
            .definitions
            .install_with_provenance(&reviewed, stored.provenance, source)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    /// Abandon one exact current proposal while preserving reviewed revisions.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the target is absent, stale, reviewed, or cannot be quarantined.
    pub fn cancel_definition_proposal(
        &self,
        semantic_digest: &str,
    ) -> Result<bool, AdapterManagementError> {
        let _guard = self
            .inner
            .definition_lock
            .lock()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let scan = self
            .inner
            .definitions
            .scan()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let superseded = self
            .inner
            .definitions
            .superseded_pending_digests(&scan)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        if superseded.contains(semantic_digest) {
            return Err(AdapterManagementError::Conflict);
        }
        let Some(target) = scan
            .definitions
            .iter()
            .find(|definition| definition.compiled.semantic_digest.as_str() == semantic_digest)
        else {
            return Ok(false);
        };
        if target.compiled.reviewed {
            return Err(AdapterManagementError::Conflict);
        }
        let definition_id = &target.compiled.definition_id;
        let mut pending = scan
            .definitions
            .iter()
            .filter(|definition| {
                !definition.compiled.reviewed && definition.compiled.definition_id == *definition_id
            })
            .map(|definition| definition.compiled.semantic_digest.to_string())
            .collect::<Vec<_>>();
        pending.sort();
        for digest in pending
            .iter()
            .filter(|digest| digest.as_str() != semantic_digest)
        {
            self.inner
                .definitions
                .quarantine(digest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
        }
        self.inner
            .definitions
            .quarantine(semantic_digest)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(true)
    }

    /// Save both connection policy choices under exact descriptor fences.
    ///
    /// # Errors
    ///
    /// Returns a safe category for invalid input, stale state, or unavailable storage.
    pub async fn save_management_policy(
        &self,
        fence: AdapterManagementFence,
        data_sharing: noema_capabilities::CapabilityDataSharingPolicy,
        unsafe_actions: noema_capabilities::CapabilityUnsafeActionPolicy,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        validate_capability_connection_policy(data_sharing, unsafe_actions)
            .map_err(|_| AdapterManagementError::Invalid)?;
        self.apply_management_change(
            fence,
            AdapterManagementChange::ConnectionPolicy {
                data_sharing,
                unsafe_actions,
            },
        )
        .await
    }

    /// Save all four human tool hints under exact descriptor and source fences.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the connection, policy, or source revision changed.
    pub async fn save_management_tool_override(
        &self,
        fence: AdapterManagementFence,
        policy: CapabilityToolPolicyOverride,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        self.apply_management_change(fence, AdapterManagementChange::ToolOverride(policy))
            .await
    }

    /// Reset one tool to the current definition-provided behavior.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the connection, policy, or source revision changed.
    pub async fn reset_management_tool_policy(
        &self,
        fence: AdapterManagementFence,
        tool_id: String,
        source_revision: String,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        self.apply_management_change(
            fence,
            AdapterManagementChange::ResetTool {
                tool_id,
                source_revision,
            },
        )
        .await
    }

    /// Enable or disable one exact current tool revision.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the connection, policy, or source revision changed.
    pub async fn set_management_tool_enabled(
        &self,
        fence: AdapterManagementFence,
        tool_id: String,
        source_revision: String,
        enabled: bool,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        self.apply_management_change(
            fence,
            AdapterManagementChange::SetToolEnabled {
                tool_id,
                source_revision,
                enabled,
            },
        )
        .await
    }

    /// Replace the human-visible connection label under an exact, non-rotating fence.
    ///
    /// # Errors
    /// Returns a safe category for invalid input, stale state, or unavailable storage.
    pub async fn save_connection_label(
        &self,
        connection_id: String,
        expected_connection_revision: u64,
        expected_connection_label: Option<String>,
        connection_label: Option<String>,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        let connection_label =
            noema_capabilities::normalize_capability_connection_label(connection_label)
                .map_err(|_| AdapterManagementError::Invalid)?;
        let expected_connection_label =
            noema_capabilities::normalize_capability_connection_label(expected_connection_label)
                .map_err(|_| AdapterManagementError::Invalid)?;
        let lock = self
            .connection_lock(&connection_id)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self.management_snapshot()?;
        let current = snapshot
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == connection_id)
            .ok_or(AdapterManagementError::NotFound)?;
        if current.descriptor.revisions.connection != expected_connection_revision
            || current.descriptor.connection_label != expected_connection_label
        {
            return Err(AdapterManagementError::Conflict);
        }
        let definition = snapshot
            .definitions
            .iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == current.descriptor.semantic_digest
            })
            .ok_or(AdapterManagementError::Unavailable)?;
        let mut replacement = current.descriptor.clone();
        replacement.connection_label = connection_label;
        self.inner
            .connections
            .replace_management_descriptor(&current.descriptor, &replacement, &definition.compiled)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    async fn apply_management_change(
        &self,
        fence: AdapterManagementFence,
        change: AdapterManagementChange,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        let lock = self
            .connection_lock(&fence.connection_id)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self.management_snapshot()?;
        let current = snapshot
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == fence.connection_id)
            .ok_or(AdapterManagementError::NotFound)?;
        let definition = snapshot
            .definitions
            .iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == current.descriptor.semantic_digest
            })
            .ok_or(AdapterManagementError::Unavailable)?;
        if current.descriptor.revisions.connection != fence.expected_connection_revision
            || current.descriptor.revisions.policy != fence.expected_policy_revision
        {
            return Err(AdapterManagementError::Conflict);
        }
        let mut replacement = current.descriptor.clone();
        match change {
            AdapterManagementChange::ConnectionPolicy {
                data_sharing,
                unsafe_actions,
            } => {
                replacement.policy = Some(CapabilityConnectionPolicy {
                    data_sharing,
                    unsafe_actions,
                    revision: fence
                        .expected_policy_revision
                        .checked_add(1)
                        .ok_or(AdapterManagementError::Conflict)?,
                });
            }
            AdapterManagementChange::ToolOverride(policy) => {
                require_current_operation(
                    &definition.compiled,
                    &policy.tool_id,
                    &policy.source_revision,
                )?;
                replacement
                    .tool_overrides
                    .retain(|candidate| candidate.tool_id != policy.tool_id);
                replacement.tool_overrides.push(policy);
                replacement
                    .tool_overrides
                    .sort_by(|left, right| left.tool_id.cmp(&right.tool_id));
            }
            AdapterManagementChange::ResetTool {
                tool_id,
                source_revision,
            } => {
                require_current_operation(&definition.compiled, &tool_id, &source_revision)?;
                replacement
                    .tool_overrides
                    .retain(|candidate| candidate.tool_id != tool_id);
            }
            AdapterManagementChange::SetToolEnabled {
                tool_id,
                source_revision,
                enabled,
            } => {
                require_current_operation(&definition.compiled, &tool_id, &source_revision)?;
                replacement
                    .allowed_operations
                    .retain(|candidate| candidate != &tool_id);
                if enabled {
                    replacement.allowed_operations.push(tool_id);
                    replacement.allowed_operations.sort();
                }
            }
        }
        replacement.revisions.connection = replacement
            .revisions
            .connection
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        replacement.revisions.policy = replacement
            .revisions
            .policy
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        if let Some(policy) = replacement.policy.as_mut() {
            policy.revision = replacement.revisions.policy;
        }
        self.inner
            .connections
            .replace_management_descriptor(&current.descriptor, &replacement, &definition.compiled)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    /// Normalize reviewed write-only credential input and publish one connection.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the exact reviewed definition is absent,
    /// extraction fails, or atomic publication cannot complete.
    pub async fn setup_connection(
        &self,
        semantic_digest: &str,
        field_values: BTreeMap<String, String>,
        document: Option<&[u8]>,
    ) -> Result<crate::ConnectionInstall, AdapterConnectionSetupError> {
        crate::SemanticDigest::parse(semantic_digest.to_string())
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let stored = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let definition = AdapterCompiler::compile(&stored.manifest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        if !definition.reviewed || definition.semantic_digest.as_str() != semantic_digest {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
        let setup_lock = self
            .connection_lock(&format!("adapter-family:{}", definition.definition_id))
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let _guard = setup_lock.write().await;
        let connection_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let connection_slug = format!("personal-{}", &connection_id[..8]);
        let generation_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let callback_mode = match definition.authentication.mode() {
            AuthenticationMode::Oauth2AuthorizationCodePkce => Some(
                self.inner
                    .oauth_callback_mode
                    .lock()
                    .ok()
                    .and_then(|mode| *mode)
                    .ok_or(AdapterConnectionSetupError::DefinitionUnavailable)?,
            ),
            AuthenticationMode::Credential => None,
            AuthenticationMode::None => {
                return Err(AdapterConnectionSetupError::DefinitionUnavailable);
            }
        };
        let credential = setup_credential(
            &definition,
            callback_mode,
            field_values,
            document,
            generation_id,
        )
        .map_err(|_| AdapterConnectionSetupError::InvalidCredential)?;
        let status = if definition.authentication.mode() == AuthenticationMode::Credential {
            AdapterConnectionStatus::Active
        } else {
            AdapterConnectionStatus::AuthenticationRequired
        };
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV3 {
            schema_version: 3,
            connection_id,
            connection_slug,
            semantic_digest: semantic_digest.to_string(),
            account_id: None,
            connection_label: None,
            account_kind: "personal".to_string(),
            status,
            revisions: AdapterConnectionRevisions {
                connection: 1,
                credential: 1,
                grant: 1,
                policy: 1,
            },
            credential_generation: Some(credential.generation_id.clone()),
            granted_scopes: Vec::new(),
            allowed_operations,
            policy: None,
            tool_overrides: Vec::new(),
        };
        self.inner
            .connections
            .install(&descriptor, Some(&credential), &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Normalize one OAuth client document through the active callback setup.
    #[cfg(test)]
    pub(crate) async fn import_oauth_client_json(
        &self,
        semantic_digest: &str,
        bytes: &[u8],
    ) -> Result<crate::ConnectionInstall, AdapterConnectionSetupError> {
        self.setup_connection(semantic_digest, BTreeMap::new(), Some(bytes))
            .await
    }

    /// Ensure one active credential-free connection exists for a reviewed definition.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the definition is absent, requires authentication,
    /// or canonical connection state cannot be read or published.
    pub async fn ensure_credential_free_connection(
        &self,
        semantic_digest: &str,
    ) -> Result<crate::ConnectionInstall, AdapterConnectionSetupError> {
        crate::SemanticDigest::parse(semantic_digest.to_string())
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let stored = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let definition = AdapterCompiler::compile(&stored.manifest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        if !definition.reviewed
            || definition.semantic_digest.as_str() != semantic_digest
            || definition.authentication.mode() != AuthenticationMode::None
        {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
        let setup_lock = self
            .connection_lock(&format!("adapter-family:{}", definition.definition_id))
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let _guard = setup_lock.write().await;
        if let Some(connection) = self
            .management_snapshot()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?
            .connections
            .into_iter()
            .find(|connection| connection.descriptor.semantic_digest == semantic_digest)
        {
            return Ok(connection);
        }
        let connection_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV3 {
            schema_version: 3,
            connection_slug: format!("personal-{}", &connection_id[..8]),
            connection_id,
            semantic_digest: semantic_digest.to_string(),
            account_id: None,
            connection_label: None,
            account_kind: "personal".to_string(),
            status: AdapterConnectionStatus::Active,
            revisions: AdapterConnectionRevisions {
                connection: 1,
                credential: 0,
                grant: 1,
                policy: 1,
            },
            credential_generation: None,
            granted_scopes: Vec::new(),
            allowed_operations,
            policy: None,
            tool_overrides: Vec::new(),
        };
        self.inner
            .connections
            .install(&descriptor, None, &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Start one provider-neutral authorization-code/PKCE attempt for a
    /// filesystem-canonical connection.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the exact connection or reviewed OAuth
    /// authority is unavailable, the callback URI is invalid, or attempt
    /// capacity is exhausted.
    pub async fn start_oauth_setup(
        &self,
        human_id: &str,
        connection_id: &str,
        expected_revisions: AdapterConnectionRevisions,
        callback_mode: Oauth2CallbackMode,
        redirect_uri: &str,
    ) -> Result<AdapterOAuthSetupStart, AdapterOAuthSetupError> {
        self.start_oauth_setup_at(
            human_id,
            connection_id,
            expected_revisions,
            callback_mode,
            redirect_uri,
            epoch_seconds()?,
        )
        .await
    }

    async fn start_oauth_setup_at(
        &self,
        human_id: &str,
        connection_id: &str,
        expected_revisions: AdapterConnectionRevisions,
        callback_mode: Oauth2CallbackMode,
        redirect_uri: &str,
        now_epoch_seconds: u64,
    ) -> Result<AdapterOAuthSetupStart, AdapterOAuthSetupError> {
        if !valid_connection_id(connection_id) {
            return Err(AdapterOAuthSetupError::Invalid);
        }
        let lock = self
            .connection_lock(connection_id)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let _guard = lock.write().await;
        let current = self.load_oauth_connection(connection_id)?;
        if !matches!(
            current.descriptor.status,
            AdapterConnectionStatus::AuthenticationRequired | AdapterConnectionStatus::Active
        ) || current.descriptor.revisions != expected_revisions
        {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        if current
            .definition
            .authentication
            .oauth2()
            .is_some_and(|config| {
                config.client_authentication != Oauth2ClientAuthentication::None
                    && current.client_secret.is_none()
            })
        {
            return Err(AdapterOAuthSetupError::Invalid);
        }
        if current.callback_mode != callback_mode {
            return Err(AdapterOAuthSetupError::Invalid);
        }
        let authority = oauth_authority(human_id, &current.descriptor);
        let attempt = AdapterOAuthAttempt::start(
            &current.definition,
            &current.client_id,
            authority,
            callback_mode,
            redirect_uri,
            now_epoch_seconds,
            OAUTH_ATTEMPT_TTL_SECONDS,
        )
        .map_err(map_oauth_error)?;
        let started = AdapterOAuthSetupStart {
            attempt_id: attempt.attempt_id().to_string(),
            authorization_url: attempt.authorization_url().to_string(),
            expires_at_epoch_seconds: attempt.expires_at_epoch_seconds(),
        };
        self.inner
            .oauth_attempts
            .lock()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?
            .insert(attempt, now_epoch_seconds)
            .map_err(map_oauth_error)?;
        Ok(started)
    }

    /// Consume one returned OAuth callback, exchange its code against the
    /// reviewed token endpoint, and atomically activate or reauthenticate the connection.
    ///
    /// # Errors
    ///
    /// Returns a safe category for a missing/expired attempt, changed
    /// authority, provider denial, invalid token response, or publication
    /// failure.
    pub async fn complete_oauth_callback(
        &self,
        callback_url: &str,
    ) -> Result<AdapterOAuthSetupCompletion, AdapterOAuthSetupError> {
        self.complete_oauth_callback_at(callback_url, epoch_seconds()?)
            .await
    }

    async fn complete_oauth_callback_at(
        &self,
        callback_url: &str,
        now_epoch_seconds: u64,
    ) -> Result<AdapterOAuthSetupCompletion, AdapterOAuthSetupError> {
        let initiating_authority = self
            .inner
            .oauth_attempts
            .lock()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?
            .authority_for_callback(callback_url)
            .map_err(map_oauth_error)?;
        let connection_id = initiating_authority.connection_id.clone();
        let lock = self
            .connection_lock(&connection_id)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let (current, reservation) = {
            let _guard = lock.write().await;
            let current = self.load_oauth_connection(&connection_id)?;
            let current_authority =
                oauth_authority(&initiating_authority.human_id, &current.descriptor);
            let reservation = self
                .inner
                .oauth_attempts
                .lock()
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?
                .reserve_for_callback(callback_url, now_epoch_seconds, &current_authority)
                .map_err(map_oauth_error)?;
            (current, reservation)
        };
        let state_key = reservation.state_key();
        let attempt_id = reservation.attempt_id().to_string();
        let result = async {
            let code = reservation
                .complete(callback_url, now_epoch_seconds, &initiating_authority)
                .map_err(map_oauth_error)?;
            let config = current
                .definition
                .authentication
                .oauth2()
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            let (authorization_code, redirect_uri, pkce_verifier) = code.token_exchange_parts();
            let token = self
                .inner
                .http
                .exchange_oauth_token(AdapterOAuthTokenRequest {
                    token_endpoint: url::Url::parse(&config.token_endpoint)
                        .map_err(|_| AdapterOAuthSetupError::Unavailable)?,
                    client_authentication: config.client_authentication,
                    client_id: current.client_id.clone(),
                    client_secret: current.client_secret.clone(),
                    code: authorization_code.to_string(),
                    redirect_uri: redirect_uri.to_string(),
                    pkce_verifier: pkce_verifier.to_string(),
                    requested_scopes: current.definition.authentication.scopes().to_vec(),
                    now_epoch_seconds,
                })
                .await
                .map_err(map_oauth_token_error)?;
            let connection_label = self
                .probe_connection_label(&current.definition, &token.access_token)
                .await;

            let _guard = lock.write().await;
            let fresh = self.load_oauth_connection(&connection_id)?;
            let fresh_authority =
                oauth_authority(&initiating_authority.human_id, &fresh.descriptor);
            if !initiating_authority.matches(&fresh_authority)
                || fresh.client_id != current.client_id
                || fresh.client_secret != current.client_secret
                || fresh.callback_mode != current.callback_mode
            {
                return Err(AdapterOAuthSetupError::Superseded);
            }
            let generation_id = random_hex(16).map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            let newly_activated =
                fresh.descriptor.status == AdapterConnectionStatus::AuthenticationRequired;
            let credential = AdapterCredentialGenerationV2 {
                schema_version: 2,
                generation_id: generation_id.clone(),
                material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                    callback_mode: fresh.callback_mode,
                    client_id: fresh.client_id,
                    client_secret: fresh.client_secret,
                    access_token: token.access_token,
                    refresh_token: token.refresh_token,
                    expires_at_epoch_seconds: token.expires_at_epoch_seconds,
                },
            };
            let mut replacement = fresh.descriptor.clone();
            replacement.status = AdapterConnectionStatus::Active;
            replacement.revisions.connection = replacement
                .revisions
                .connection
                .checked_add(1)
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            replacement.revisions.credential = replacement
                .revisions
                .credential
                .checked_add(1)
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            replacement.revisions.grant = replacement
                .revisions
                .grant
                .checked_add(1)
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            replacement.credential_generation = Some(generation_id);
            replacement.granted_scopes = token.granted_scopes;
            if replacement.connection_label.is_none() && connection_label.is_some() {
                replacement.connection_label = connection_label;
            }
            let connection = self
                .inner
                .connections
                .promote_oauth_credential(
                    &fresh.descriptor,
                    &replacement,
                    &credential,
                    &fresh.definition,
                )
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            Ok(AdapterOAuthSetupCompletion {
                attempt_id,
                connection,
                newly_activated,
            })
        }
        .await;
        if let Ok(mut attempts) = self.inner.oauth_attempts.lock() {
            attempts.finish(state_key);
        }
        result
    }

    async fn probe_connection_label(
        &self,
        definition: &CompiledAdapterDefinition,
        access_token: &str,
    ) -> Option<String> {
        let probe = definition.authentication.account_identity()?;
        let operation = definition
            .operations
            .iter()
            .find(|operation| operation.operation_id == probe.operation_id)?;
        let arguments = serde_json::Value::Object(probe.arguments.clone().into_iter().collect());
        let request = crate::request::encode_request(definition, operation, &arguments).ok()?;
        let response = self
            .inner
            .http
            .execute(
                operation.method,
                operation.retry,
                request,
                Some(AdapterBearerCredential::new(access_token.to_string())),
            )
            .await
            .ok()?;
        if !(200..300).contains(&response.status) || response.status == 204 {
            return None;
        }
        let value = crate::response::success(&response, &operation.response)
            .await
            .ok()?;
        noema_capabilities::normalize_capability_connection_label(Some(
            value.pointer(&probe.output_pointer)?.as_str()?.to_owned(),
        ))
        .ok()
        .flatten()
    }

    fn load_oauth_connection(
        &self,
        connection_id: &str,
    ) -> Result<LoadedOAuthConnection, AdapterOAuthSetupError> {
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let descriptor = connections
            .connections
            .into_iter()
            .find(|connection| connection.descriptor.connection_id == connection_id)
            .map(|connection| connection.descriptor)
            .ok_or(AdapterOAuthSetupError::Unavailable)?;
        let definition = definitions
            .definitions
            .into_iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == descriptor.semantic_digest
            })
            .map(|definition| definition.compiled)
            .ok_or(AdapterOAuthSetupError::Unavailable)?;
        let (_, credential) = self
            .inner
            .connections
            .load_for_invocation(connection_id, &definition)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let Some(credential) = credential else {
            return Err(AdapterOAuthSetupError::Superseded);
        };
        let (callback_mode, client_id, client_secret) = match credential.material {
            AdapterCredentialMaterial::Oauth2ClientMetadata {
                callback_mode,
                client_id,
                client_secret,
            }
            | AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                callback_mode,
                client_id,
                client_secret,
                ..
            } => (callback_mode, client_id, client_secret),
            _ => return Err(AdapterOAuthSetupError::Superseded),
        };
        Ok(LoadedOAuthConnection {
            definition,
            descriptor,
            client_id,
            client_secret,
            callback_mode,
        })
    }

    /// Quarantine one connection under the same lifecycle fence as invocation.
    ///
    /// # Errors
    ///
    /// Returns whether an exact current connection was quarantined. A stale
    /// revision or failed durable rename returns a redacted store error.
    pub async fn quarantine_connection(
        &self,
        connection_id: &str,
        expected_connection_revision: u64,
    ) -> Result<bool, crate::ConnectionStoreError> {
        let lock = self
            .connection_lock(connection_id)
            .map_err(|_| crate::ConnectionStoreError::Integrity("lifecycle_lock"))?;
        let _guard = lock.write().await;
        let definitions =
            self.inner.definitions.scan().map_err(|_| {
                crate::ConnectionStoreError::Integrity("definition_scan_unavailable")
            })?;
        let connections = self.inner.connections.scan(&definitions.definitions)?;
        let Some(connection) = connections
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == connection_id)
        else {
            return Ok(false);
        };
        if connection.descriptor.revisions.connection != expected_connection_revision {
            return Err(crate::ConnectionStoreError::Integrity(
                "stale_connection_revision",
            ));
        }
        self.inner.connections.quarantine(connection_id)?;
        Ok(true)
    }

    /// Quarantine every revision in one adapter definition family.
    ///
    /// # Errors
    ///
    /// Returns a redacted store error when the selected revision is stale, a
    /// connection or schedule still references the family, or a durable rename
    /// cannot complete.
    pub async fn quarantine_definition_family(
        &self,
        definition_id: &str,
        expected_semantic_digest: &str,
    ) -> Result<bool, crate::DefinitionStoreError> {
        let lock = self
            .connection_lock(&format!("adapter-family:{definition_id}"))
            .map_err(|_| crate::DefinitionStoreError::Integrity("lifecycle_lock"))?;
        let _guard = lock.write().await;
        let definitions = self.inner.definitions.scan()?;
        let family = definitions
            .definitions
            .iter()
            .filter(|definition| definition.compiled.definition_id == definition_id)
            .collect::<Vec<_>>();
        let Some(current) = family.iter().max_by(|left, right| {
            left.compiled
                .reviewed
                .cmp(&right.compiled.reviewed)
                .then_with(|| {
                    left.compiled
                        .definition_revision
                        .cmp(&right.compiled.definition_revision)
                })
                .then_with(|| {
                    left.compiled
                        .semantic_digest
                        .cmp(&right.compiled.semantic_digest)
                })
        }) else {
            return Ok(false);
        };
        let current_digest = current.compiled.semantic_digest.to_string();
        if current_digest != expected_semantic_digest {
            return Err(crate::DefinitionStoreError::Integrity(
                "stale_definition_revision",
            ));
        }
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| crate::DefinitionStoreError::Integrity("connection_scan_unavailable"))?;
        if connections.connections.iter().any(|connection| {
            family.iter().any(|definition| {
                definition.compiled.semantic_digest.as_str()
                    == connection.descriptor.semantic_digest
            })
        }) {
            return Err(crate::DefinitionStoreError::Integrity(
                "definition_has_connections",
            ));
        }
        for definition in &family {
            if self
                .inner
                .schedules
                .references_definition(definition.compiled.semantic_digest.as_str())
                .map_err(|_| crate::DefinitionStoreError::Integrity("schedule_scan_unavailable"))?
            {
                return Err(crate::DefinitionStoreError::Integrity(
                    "definition_has_schedules",
                ));
            }
        }
        for definition in family
            .iter()
            .filter(|definition| definition.compiled.semantic_digest.as_str() != current_digest)
        {
            self.inner
                .definitions
                .quarantine(definition.compiled.semantic_digest.as_str())?;
        }
        self.inner.definitions.quarantine(&current_digest)?;
        Ok(true)
    }

    pub(crate) fn connection_lock(
        &self,
        connection_id: &str,
    ) -> Result<Arc<RwLock<()>>, CapabilityBindingSourceError> {
        let mut locks = self
            .inner
            .connection_locks
            .lock()
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        Ok(locks
            .entry(connection_id.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone())
    }

    fn compile_catalog(&self) -> Result<CapabilityCatalogResult, CapabilityBindingSourceError> {
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        let diagnostic_count = connections.diagnostics.len();
        let mut catalog = AdapterCatalogCompiler::compile(&definitions.definitions, &connections)
            .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        let mut bindings = CapabilityCatalogBuilder::new();
        bindings
            .add(
                crate::setup::definition_template_binding()
                    .map_err(|_| CapabilityBindingSourceError::Invalid)?,
            )
            .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        bindings
            .add(
                crate::setup::proposal_binding()
                    .map_err(|_| CapabilityBindingSourceError::Invalid)?,
            )
            .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        for binding in catalog.snapshot.iter() {
            bindings
                .add(binding.clone())
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        }
        catalog.snapshot = bindings.build();
        catalog
            .availability_notices
            .extend((0..diagnostic_count).map(|_| {
                noema_capabilities::CapabilityAvailabilityNotice {
                    capability: None,
                    status: noema_capabilities::CapabilityAvailabilityStatus::Unavailable,
                }
            }));
        Ok(catalog)
    }

    /// Recoverably invalidate v1-v4 adapter state before v5 discovery.
    ///
    /// # Errors
    ///
    /// Returns a safe filesystem category if an exact rewrite cannot complete.
    pub fn prepare_filesystem(&self) -> Result<(), AdapterMigrationError> {
        let _guard = self
            .inner
            .migration_lock
            .lock()
            .map_err(|_| AdapterMigrationError::Unavailable)?;
        self.inner.connections.recover()?;
        self.inner.schedules.recover()?;
        self.inner.connections.upgrade_legacy_descriptors()?;
        let legacy = self.inner.definitions.legacy_definition_digests()?;
        self.inner.schedules.quarantine_referencing(&legacy)?;
        self.inner.connections.quarantine_referencing(&legacy)?;
        for digest in legacy {
            self.inner.definitions.quarantine(&digest)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn new_with_http_for_tests(
        paths: NoemaPaths,
        http: Arc<dyn AdapterHttpExecutor>,
    ) -> Self {
        Self::with_http(paths, http)
    }
}

fn require_current_operation(
    definition: &CompiledAdapterDefinition,
    tool_id: &str,
    source_revision: &str,
) -> Result<(), AdapterManagementError> {
    if definition.operations.iter().any(|operation| {
        operation.operation_id == tool_id && operation.operation_digest.as_str() == source_revision
    }) {
        Ok(())
    } else {
        Err(AdapterManagementError::Conflict)
    }
}

impl CapabilityBindingSource for AdapterCapabilityService {
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>> {
        let service = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || service.compile_catalog())
                .await
                .map_err(|_| CapabilityBindingSourceError::Unavailable)?
        })
    }
}

fn oauth_authority(human_id: &str, descriptor: &AdapterConnectionV3) -> AdapterOAuthAuthorityV1 {
    AdapterOAuthAuthorityV1 {
        human_id: human_id.to_string(),
        connection_id: descriptor.connection_id.clone(),
        account_id: descriptor.account_id.clone(),
        account_kind: descriptor.account_kind.clone(),
        semantic_digest: descriptor.semantic_digest.clone(),
        connection_revision: descriptor.revisions.connection,
        credential_revision: descriptor.revisions.credential,
        grant_revision: descriptor.revisions.grant,
        policy_revision: descriptor.revisions.policy,
    }
}

const fn map_oauth_error(error: AdapterOAuthError) -> AdapterOAuthSetupError {
    match error {
        AdapterOAuthError::Unsupported
        | AdapterOAuthError::InvalidInput
        | AdapterOAuthError::CallbackMismatch => AdapterOAuthSetupError::Invalid,
        AdapterOAuthError::Superseded => AdapterOAuthSetupError::Superseded,
        AdapterOAuthError::Expired => AdapterOAuthSetupError::Expired,
        AdapterOAuthError::ProviderDenied => AdapterOAuthSetupError::Denied,
        AdapterOAuthError::Unavailable => AdapterOAuthSetupError::Unavailable,
    }
}

const fn map_oauth_token_error(
    error: crate::network::AdapterOAuthTokenError,
) -> AdapterOAuthSetupError {
    match error {
        crate::network::AdapterOAuthTokenError::Rejected => AdapterOAuthSetupError::Denied,
        crate::network::AdapterOAuthTokenError::InvalidRequest
        | crate::network::AdapterOAuthTokenError::InvalidResponse
        | crate::network::AdapterOAuthTokenError::Unavailable => {
            AdapterOAuthSetupError::Unavailable
        }
    }
}

fn epoch_seconds() -> Result<u64, AdapterOAuthSetupError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| AdapterOAuthSetupError::Unavailable)
}

fn valid_connection_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests;
