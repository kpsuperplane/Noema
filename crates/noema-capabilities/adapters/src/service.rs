//! Root-contract adapter capability service.

use crate::{
    AdapterCatalogCompiler, AdapterCompiler, AdapterConnectionAuthenticationV1,
    AdapterConnectionStatus, AdapterConnectionStore, AdapterConnectionV4, AdapterDefinitionStore,
    AuthenticationMode, AuthorizationGrantStatus, AuthorizationGrantV1, CompiledAdapterDefinition,
    ExternalAccountV1, Oauth2CallbackMode, Oauth2ClientAuthentication, OauthApplicationStatus,
    OauthApplicationV1, OauthGrantTokenV1,
    credential_import::{AdapterCredentialImportError, setup_credential},
    network::{
        AdapterBearerCredential, AdapterHttpExecutor, AdapterOAuthTokenGrant,
        AdapterOAuthTokenRequest, ReqwestAdapterHttpExecutor,
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
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{RwLock, broadcast};

/// Safe failure from the human-owned adapter connection setup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdapterConnectionSetupError {
    /// The exact reviewed definition is absent or invalid.
    #[error("adapter definition is unavailable")]
    DefinitionUnavailable,
    /// The transient credential document does not match the reviewed schema.
    #[error("adapter credential document is invalid")]
    InvalidCredential,
    /// The OAuth client document is not valid JSON.
    #[error(
        "OAuth client document is not valid JSON. Download the JSON document from the provider and try again."
    )]
    InvalidOauthDocumentJson,
    /// The OAuth client document does not match the reviewed setup.
    #[error(
        "OAuth client document does not match this setup. Download the requested client type and try again."
    )]
    OauthDocumentMismatch,
    /// The hosted OAuth client omits the exact configured redirect URI.
    #[error(
        "OAuth client redirect URI does not match. Add the shown redirect URI, then download the document again."
    )]
    OauthRedirectMismatch,
    /// The OAuth client document exceeds the input limit.
    #[error(
        "OAuth client document exceeds 128 KB. Download the original JSON document and try again."
    )]
    OauthDocumentOversized,
    /// The same public client identity already has different protected details.
    #[error("An OAuth client with this client ID already exists with different client details.")]
    OauthClientConflict,
    /// A selected reusable grant revision changed before attachment.
    #[error("adapter account state changed")]
    Conflict,
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
    /// Immutable compiled definition registry and its diagnostics.
    pub definitions: Arc<crate::DefinitionScan>,
    /// Pending definitions hidden by a later proposal or approval.
    pub superseded_pending_digests: BTreeSet<String>,
    /// Earlier definition revisions replaced by reviewed definitions.
    pub replaced_definition_digests: BTreeSet<String>,
    /// Connection scan and its quarantine diagnostics.
    pub connections: crate::ConnectionScan,
    /// Complete reusable OAuth authority hierarchy.
    pub oauth_authorities: crate::OauthAuthoritySnapshot,
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

fn oauth_document_error(error: AdapterCredentialImportError) -> AdapterConnectionSetupError {
    match error {
        AdapterCredentialImportError::InvalidJson => {
            AdapterConnectionSetupError::InvalidOauthDocumentJson
        }
        AdapterCredentialImportError::RedirectMismatch => {
            AdapterConnectionSetupError::OauthRedirectMismatch
        }
        AdapterCredentialImportError::Oversized => {
            AdapterConnectionSetupError::OauthDocumentOversized
        }
        AdapterCredentialImportError::Unsupported
        | AdapterCredentialImportError::InvalidInput
        | AdapterCredentialImportError::InvalidDocument => {
            AdapterConnectionSetupError::OauthDocumentMismatch
        }
    }
}

pub(crate) fn compatible_authentication_replacement(
    current: &crate::AuthenticationSchemeV4,
    replacement: &crate::AuthenticationSchemeV4,
) -> bool {
    match (current, replacement) {
        (
            crate::AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(current),
            crate::AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(replacement),
        ) => current.profile_digest == replacement.profile_digest,
        _ => current == replacement,
    }
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
    /// Continuation cursors could not be quarantined safely.
    #[error("adapter cursor migration failed: {0}")]
    Cursor(#[from] crate::DurableCursorError),
    /// Reusable OAuth authority roots could not be prepared safely.
    #[error("adapter OAuth authority migration failed: {0}")]
    OauthAuthority(#[from] crate::OauthAuthorityStoreError),
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

/// One reviewed API selection within a shared OAuth request.
pub struct AdapterOAuthServiceSelection {
    /// Reviewed API definition included in this authorization.
    pub semantic_digest: String,
    /// Operations whose access must be covered.
    pub operation_ids: Vec<String>,
}

/// Exact non-secret request for a new account or added OAuth access.
pub struct AdapterOAuthAuthorizationRequest {
    /// Reusable OAuth application identity.
    pub application_id: String,
    /// Exact application revision selected by the caller.
    pub expected_application_revision: u64,
    /// Existing grant for access expansion or reconnection.
    pub grant_id: Option<String>,
    /// Exact grant authority revision selected by the caller.
    pub expected_grant_revision: Option<u64>,
    /// Reviewed API definition used to calculate required scopes.
    pub semantic_digest: String,
    /// Operations whose access must be covered.
    pub operation_ids: Vec<String>,
    /// Other reviewed APIs authorized by the same application and grant.
    pub additional_services: Vec<AdapterOAuthServiceSelection>,
    /// Callback mode registered by the application.
    pub callback_mode: Oauth2CallbackMode,
    /// Exact redirect URI for this attempt.
    pub redirect_uri: String,
}

/// Completed OAuth publication and the attempt that authorized it.
#[derive(Debug)]
pub struct AdapterOAuthSetupCompletion {
    /// Process-local attempt identity used by paused capability calls.
    pub attempt_id: String,
    /// Reviewed definition used to calculate requested access.
    pub semantic_digest: String,
    /// Canonical active authorization grant after token publication.
    pub grant: AuthorizationGrantV1,
    /// Stable account resolved by the reviewed profile, when available.
    pub account: Option<ExternalAccountV1>,
    /// Whether this callback authorized a new grant.
    pub newly_authorized: bool,
}

/// Process-local lifecycle of one browser OAuth attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterOAuthAttemptStatus {
    /// Browser authorization can continue.
    Authorizing,
    /// Authorization and token publication completed.
    Completed,
    /// The human or provider denied authorization.
    Denied,
    /// The bounded browser attempt expired.
    Expired,
    /// A newer exact-grant attempt replaced this attempt.
    Superseded,
    /// The attempt failed without changing current grant access.
    Failed,
}

/// Non-secret OAuth attempt event for client recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterOAuthAttemptEvent {
    /// Stable process-local attempt identity.
    pub attempt_id: String,
    /// Exact definition that requested authorization.
    pub semantic_digest: Option<String>,
    /// Canonical grant produced or expanded after completion.
    pub grant_id: Option<String>,
    /// Exact grant authority revision after completion.
    pub grant_revision: Option<u64>,
    /// Current attempt lifecycle state.
    pub status: AdapterOAuthAttemptStatus,
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
    definition_registry: Mutex<Option<Arc<crate::DefinitionScan>>>,
    pub(crate) connections: AdapterConnectionStore,
    pub(crate) oauth_authorities: crate::OauthAuthorityStore,
    pub(crate) cursors: crate::DurableCursorStore,
    schedules: crate::ScheduleStore,
    transitions: crate::transition::DefinitionTransitionJournalStore,
    pub(crate) oauth_callback_mode: Mutex<Option<Oauth2CallbackMode>>,
    migration_lock: Mutex<()>,
    pub(crate) definition_lock: Mutex<()>,
    connection_locks: Mutex<BTreeMap<String, Arc<RwLock<()>>>>,
    pub(crate) cursor_lock: tokio::sync::Mutex<()>,
    oauth_attempts: Mutex<AdapterOAuthAttemptRegistry>,
    oauth_attempt_statuses: Mutex<BTreeMap<String, AdapterOAuthAttemptEvent>>,
    oauth_attempt_events: broadcast::Sender<AdapterOAuthAttemptEvent>,
    pub(crate) http: Arc<dyn AdapterHttpExecutor>,
}

/// Filesystem-backed binding source and credentialed JSON invoker.
#[derive(Clone)]
pub struct AdapterCapabilityService {
    pub(crate) inner: Arc<AdapterCapabilityServiceInner>,
}

struct LoadedOAuthGrant {
    definition: CompiledAdapterDefinition,
    profile: crate::OauthProfileV1,
    profile_digest: String,
    application: OauthApplicationV1,
    application_credential: crate::OauthApplicationCredentialV1,
    grant: Option<AuthorizationGrantV1>,
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
        let definitions = AdapterDefinitionStore::new(paths.clone());
        let definition_registry = definitions.scan().ok().map(Arc::new);
        let (oauth_attempt_events, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(AdapterCapabilityServiceInner {
                definitions,
                definition_registry: Mutex::new(definition_registry),
                connections: AdapterConnectionStore::new(paths.clone()),
                oauth_authorities: crate::OauthAuthorityStore::new(paths.clone()),
                cursors: crate::DurableCursorStore::new(paths.clone()),
                schedules: crate::ScheduleStore::new(paths.clone()),
                transitions: crate::transition::DefinitionTransitionJournalStore::new(paths),
                oauth_callback_mode: Mutex::new(None),
                migration_lock: Mutex::new(()),
                definition_lock: Mutex::new(()),
                connection_locks: Mutex::new(BTreeMap::new()),
                cursor_lock: tokio::sync::Mutex::new(()),
                oauth_attempts: Mutex::new(AdapterOAuthAttemptRegistry::default()),
                oauth_attempt_statuses: Mutex::new(BTreeMap::new()),
                oauth_attempt_events,
                http,
            }),
        }
    }

    pub(crate) fn definition_registry(
        &self,
    ) -> Result<Arc<crate::DefinitionScan>, crate::DefinitionStoreError> {
        self.inner
            .definition_registry
            .lock()
            .map_err(|_| crate::DefinitionStoreError::Integrity("definition_registry"))?
            .clone()
            .ok_or(crate::DefinitionStoreError::Integrity(
                "definition_registry_unavailable",
            ))
    }

    pub(crate) fn refresh_definition_registry(
        &self,
    ) -> Result<Arc<crate::DefinitionScan>, crate::DefinitionStoreError> {
        let definitions = Arc::new(self.inner.definitions.scan()?);
        *self
            .inner
            .definition_registry
            .lock()
            .map_err(|_| crate::DefinitionStoreError::Integrity("definition_registry"))? =
            Some(definitions.clone());
        Ok(definitions)
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

    /// Return the latest process-local lifecycle state for one OAuth attempt.
    #[must_use]
    pub fn oauth_attempt_status(&self, attempt_id: &str) -> Option<AdapterOAuthAttemptEvent> {
        self.inner
            .oauth_attempt_statuses
            .lock()
            .ok()?
            .get(attempt_id)
            .cloned()
    }

    /// Subscribe to process-local OAuth attempt lifecycle events.
    #[must_use]
    pub fn subscribe_oauth_attempts(&self) -> broadcast::Receiver<AdapterOAuthAttemptEvent> {
        self.inner.oauth_attempt_events.subscribe()
    }

    fn record_oauth_attempt_status(&self, event: AdapterOAuthAttemptEvent) {
        if let Ok(mut statuses) = self.inner.oauth_attempt_statuses.lock() {
            if statuses.len() >= 64
                && !statuses.contains_key(&event.attempt_id)
                && let Some(oldest) = statuses.keys().next().cloned()
            {
                statuses.remove(&oldest);
            }
            statuses.insert(event.attempt_id.clone(), event.clone());
        }
        let _ = self.inner.oauth_attempt_events.send(event);
    }

    fn transition_oauth_attempt_status(
        &self,
        attempt_id: &str,
        status: AdapterOAuthAttemptStatus,
        grant_id: Option<String>,
        grant_revision: Option<u64>,
    ) {
        let current = self.oauth_attempt_status(attempt_id);
        self.record_oauth_attempt_status(AdapterOAuthAttemptEvent {
            attempt_id: attempt_id.to_string(),
            semantic_digest: current
                .as_ref()
                .and_then(|event| event.semantic_digest.clone()),
            grant_id: grant_id
                .or_else(|| current.as_ref().and_then(|event| event.grant_id.clone())),
            grant_revision: grant_revision
                .or_else(|| current.as_ref().and_then(|event| event.grant_revision)),
            status,
        });
    }

    fn expire_oauth_attempt(&self, attempt_id: &str, now_epoch_seconds: u64) {
        let expired = self
            .inner
            .oauth_attempts
            .lock()
            .is_ok_and(|mut attempts| attempts.expire_attempt(attempt_id, now_epoch_seconds));
        if expired {
            self.transition_oauth_attempt_status(
                attempt_id,
                AdapterOAuthAttemptStatus::Expired,
                None,
                None,
            );
        }
    }

    /// Read the current non-secret adapter management hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a safe category when canonical filesystem state is unavailable.
    pub fn management_snapshot(&self) -> Result<AdapterManagementSnapshot, AdapterManagementError> {
        let definitions = self
            .definition_registry()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let superseded_pending_digests = definitions.superseded_pending_digests.clone();
        let replaced_definition_digests = definitions.replaced_definition_digests.clone();
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let oauth_authorities = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(AdapterManagementSnapshot {
            definitions,
            superseded_pending_digests,
            replaced_definition_digests,
            connections,
            oauth_authorities,
        })
    }

    pub(crate) fn plan_definition_transition(
        &self,
        replacement: &CompiledAdapterDefinition,
        replaces_semantic_digests: &[String],
    ) -> Result<crate::AdapterDefinitionTransition, AdapterManagementError> {
        let mut lineage = BTreeSet::new();
        let mut unvisited = replaces_semantic_digests.to_vec();
        let mut current = None;
        while let Some(digest) = unvisited.pop() {
            if !lineage.insert(digest.clone()) {
                continue;
            }
            let ancestor = self
                .inner
                .definitions
                .load(&digest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
            if current.is_none() && ancestor.manifest.reviewed {
                current = Some(
                    AdapterCompiler::compile(&ancestor.manifest)
                        .map_err(|_| AdapterManagementError::Unavailable)?,
                );
            }
            unvisited.extend(ancestor.provenance.replaces_semantic_digests);
        }
        let mut transition = current
            .as_ref()
            .map_or_else(crate::AdapterDefinitionTransition::default, |current| {
                crate::AdapterDefinitionTransition::between(current, replacement)
            });
        let snapshot = self.management_snapshot()?;
        let affected = snapshot
            .connections
            .connections
            .iter()
            .filter(|connection| lineage.contains(&connection.descriptor.semantic_digest))
            .collect::<Vec<_>>();
        transition.affected_connections = affected.len();
        transition.authentication_required_connections = affected
            .iter()
            .filter(|connection| {
                snapshot
                    .definitions
                    .definitions
                    .iter()
                    .find(|definition| {
                        definition.compiled.semantic_digest.as_str()
                            == connection.descriptor.semantic_digest
                    })
                    .is_some_and(|definition| {
                        !compatible_authentication_replacement(
                            &definition.compiled.authentication,
                            &replacement.authentication,
                        )
                    })
            })
            .count();
        let mut grants = BTreeMap::<String, usize>::new();
        for connection in affected {
            if let AdapterConnectionAuthenticationV1::OauthGrant { grant_id } =
                &connection.descriptor.authentication
            {
                *grants.entry(grant_id.clone()).or_default() += 1;
            }
        }
        transition.consolidated_connections =
            grants.values().map(|count| count.saturating_sub(1)).sum();
        transition.affected_schedules = self
            .inner
            .schedules
            .scan()
            .map_err(|_| AdapterManagementError::Unavailable)?
            .iter()
            .filter(|schedule| lineage.contains(&schedule.schedule.semantic_digest))
            .count();
        Ok(transition)
    }

    fn definition_is_current_reviewed(&self, semantic_digest: &str) -> bool {
        let Ok(scan) = self.definition_registry() else {
            return false;
        };
        !scan.replaced_definition_digests.contains(semantic_digest)
            && scan.definitions.iter().any(|definition| {
                definition.compiled.semantic_digest.as_str() == semantic_digest
                    && definition.compiled.reviewed
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
        let reviewed = self.review_definition_without_refresh(semantic_digest)?;
        self.refresh_definition_registry()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(reviewed)
    }

    fn review_definition_without_refresh(
        &self,
        semantic_digest: &str,
    ) -> Result<crate::DefinitionInstall, AdapterManagementError> {
        let _guard = self
            .inner
            .definition_lock
            .lock()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let definitions = self
            .definition_registry()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        if definitions
            .superseded_pending_digests
            .contains(semantic_digest)
        {
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
        let reviewed = self
            .inner
            .definitions
            .install_with_provenance(&reviewed, stored.provenance, source)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(reviewed)
    }

    /// Review one pending definition and move compatible connections from its
    /// exact replacement lineage onto the newly reviewed authority.
    ///
    /// # Errors
    ///
    /// Returns a safe management error when the definition is absent, stale,
    /// incompatible canonical state cannot be read, or a fenced replacement
    /// cannot be published.
    pub async fn review_definition_and_adopt(
        &self,
        semantic_digest: &str,
    ) -> Result<crate::DefinitionInstall, AdapterManagementError> {
        let target = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterManagementError::NotFound)?;
        let family_lock = self
            .connection_lock(&format!("adapter-family:{}", target.manifest.definition_id))
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _family_guard = family_lock.write().await;
        if !target.manifest.reviewed
            && let Some(expected) = &target.provenance.transition
        {
            let replacement = AdapterCompiler::compile(&target.manifest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
            let current = self.plan_definition_transition(
                &replacement,
                &target.provenance.replaces_semantic_digests,
            )?;
            if current != *expected {
                return Err(AdapterManagementError::Conflict);
            }
        }
        let mut journal = crate::transition::DefinitionTransitionJournal {
            schema_version: 1,
            definition_id: target.manifest.definition_id.clone(),
            requested_digest: semantic_digest.to_string(),
            reviewed_digest: target
                .manifest
                .reviewed
                .then(|| semantic_digest.to_string()),
        };
        self.inner
            .transitions
            .save(&journal)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let reviewed = if target.manifest.reviewed {
            self.inner
                .definitions
                .scan()
                .map_err(|_| AdapterManagementError::Unavailable)?
                .definitions
                .into_iter()
                .find(|definition| definition.compiled.semantic_digest.as_str() == semantic_digest)
                .ok_or(AdapterManagementError::NotFound)?
        } else {
            self.review_definition_without_refresh(semantic_digest)?
        };
        journal.reviewed_digest = Some(reviewed.compiled.semantic_digest.to_string());
        self.inner
            .transitions
            .save(&journal)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let stored = self
            .inner
            .definitions
            .load(reviewed.compiled.semantic_digest.as_str())
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let mut replacement_lineage = BTreeMap::<String, usize>::new();
        let mut unvisited = stored
            .provenance
            .replaces_semantic_digests
            .into_iter()
            .map(|digest| (digest, 1_usize))
            .collect::<Vec<_>>();
        while let Some((replaced_digest, depth)) = unvisited.pop() {
            if replacement_lineage
                .get(&replaced_digest)
                .is_some_and(|current| *current >= depth)
            {
                continue;
            }
            replacement_lineage.insert(replaced_digest.clone(), depth);
            let ancestor = self
                .inner
                .definitions
                .load(&replaced_digest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
            unvisited.extend(
                ancestor
                    .provenance
                    .replaces_semantic_digests
                    .into_iter()
                    .map(|digest| (digest, depth.saturating_add(1))),
            );
        }
        let snapshot = self.management_snapshot()?;
        let mut preferred_by_grant = BTreeMap::<String, (usize, String)>::new();
        for connection in &snapshot.connections.connections {
            let Some(depth) = replacement_lineage.get(&connection.descriptor.semantic_digest)
            else {
                continue;
            };
            let AdapterConnectionAuthenticationV1::OauthGrant { grant_id } =
                &connection.descriptor.authentication
            else {
                continue;
            };
            let candidate = (*depth, connection.descriptor.connection_id.clone());
            if preferred_by_grant
                .get(grant_id)
                .is_none_or(|current| candidate.0 > current.0 || candidate < *current)
            {
                preferred_by_grant.insert(grant_id.clone(), candidate);
            }
        }
        for replaced_digest in replacement_lineage.keys() {
            let current_definition = self
                .inner
                .definitions
                .load(replaced_digest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
            let current_definition = AdapterCompiler::compile(&current_definition.manifest)
                .map_err(|_| AdapterManagementError::Unavailable)?;
            let snapshot = self.management_snapshot()?;
            let connection_ids = snapshot
                .connections
                .connections
                .iter()
                .filter(|connection| connection.descriptor.semantic_digest == *replaced_digest)
                .map(|connection| connection.descriptor.connection_id.clone())
                .collect::<Vec<_>>();
            for connection_id in connection_ids {
                let lock = self
                    .connection_lock(&connection_id)
                    .map_err(|_| AdapterManagementError::Unavailable)?;
                let _guard = lock.write().await;
                let snapshot = self.management_snapshot()?;
                let Some(current) = snapshot.connections.connections.iter().find(|connection| {
                    connection.descriptor.connection_id == connection_id
                        && connection.descriptor.semantic_digest == *replaced_digest
                }) else {
                    continue;
                };
                if !compatible_authentication_replacement(
                    &current_definition.authentication,
                    &reviewed.compiled.authentication,
                ) {
                    self.inner
                        .connections
                        .require_new_authentication(
                            &current.descriptor,
                            &current_definition,
                            &reviewed.compiled,
                        )
                        .map_err(|_| AdapterManagementError::Unavailable)?;
                    for schedule in self
                        .inner
                        .schedules
                        .scan()
                        .map_err(|_| AdapterManagementError::Unavailable)?
                        .into_iter()
                        .filter(|schedule| {
                            schedule.schedule.connection_id == connection_id
                                && schedule.schedule.semantic_digest == *replaced_digest
                        })
                    {
                        self.inner
                            .schedules
                            .revoke(&schedule.schedule.schedule_id)
                            .map_err(|_| AdapterManagementError::Unavailable)?;
                    }
                    continue;
                }
                let unchanged_operations = current_definition
                    .operations
                    .iter()
                    .filter(|operation| {
                        reviewed.compiled.operations.iter().any(|replacement| {
                            replacement.operation_id == operation.operation_id
                                && replacement.operation_digest == operation.operation_digest
                        })
                    })
                    .map(|operation| operation.operation_id.clone())
                    .collect();
                self.inner
                    .schedules
                    .migrate_connection_references(
                        replaced_digest,
                        reviewed.compiled.semantic_digest.as_str(),
                        &connection_id,
                        &connection_id,
                        &unchanged_operations,
                        &self.inner.cursors,
                    )
                    .map_err(|_| AdapterManagementError::Unavailable)?;
                self.inner
                    .connections
                    .rebind_definition_descriptor(
                        &current.descriptor,
                        &current_definition,
                        &reviewed.compiled,
                    )
                    .map_err(|_| AdapterManagementError::Unavailable)?;
            }
        }
        self.consolidate_oauth_family(&reviewed.compiled, &preferred_by_grant)
            .await?;
        self.refresh_definition_registry()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        self.inner
            .transitions
            .remove(&journal.requested_digest)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(reviewed)
    }

    /// Resume only definition transitions that own a durable incomplete journal.
    ///
    /// # Errors
    ///
    /// Returns a migration error when a journal or its canonical adapter state cannot be recovered.
    pub async fn resume_definition_transitions(&self) -> Result<(), AdapterMigrationError> {
        for journal in self.inner.transitions.scan()? {
            let digest = if let Some(reviewed_digest) = journal.reviewed_digest.as_deref() {
                reviewed_digest.to_string()
            } else {
                let stored = self.inner.definitions.load(&journal.requested_digest)?;
                let mut reviewed = stored.manifest;
                reviewed.reviewed = true;
                let predicted = AdapterCompiler::compile(&reviewed)
                    .map_err(crate::DefinitionStoreError::Compile)?
                    .semantic_digest
                    .to_string();
                if self.inner.definitions.load(&predicted).is_ok() {
                    predicted
                } else {
                    journal.requested_digest.clone()
                }
            };
            self.review_definition_and_adopt(&digest)
                .await
                .map_err(|_| AdapterMigrationError::Unavailable)?;
            self.inner.transitions.remove(&journal.requested_digest)?;
        }
        Ok(())
    }

    async fn consolidate_oauth_family(
        &self,
        definition: &CompiledAdapterDefinition,
        preferred_by_grant: &BTreeMap<String, (usize, String)>,
    ) -> Result<(), AdapterManagementError> {
        let snapshot = self.management_snapshot()?;
        let mut by_grant = BTreeMap::<String, Vec<String>>::new();
        for connection in snapshot
            .connections
            .connections
            .iter()
            .filter(|connection| {
                connection.descriptor.semantic_digest == definition.semantic_digest.as_str()
            })
        {
            if let AdapterConnectionAuthenticationV1::OauthGrant { grant_id } =
                &connection.descriptor.authentication
            {
                by_grant
                    .entry(grant_id.clone())
                    .or_default()
                    .push(connection.descriptor.connection_id.clone());
            }
        }
        let unchanged_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect();
        for (grant_id, connection_ids) in &mut by_grant {
            connection_ids.sort();
            let Some(survivor_id) = preferred_by_grant
                .get(grant_id)
                .map(|(_, connection_id)| connection_id.clone())
                .filter(|connection_id| connection_ids.contains(connection_id))
                .or_else(|| connection_ids.first().cloned())
            else {
                continue;
            };
            for redundant_id in connection_ids
                .iter()
                .filter(|connection_id| **connection_id != survivor_id)
            {
                self.inner
                    .connections
                    .merge_oauth_connections(&survivor_id, redundant_id, definition)
                    .map_err(|_| AdapterManagementError::Unavailable)?;
                self.inner
                    .schedules
                    .migrate_connection_references(
                        definition.semantic_digest.as_str(),
                        definition.semantic_digest.as_str(),
                        redundant_id,
                        &survivor_id,
                        &unchanged_operations,
                        &self.inner.cursors,
                    )
                    .map_err(|_| AdapterManagementError::Unavailable)?;
                self.inner
                    .connections
                    .quarantine(redundant_id)
                    .map_err(|_| AdapterManagementError::Unavailable)?;
            }
        }
        Ok(())
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
        let definitions = self
            .definition_registry()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        if definitions
            .superseded_pending_digests
            .contains(semantic_digest)
        {
            return Err(AdapterManagementError::Conflict);
        }
        let Some(target) = definitions
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
        let mut pending = definitions
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
        self.refresh_definition_registry()
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
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == connection_id)
            .ok_or(AdapterManagementError::NotFound)?;
        if current.descriptor.connection_revision != expected_connection_revision
            || current.descriptor.connection_label != expected_connection_label
        {
            return Err(AdapterManagementError::Conflict);
        }
        let definition = snapshot
            .definitions
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
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == fence.connection_id)
            .ok_or(AdapterManagementError::NotFound)?;
        let definition = snapshot
            .definitions
            .definitions
            .iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == current.descriptor.semantic_digest
            })
            .ok_or(AdapterManagementError::Unavailable)?;
        if current.descriptor.connection_revision != fence.expected_connection_revision
            || current.descriptor.policy_revision != fence.expected_policy_revision
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
        replacement.connection_revision = replacement
            .connection_revision
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        replacement.policy_revision = replacement
            .policy_revision
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        if let Some(policy) = replacement.policy.as_mut() {
            policy.revision = replacement.policy_revision;
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
        replacement_connection_id: Option<&str>,
        field_values: BTreeMap<String, String>,
        document: Option<&[u8]>,
    ) -> Result<crate::ConnectionInstall, AdapterConnectionSetupError> {
        crate::SemanticDigest::parse(semantic_digest.to_string())
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        if !self.definition_is_current_reviewed(semantic_digest) {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
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
        if definition.authentication.mode() != AuthenticationMode::Credential {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
        let credential = setup_credential(&definition, field_values, document, generation_id)
            .map_err(|_| AdapterConnectionSetupError::InvalidCredential)?;
        if let Some(connection_id) = replacement_connection_id {
            let snapshot = self
                .management_snapshot()
                .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
            let current = snapshot
                .connections
                .connections
                .iter()
                .find(|connection| connection.descriptor.connection_id == connection_id)
                .ok_or(AdapterConnectionSetupError::Conflict)?;
            let credential_revision = match &current.descriptor.authentication {
                AdapterConnectionAuthenticationV1::Credential { revision, .. } => revision
                    .checked_add(1)
                    .ok_or(AdapterConnectionSetupError::Conflict)?,
                AdapterConnectionAuthenticationV1::Pending
                | AdapterConnectionAuthenticationV1::None
                | AdapterConnectionAuthenticationV1::OauthGrant { .. } => 1,
            };
            return self
                .inner
                .connections
                .replace_authentication(
                    &current.descriptor,
                    AdapterConnectionAuthenticationV1::Credential {
                        generation_id: credential.generation_id.clone(),
                        revision: credential_revision,
                    },
                    Some(&credential),
                    &definition,
                )
                .map_err(|_| AdapterConnectionSetupError::Conflict);
        }
        let status = AdapterConnectionStatus::Active;
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV4 {
            schema_version: 4,
            connection_id,
            connection_slug,
            semantic_digest: semantic_digest.to_string(),
            connection_label: None,
            status,
            connection_revision: 1,
            policy_revision: 1,
            authentication: AdapterConnectionAuthenticationV1::Credential {
                generation_id: credential.generation_id.clone(),
                revision: 1,
            },
            allowed_operations,
            policy: None,
            tool_overrides: Vec::new(),
        };
        self.inner
            .connections
            .install(&descriptor, Some(&credential), &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Import one reusable OAuth application from a transient client document.
    ///
    /// # Errors
    ///
    /// Returns a safe category when setup input or canonical state is invalid.
    pub async fn import_oauth_application(
        &self,
        profile_digest: &str,
        callback_mode: Oauth2CallbackMode,
        redirect_uri: &str,
        document: &[u8],
        project_label: Option<String>,
    ) -> Result<OauthApplicationV1, AdapterConnectionSetupError> {
        let configured_mode = self
            .inner
            .oauth_callback_mode
            .lock()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?
            .ok_or(AdapterConnectionSetupError::DefinitionUnavailable)?;
        if configured_mode != callback_mode {
            return Err(AdapterConnectionSetupError::OauthDocumentMismatch);
        }
        let profile = self
            .inner
            .oauth_authorities
            .load_profile(profile_digest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let generation_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let (client_id, credential) = crate::credential_import::setup_oauth_application(
            &profile.profile,
            callback_mode,
            redirect_uri,
            document,
            generation_id,
        )
        .map_err(oauth_document_error)?;
        let lock = self
            .connection_lock(&format!("oauth-application:{profile_digest}:{client_id}"))
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        if let Some(existing) = snapshot.applications.into_iter().find(|application| {
            application.profile_digest == profile_digest
                && application.callback_mode == callback_mode
                && application.client_id == client_id
        }) {
            let (_, current_credential) = self
                .inner
                .oauth_authorities
                .load_application_authority(&existing.application_id)
                .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
            if current_credential.client_secret == credential.client_secret {
                return Ok(existing);
            }
            return Err(AdapterConnectionSetupError::OauthClientConflict);
        }
        let application = OauthApplicationV1 {
            schema_version: 1,
            application_id: random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?,
            profile_digest: profile_digest.to_string(),
            callback_mode,
            client_id,
            project_label,
            credential_generation: credential.generation_id.clone(),
            revision: 1,
            status: OauthApplicationStatus::Active,
        };
        self.inner
            .oauth_authorities
            .install_application(&application, &credential)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Replace one OAuth application client document under an exact revision.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority changed or publication fails.
    pub async fn replace_oauth_application(
        &self,
        application_id: &str,
        expected_revision: u64,
        redirect_uri: &str,
        document: &[u8],
    ) -> Result<OauthApplicationV1, AdapterManagementError> {
        let lock = self
            .connection_lock(&format!("oauth-application:{application_id}"))
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let (current, _) = self
            .inner
            .oauth_authorities
            .load_application_authority(application_id)
            .map_err(|_| AdapterManagementError::NotFound)?;
        if current.revision != expected_revision {
            return Err(AdapterManagementError::Conflict);
        }
        let profile = self
            .inner
            .oauth_authorities
            .load_profile(&current.profile_digest)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let generation_id = random_hex(16).map_err(|_| AdapterManagementError::Unavailable)?;
        let (client_id, credential) = crate::credential_import::setup_oauth_application(
            &profile.profile,
            current.callback_mode,
            redirect_uri,
            document,
            generation_id,
        )
        .map_err(|_| AdapterManagementError::Invalid)?;
        if client_id != current.client_id {
            return Err(AdapterManagementError::Invalid);
        }
        let mut replacement = current.clone();
        replacement.credential_generation = credential.generation_id.clone();
        replacement.revision = replacement
            .revision
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        self.inner
            .oauth_authorities
            .replace_application_credential(&current, &replacement, &credential)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    /// Disconnect one account grant while retaining its public identity.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority changed or publication fails.
    pub async fn disconnect_oauth_grant(
        &self,
        grant_id: &str,
        expected_authority_revision: u64,
    ) -> Result<AuthorizationGrantV1, AdapterManagementError> {
        let lock = self
            .connection_lock(&format!("oauth-grant:{grant_id}"))
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let (grant, _) = self
            .inner
            .oauth_authorities
            .load_grant_authority(grant_id)
            .map_err(|_| AdapterManagementError::NotFound)?;
        if grant.authority_revision != expected_authority_revision {
            return Err(AdapterManagementError::Conflict);
        }
        self.inner
            .oauth_authorities
            .deactivate_grant(&grant, AuthorizationGrantStatus::Revoked)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    /// Save a human label for a grant without stable provider identity.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority changed or publication fails.
    pub async fn save_oauth_grant_label(
        &self,
        grant_id: &str,
        expected_authority_revision: u64,
        account_label: Option<String>,
    ) -> Result<AuthorizationGrantV1, AdapterManagementError> {
        let account_label =
            noema_capabilities::normalize_capability_connection_label(account_label)
                .map_err(|_| AdapterManagementError::Invalid)?;
        let lock = self
            .connection_lock(&format!("oauth-grant:{grant_id}"))
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let (grant, _) = self
            .inner
            .oauth_authorities
            .load_grant_authority(grant_id)
            .map_err(|_| AdapterManagementError::NotFound)?;
        if grant.authority_revision != expected_authority_revision {
            return Err(AdapterManagementError::Conflict);
        }
        self.inner
            .oauth_authorities
            .save_grant_label(&grant, account_label)
            .map_err(|_| AdapterManagementError::Unavailable)
    }

    /// Delete one OAuth application when no grant still references it.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority changed or dependencies remain.
    pub async fn delete_oauth_application(
        &self,
        application_id: &str,
        expected_revision: u64,
    ) -> Result<bool, AdapterManagementError> {
        let lock = self
            .connection_lock(&format!("oauth-application:{application_id}"))
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let application = snapshot
            .applications
            .iter()
            .find(|application| application.application_id == application_id)
            .ok_or(AdapterManagementError::NotFound)?;
        if application.revision != expected_revision {
            return Err(AdapterManagementError::Conflict);
        }
        if snapshot
            .grants
            .iter()
            .any(|grant| grant.application_id == application_id)
        {
            return Err(AdapterManagementError::Conflict);
        }
        self.inner
            .oauth_authorities
            .quarantine_application(application)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        Ok(true)
    }

    /// Attach one reviewed API definition to an existing OAuth grant.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the definition and grant are incompatible.
    pub async fn attach_oauth_connection(
        &self,
        semantic_digest: &str,
        grant_id: &str,
        expected_grant_revision: u64,
        replacement_connection_id: Option<&str>,
    ) -> Result<crate::ConnectionInstall, AdapterConnectionSetupError> {
        if !self.definition_is_current_reviewed(semantic_digest) {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
        let definition = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let definition = AdapterCompiler::compile(&definition.manifest)
            .map_err(|_| AdapterConnectionSetupError::DefinitionUnavailable)?;
        let profile_digest = definition
            .authentication
            .oauth2()
            .map(|oauth| oauth.profile_digest.as_str())
            .ok_or(AdapterConnectionSetupError::DefinitionUnavailable)?;
        let (grant, _) = self
            .inner
            .oauth_authorities
            .load_grant_authority(grant_id)
            .map_err(|_| AdapterConnectionSetupError::InvalidCredential)?;
        if grant.authority_revision != expected_grant_revision {
            return Err(AdapterConnectionSetupError::Conflict);
        }
        let application = self
            .inner
            .oauth_authorities
            .load_application(&grant.application_id)
            .map_err(|_| AdapterConnectionSetupError::InvalidCredential)?;
        if application.profile_digest != profile_digest {
            return Err(AdapterConnectionSetupError::InvalidCredential);
        }
        let lock = self
            .connection_lock(&format!("adapter-family:{}", definition.definition_id))
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self
            .management_snapshot()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        if let Some(connection_id) = replacement_connection_id {
            let current = snapshot
                .connections
                .connections
                .iter()
                .find(|connection| connection.descriptor.connection_id == connection_id)
                .ok_or(AdapterConnectionSetupError::Conflict)?;
            return self
                .inner
                .connections
                .replace_authentication(
                    &current.descriptor,
                    AdapterConnectionAuthenticationV1::OauthGrant {
                        grant_id: grant_id.to_string(),
                    },
                    None,
                    &definition,
                )
                .map_err(|_| AdapterConnectionSetupError::Conflict);
        }
        let family_digests = snapshot
            .definitions
            .definitions
            .iter()
            .filter(|candidate| candidate.compiled.definition_id == definition.definition_id)
            .map(|candidate| candidate.compiled.semantic_digest.to_string())
            .collect::<BTreeSet<_>>();
        let mut matches = snapshot
            .connections
            .connections
            .into_iter()
            .filter(|connection| {
                family_digests.contains(&connection.descriptor.semantic_digest)
                    && matches!(
                        &connection.descriptor.authentication,
                        AdapterConnectionAuthenticationV1::OauthGrant { grant_id: current }
                            if current == grant_id
                    )
            });
        if let Some(existing) = matches.next() {
            if matches.next().is_some() || existing.descriptor.semantic_digest != semantic_digest {
                return Err(AdapterConnectionSetupError::Conflict);
            }
            return Ok(existing);
        }
        let connection_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV4 {
            schema_version: 4,
            connection_slug: format!("personal-{}", &connection_id[..8]),
            connection_id,
            semantic_digest: semantic_digest.to_string(),
            connection_label: None,
            status: AdapterConnectionStatus::Active,
            connection_revision: 1,
            policy_revision: 1,
            authentication: AdapterConnectionAuthenticationV1::OauthGrant {
                grant_id: grant_id.to_string(),
            },
            allowed_operations,
            policy: None,
            tool_overrides: Vec::new(),
        };
        self.inner
            .connections
            .install(&descriptor, None, &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Suspend or resume one connection without changing its grant.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority changed or publication fails.
    pub async fn set_connection_active(
        &self,
        connection_id: &str,
        expected_connection_revision: u64,
        active: bool,
    ) -> Result<crate::ConnectionInstall, AdapterManagementError> {
        let lock = self
            .connection_lock(connection_id)
            .map_err(|_| AdapterManagementError::Unavailable)?;
        let _guard = lock.write().await;
        let snapshot = self.management_snapshot()?;
        let current = snapshot
            .connections
            .connections
            .iter()
            .find(|connection| connection.descriptor.connection_id == connection_id)
            .ok_or(AdapterManagementError::NotFound)?;
        if current.descriptor.connection_revision != expected_connection_revision {
            return Err(AdapterManagementError::Conflict);
        }
        let definition = snapshot
            .definitions
            .definitions
            .iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == current.descriptor.semantic_digest
            })
            .ok_or(AdapterManagementError::Unavailable)?;
        let mut replacement = current.descriptor.clone();
        replacement.status = if active {
            AdapterConnectionStatus::Active
        } else {
            AdapterConnectionStatus::Suspended
        };
        if replacement.status == current.descriptor.status {
            return Ok(current.clone());
        }
        replacement.connection_revision = replacement
            .connection_revision
            .checked_add(1)
            .ok_or(AdapterManagementError::Conflict)?;
        self.inner
            .connections
            .replace_management_descriptor(&current.descriptor, &replacement, &definition.compiled)
            .map_err(|_| AdapterManagementError::Unavailable)
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
        if !self.definition_is_current_reviewed(semantic_digest) {
            return Err(AdapterConnectionSetupError::DefinitionUnavailable);
        }
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
        let snapshot = self
            .management_snapshot()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let family_digests = snapshot
            .definitions
            .definitions
            .iter()
            .filter(|candidate| candidate.compiled.definition_id == definition.definition_id)
            .map(|candidate| candidate.compiled.semantic_digest.to_string())
            .collect::<BTreeSet<_>>();
        let mut matches = snapshot
            .connections
            .connections
            .into_iter()
            .filter(|connection| {
                family_digests.contains(&connection.descriptor.semantic_digest)
                    && matches!(
                        &connection.descriptor.authentication,
                        AdapterConnectionAuthenticationV1::None
                    )
            });
        if let Some(connection) = matches.next() {
            if matches.next().is_some() || connection.descriptor.semantic_digest != semantic_digest
            {
                return Err(AdapterConnectionSetupError::Conflict);
            }
            return Ok(connection);
        }
        let connection_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV4 {
            schema_version: 4,
            connection_slug: format!("personal-{}", &connection_id[..8]),
            connection_id,
            semantic_digest: semantic_digest.to_string(),
            connection_label: None,
            status: AdapterConnectionStatus::Active,
            connection_revision: 1,
            policy_revision: 1,
            authentication: AdapterConnectionAuthenticationV1::None,
            allowed_operations,
            policy: None,
            tool_overrides: Vec::new(),
        };
        self.inner
            .connections
            .install(&descriptor, None, &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
    }

    /// Start a new-account, reconnect, or added-access OAuth attempt.
    ///
    /// # Errors
    ///
    /// Returns a safe category when authority or setup state is invalid.
    pub async fn start_oauth_authorization(
        &self,
        human_id: &str,
        request: AdapterOAuthAuthorizationRequest,
    ) -> Result<AdapterOAuthSetupStart, AdapterOAuthSetupError> {
        let now_epoch_seconds = epoch_seconds()?;
        let lock_id = request
            .grant_id
            .as_deref()
            .unwrap_or(&request.application_id);
        let lock = self
            .connection_lock(&format!("oauth-grant:{lock_id}"))
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let _guard = lock.write().await;
        let current = self.load_oauth_grant(
            &request.semantic_digest,
            &request.application_id,
            request.grant_id.as_deref(),
        )?;
        if current.application.revision != request.expected_application_revision
            || current.grant.as_ref().map(|grant| grant.authority_revision)
                != request.expected_grant_revision
        {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        if current.application.status != OauthApplicationStatus::Active
            || current.application.callback_mode != request.callback_mode
            || (current.profile.client_authentication != Oauth2ClientAuthentication::None
                && current.application_credential.client_secret.is_none())
        {
            return Err(AdapterOAuthSetupError::Invalid);
        }
        let granted = current
            .grant
            .as_ref()
            .map_or(&[][..], |grant| grant.granted_scopes.as_slice());
        let mut target_scopes = current
            .definition
            .scope_target(&request.operation_ids, granted)
            .ok_or(AdapterOAuthSetupError::Invalid)?;
        let mut selected_digests = BTreeSet::from([request.semantic_digest.as_str()]);
        for selection in &request.additional_services {
            if !selected_digests.insert(&selection.semantic_digest) {
                return Err(AdapterOAuthSetupError::Invalid);
            }
            let additional = self.load_oauth_grant(
                &selection.semantic_digest,
                &request.application_id,
                request.grant_id.as_deref(),
            )?;
            if additional.application.revision != request.expected_application_revision
                || additional
                    .grant
                    .as_ref()
                    .map(|grant| grant.authority_revision)
                    != request.expected_grant_revision
            {
                return Err(AdapterOAuthSetupError::Superseded);
            }
            target_scopes.extend(
                additional
                    .definition
                    .scope_target(&selection.operation_ids, granted)
                    .ok_or(AdapterOAuthSetupError::Invalid)?,
            );
        }
        target_scopes.sort();
        target_scopes.dedup();
        let authority = oauth_authority(human_id, &current, target_scopes.clone());
        let attempt = AdapterOAuthAttempt::start(
            &current.definition,
            &current.profile,
            crate::oauth::AdapterOAuthStart {
                profile_digest: &current.profile_digest,
                target_scopes: &target_scopes,
                select_account: current.grant.is_none(),
                client_id: &current.application.client_id,
                authority,
                callback_mode: request.callback_mode,
                redirect_uri: &request.redirect_uri,
                now_epoch_seconds,
                ttl_seconds: OAUTH_ATTEMPT_TTL_SECONDS,
            },
        )
        .map_err(map_oauth_error)?;
        let started = AdapterOAuthSetupStart {
            attempt_id: attempt.attempt_id().to_string(),
            authorization_url: attempt.authorization_url().to_string(),
            expires_at_epoch_seconds: attempt.expires_at_epoch_seconds(),
        };
        let superseded = request.grant_id.as_deref().and_then(|grant_id| {
            self.inner
                .oauth_attempts
                .lock()
                .ok()?
                .active_attempt_for_grant(grant_id)
        });
        self.inner
            .oauth_attempts
            .lock()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?
            .insert(attempt, now_epoch_seconds)
            .map_err(map_oauth_error)?;
        if let Some(attempt_id) = superseded {
            self.transition_oauth_attempt_status(
                &attempt_id,
                AdapterOAuthAttemptStatus::Superseded,
                None,
                None,
            );
        }
        self.record_oauth_attempt_status(AdapterOAuthAttemptEvent {
            attempt_id: started.attempt_id.clone(),
            semantic_digest: Some(request.semantic_digest.clone()),
            grant_id: request.grant_id.clone(),
            grant_revision: request.expected_grant_revision,
            status: AdapterOAuthAttemptStatus::Authorizing,
        });
        let service = self.clone();
        let expiring_attempt_id = started.attempt_id.clone();
        let expires_at = started.expires_at_epoch_seconds;
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(OAUTH_ATTEMPT_TTL_SECONDS)).await;
            service.expire_oauth_attempt(&expiring_attempt_id, expires_at);
        });
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
        let (known_attempt_id, initiating_authority) = {
            let attempts = self
                .inner
                .oauth_attempts
                .lock()
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            (
                attempts.attempt_id_for_callback(callback_url),
                attempts
                    .authority_for_callback(callback_url)
                    .map_err(map_oauth_error)?,
            )
        };
        let lock_id = initiating_authority
            .grant_id
            .as_deref()
            .unwrap_or(&initiating_authority.application_id);
        let lock = self
            .connection_lock(&format!("oauth-grant:{lock_id}"))
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let (current, reservation) = {
            let _guard = lock.write().await;
            let current = self.load_oauth_grant(
                &initiating_authority.semantic_digest,
                &initiating_authority.application_id,
                initiating_authority.grant_id.as_deref(),
            )?;
            let current_authority = oauth_authority(
                &initiating_authority.human_id,
                &current,
                initiating_authority.target_scopes.clone(),
            );
            let reservation = self
                .inner
                .oauth_attempts
                .lock()
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?
                .reserve_for_callback(callback_url, now_epoch_seconds, &current_authority)
                .map_err(map_oauth_error);
            let reservation = match reservation {
                Ok(reservation) => reservation,
                Err(error) => {
                    if let Some(attempt_id) = known_attempt_id.as_ref() {
                        self.transition_oauth_attempt_status(
                            attempt_id,
                            oauth_attempt_status_for_error(error),
                            None,
                            None,
                        );
                    }
                    return Err(error);
                }
            };
            (current, reservation)
        };
        let state_key = reservation.state_key();
        let attempt_id = reservation.attempt_id().to_string();
        let result = async {
            let code = reservation
                .complete(callback_url, now_epoch_seconds, &initiating_authority)
                .map_err(map_oauth_error)?;
            let (authorization_code, redirect_uri, pkce_verifier) = code.token_exchange_parts();
            let token = self
                .inner
                .http
                .exchange_oauth_token(AdapterOAuthTokenRequest {
                    token_endpoint: url::Url::parse(&current.profile.token_endpoint)
                        .map_err(|_| AdapterOAuthSetupError::Unavailable)?,
                    client_authentication: current.profile.client_authentication,
                    client_id: current.application.client_id.clone(),
                    client_secret: current.application_credential.client_secret.clone(),
                    grant: AdapterOAuthTokenGrant::AuthorizationCode {
                        code: authorization_code.to_string(),
                        redirect_uri: redirect_uri.to_string(),
                        pkce_verifier: pkce_verifier.to_string(),
                    },
                    expected_scopes: initiating_authority.target_scopes.clone(),
                    omitted_scope_policy: current.profile.omitted_scope_policy,
                    now_epoch_seconds,
                })
                .await
                .map_err(map_oauth_token_error)?;
            let account_label = self
                .probe_account_identity(&current.definition, &current.profile, &token.access_token)
                .await;

            let _guard = lock.write().await;
            let fresh = self.load_oauth_grant(
                &initiating_authority.semantic_digest,
                &initiating_authority.application_id,
                initiating_authority.grant_id.as_deref(),
            )?;
            let fresh_authority = oauth_authority(
                &initiating_authority.human_id,
                &fresh,
                initiating_authority.target_scopes.clone(),
            );
            if !initiating_authority.matches(&fresh_authority)
                || fresh.application_credential != current.application_credential
            {
                return Err(AdapterOAuthSetupError::Superseded);
            }
            let account = self.resolve_external_account(&fresh, account_label)?;
            let (grant, old_token, newly_authorized) = self.resolve_authorization_grant(
                &fresh,
                account.as_ref(),
                &initiating_authority.target_scopes,
            )?;
            let refresh_token = token.refresh_token.or_else(|| {
                (fresh.profile.preserve_refresh_token_on_expansion)
                    .then(|| old_token.and_then(|token| token.refresh_token))
                    .flatten()
            });
            let generation_id = random_hex(16).map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            let grant_token = OauthGrantTokenV1 {
                schema_version: 1,
                generation_id: generation_id.clone(),
                access_token: token.access_token,
                refresh_token,
                expires_at_epoch_seconds: token.expires_at_epoch_seconds,
            };
            let mut replacement = grant.clone();
            replacement.desired_scopes = initiating_authority.target_scopes.clone();
            replacement.granted_scopes = token.granted_scopes;
            replacement.authority_revision = replacement
                .authority_revision
                .checked_add(1)
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            replacement.token_revision = replacement
                .token_revision
                .checked_add(1)
                .ok_or(AdapterOAuthSetupError::Unavailable)?;
            replacement.token_generation = Some(generation_id);
            replacement.status = AuthorizationGrantStatus::Active;
            let grant = self
                .inner
                .oauth_authorities
                .promote_grant(&grant, &replacement, &grant_token)
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            Ok(AdapterOAuthSetupCompletion {
                attempt_id: attempt_id.clone(),
                semantic_digest: initiating_authority.semantic_digest.clone(),
                grant,
                account,
                newly_authorized,
            })
        }
        .await;
        if let Ok(mut attempts) = self.inner.oauth_attempts.lock() {
            attempts.finish(state_key);
        }
        let status = result.as_ref().map_or_else(
            |error| oauth_attempt_status_for_error(*error),
            |_| AdapterOAuthAttemptStatus::Completed,
        );
        let grant_id = result
            .as_ref()
            .ok()
            .map(|completion| completion.grant.grant_id.clone());
        let grant_revision = result
            .as_ref()
            .ok()
            .map(|completion| completion.grant.authority_revision);
        self.transition_oauth_attempt_status(&attempt_id, status, grant_id, grant_revision);
        result
    }

    async fn probe_account_identity(
        &self,
        definition: &CompiledAdapterDefinition,
        profile: &crate::OauthProfileV1,
        access_token: &str,
    ) -> Option<String> {
        let probe = profile.account_identity.as_ref()?;
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

    fn load_oauth_grant(
        &self,
        semantic_digest: &str,
        application_id: &str,
        grant_id: Option<&str>,
    ) -> Result<LoadedOAuthGrant, AdapterOAuthSetupError> {
        if !self.definition_is_current_reviewed(semantic_digest) {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        let definition = self
            .inner
            .definitions
            .load(semantic_digest)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let definition = AdapterCompiler::compile(&definition.manifest)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        let profile_digest = definition
            .authentication
            .oauth2()
            .map(|config| config.profile_digest.clone())
            .ok_or(AdapterOAuthSetupError::Unavailable)?;
        let profile = self
            .inner
            .oauth_authorities
            .load_profile(&profile_digest)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?
            .profile;
        let (application, application_credential) = self
            .inner
            .oauth_authorities
            .load_application_authority(application_id)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        if application.profile_digest != profile_digest {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        let grant = grant_id
            .map(|grant_id| {
                self.inner
                    .oauth_authorities
                    .load_grant_authority(grant_id)
                    .map(|(grant, _)| grant)
            })
            .transpose()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        if grant
            .as_ref()
            .is_some_and(|grant| grant.application_id != application_id)
        {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        Ok(LoadedOAuthGrant {
            definition,
            profile,
            profile_digest,
            application,
            application_credential,
            grant,
        })
    }

    fn resolve_external_account(
        &self,
        loaded: &LoadedOAuthGrant,
        label: Option<String>,
    ) -> Result<Option<ExternalAccountV1>, AdapterOAuthSetupError> {
        let Some(provider_subject) = label else {
            return loaded
                .grant
                .as_ref()
                .and_then(|grant| grant.account_id.as_deref())
                .map(|account_id| self.inner.oauth_authorities.load_account(account_id))
                .transpose()
                .map_err(|_| AdapterOAuthSetupError::Unavailable);
        };
        let snapshot = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        if let Some(account) = snapshot.accounts.into_iter().find(|account| {
            account.profile_digest == loaded.profile_digest
                && account.provider_subject == provider_subject
        }) {
            return Ok(Some(account));
        }
        let account = ExternalAccountV1 {
            schema_version: 1,
            account_id: random_hex(16).map_err(|_| AdapterOAuthSetupError::Unavailable)?,
            profile_digest: loaded.profile_digest.clone(),
            provider_subject: provider_subject.clone(),
            account_label: Some(provider_subject),
            revision: 1,
        };
        self.inner
            .oauth_authorities
            .install_account(&account)
            .map(Some)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)
    }

    fn resolve_authorization_grant(
        &self,
        loaded: &LoadedOAuthGrant,
        account: Option<&ExternalAccountV1>,
        target_scopes: &[String],
    ) -> Result<(AuthorizationGrantV1, Option<OauthGrantTokenV1>, bool), AdapterOAuthSetupError>
    {
        if let Some(grant) = &loaded.grant {
            let (_, token) = self
                .inner
                .oauth_authorities
                .load_grant_authority(&grant.grant_id)
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            return Ok((grant.clone(), token, false));
        }
        let snapshot = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        if let Some(account) = account
            && let Some(grant) = snapshot.grants.into_iter().find(|grant| {
                grant.application_id == loaded.application.application_id
                    && grant.account_id.as_deref() == Some(account.account_id.as_str())
                    && grant.audience == loaded.profile.grant_audience
            })
        {
            let (_, token) = self
                .inner
                .oauth_authorities
                .load_grant_authority(&grant.grant_id)
                .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            return Ok((grant, token, false));
        }
        let grant = AuthorizationGrantV1 {
            schema_version: 1,
            grant_id: random_hex(16).map_err(|_| AdapterOAuthSetupError::Unavailable)?,
            application_id: loaded.application.application_id.clone(),
            account_id: account.map(|account| account.account_id.clone()),
            account_label: None,
            audience: loaded.profile.grant_audience.clone(),
            desired_scopes: target_scopes.to_vec(),
            granted_scopes: Vec::new(),
            authority_revision: 1,
            token_generation: None,
            token_revision: 1,
            status: AuthorizationGrantStatus::AuthenticationRequired,
        };
        let grant = self
            .inner
            .oauth_authorities
            .install_grant(&grant, None)
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
        Ok((grant, None, true))
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
        if connection.descriptor.connection_revision != expected_connection_revision {
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
        self.refresh_definition_registry()?;
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
            .definition_registry()
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        let diagnostic_count = connections.diagnostics.len();
        let oauth = self
            .inner
            .oauth_authorities
            .snapshot()
            .map_err(|_| CapabilityBindingSourceError::Unavailable)?;
        let mut catalog =
            AdapterCatalogCompiler::compile(&definitions.definitions, &connections, &oauth)
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

    /// Invalidate retired adapter state before discovery.
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
        self.inner.oauth_authorities.prepare()?;
        self.inner
            .oauth_authorities
            .install_profile(&crate::reviewed_google_oauth_profile())?;
        self.inner.schedules.recover()?;
        let legacy_connections = self.inner.connections.quarantine_legacy_descriptors()?;
        let legacy = self.inner.definitions.legacy_definition_digests()?;
        self.inner
            .schedules
            .quarantine_referencing(&legacy, &legacy_connections)?;
        if !legacy_connections.is_empty() {
            self.inner.cursors.quarantine_for_connection_cutover()?;
        }
        self.inner.connections.quarantine_referencing(&legacy)?;
        for digest in legacy {
            self.inner.definitions.quarantine(&digest)?;
        }
        self.refresh_definition_registry()?;
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

fn oauth_authority(
    human_id: &str,
    loaded: &LoadedOAuthGrant,
    target_scopes: Vec<String>,
) -> AdapterOAuthAuthorityV1 {
    AdapterOAuthAuthorityV1 {
        human_id: human_id.to_string(),
        application_id: loaded.application.application_id.clone(),
        application_revision: loaded.application.revision,
        grant_id: loaded.grant.as_ref().map(|grant| grant.grant_id.clone()),
        grant_authority_revision: loaded.grant.as_ref().map(|grant| grant.authority_revision),
        semantic_digest: loaded.definition.semantic_digest.to_string(),
        profile_digest: loaded.profile_digest.clone(),
        target_scopes,
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

const fn oauth_attempt_status_for_error(
    error: AdapterOAuthSetupError,
) -> AdapterOAuthAttemptStatus {
    match error {
        AdapterOAuthSetupError::Denied => AdapterOAuthAttemptStatus::Denied,
        AdapterOAuthSetupError::Expired => AdapterOAuthAttemptStatus::Expired,
        AdapterOAuthSetupError::Superseded => AdapterOAuthAttemptStatus::Superseded,
        AdapterOAuthSetupError::Invalid | AdapterOAuthSetupError::Unavailable => {
            AdapterOAuthAttemptStatus::Failed
        }
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
