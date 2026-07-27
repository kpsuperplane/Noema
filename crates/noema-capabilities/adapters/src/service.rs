//! Root-contract adapter capability service.

use crate::{
    AdapterCatalogCompiler, AdapterCompiler, AdapterConnectionRevisions, AdapterConnectionStatus,
    AdapterConnectionStore, AdapterConnectionV2, AdapterCredentialGenerationV1,
    AdapterCredentialMaterial, AdapterDefinitionStore, CompiledAdapterDefinition,
    Oauth2CallbackMode, Oauth2ClientAuthentication,
    credential_import::import_client_json,
    network::{AdapterHttpExecutor, AdapterOAuthTokenRequest, ReqwestAdapterHttpExecutor},
    oauth::{
        AdapterOAuthAttempt, AdapterOAuthAttemptRegistry, AdapterOAuthAuthorityV1,
        AdapterOAuthError,
    },
    private_fs::random_hex,
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogBuilder, CapabilityCatalogResult, CapabilityFuture,
    CapabilityInvokerRegistration, InvokerKey,
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
    /// This definition already has a connection.
    #[error("adapter connection already exists")]
    AlreadyExists,
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
    schedules: crate::ScheduleStore,
    migration_lock: Mutex<()>,
    connection_locks: Mutex<BTreeMap<String, Arc<RwLock<()>>>>,
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
    descriptor: AdapterConnectionV2,
    client_id: String,
    client_secret: Option<String>,
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
                schedules: crate::ScheduleStore::new(paths),
                migration_lock: Mutex::new(()),
                connection_locks: Mutex::new(BTreeMap::new()),
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

    /// Import definition-declared OAuth client JSON and publish one pending
    /// connection. The raw document is dropped after bounded extraction and is
    /// never installed as canonical adapter state.
    ///
    /// # Errors
    ///
    /// Returns a safe category when the exact reviewed definition is absent,
    /// another connection already owns it, extraction fails, or atomic
    /// publication cannot complete.
    pub async fn import_oauth_client_json(
        &self,
        semantic_digest: &str,
        bytes: &[u8],
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
            .connection_lock(&format!("adapter-family:{}", definition.adapter_id))
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let _guard = setup_lock.write().await;
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let connections = self
            .inner
            .connections
            .scan(&definitions.definitions)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        if connections.connections.iter().any(|connection| {
            definitions.definitions.iter().any(|candidate| {
                candidate.compiled.semantic_digest.as_str() == connection.descriptor.semantic_digest
                    && candidate.compiled.adapter_id == definition.adapter_id
            })
        }) {
            return Err(AdapterConnectionSetupError::AlreadyExists);
        }
        let connection_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let generation_id = random_hex(16).map_err(|_| AdapterConnectionSetupError::Unavailable)?;
        let credential = import_client_json(&definition, bytes, generation_id)
            .map_err(|_| AdapterConnectionSetupError::InvalidCredential)?;
        let mut allowed_operations = definition
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        allowed_operations.sort();
        let descriptor = AdapterConnectionV2 {
            schema_version: 2,
            connection_id,
            connection_slug: "personal".to_string(),
            semantic_digest: semantic_digest.to_string(),
            account_id: None,
            account_kind: "personal".to_string(),
            status: AdapterConnectionStatus::AuthenticationRequired,
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
        if current.descriptor.status != AdapterConnectionStatus::AuthenticationRequired
            || current.descriptor.revisions != expected_revisions
        {
            return Err(AdapterOAuthSetupError::Superseded);
        }
        if current
            .definition
            .authentication
            .oauth2
            .as_ref()
            .is_some_and(|config| {
                config.client_authentication != Oauth2ClientAuthentication::None
                    && current.client_secret.is_none()
            })
        {
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
    /// reviewed token endpoint, and atomically activate the exact connection.
    ///
    /// # Errors
    ///
    /// Returns a safe category for a missing/expired attempt, changed
    /// authority, provider denial, invalid token response, or publication
    /// failure.
    pub async fn complete_oauth_callback(
        &self,
        callback_url: &str,
    ) -> Result<crate::ConnectionInstall, AdapterOAuthSetupError> {
        self.complete_oauth_callback_at(callback_url, epoch_seconds()?)
            .await
    }

    async fn complete_oauth_callback_at(
        &self,
        callback_url: &str,
        now_epoch_seconds: u64,
    ) -> Result<crate::ConnectionInstall, AdapterOAuthSetupError> {
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
        let result = async {
            let code = reservation
                .complete(callback_url, now_epoch_seconds, &initiating_authority)
                .map_err(map_oauth_error)?;
            let config = current
                .definition
                .authentication
                .oauth2
                .as_ref()
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
                    requested_scopes: current.definition.authentication.scopes.clone(),
                    now_epoch_seconds,
                })
                .await
                .map_err(map_oauth_token_error)?;

            let _guard = lock.write().await;
            let fresh = self.load_oauth_connection(&connection_id)?;
            let fresh_authority =
                oauth_authority(&initiating_authority.human_id, &fresh.descriptor);
            if !initiating_authority.matches(&fresh_authority)
                || fresh.client_id != current.client_id
                || fresh.client_secret != current.client_secret
            {
                return Err(AdapterOAuthSetupError::Superseded);
            }
            let generation_id = random_hex(16).map_err(|_| AdapterOAuthSetupError::Unavailable)?;
            let credential = AdapterCredentialGenerationV1 {
                schema_version: 1,
                generation_id: generation_id.clone(),
                material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
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
            self.inner
                .connections
                .promote_oauth_credential(
                    &fresh.descriptor,
                    &replacement,
                    &credential,
                    &fresh.definition,
                )
                .map_err(|_| AdapterOAuthSetupError::Unavailable)
        }
        .await;
        if let Ok(mut attempts) = self.inner.oauth_attempts.lock() {
            attempts.finish(state_key);
        }
        result
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
        let Some(AdapterCredentialGenerationV1 {
            material:
                AdapterCredentialMaterial::Oauth2ClientMetadata {
                    client_id,
                    client_secret,
                },
            ..
        }) = credential
        else {
            return Err(AdapterOAuthSetupError::Superseded);
        };
        Ok(LoadedOAuthConnection {
            definition,
            descriptor,
            client_id,
            client_secret,
        })
    }

    /// Quarantine one connection under the same lifecycle fence as invocation.
    ///
    /// # Errors
    ///
    /// Returns a redacted store error if the durable rename cannot complete.
    pub async fn quarantine_connection(
        &self,
        connection_id: &str,
    ) -> Result<(), crate::ConnectionStoreError> {
        let lock = self
            .connection_lock(connection_id)
            .map_err(|_| crate::ConnectionStoreError::Integrity("lifecycle_lock"))?;
        let _guard = lock.write().await;
        self.inner.connections.quarantine(connection_id)
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

    /// Rewrite canonical v1 definitions and their descriptors before discovery.
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
        self.inner.connections.upgrade_v1_descriptors()?;
        let legacy = self.inner.definitions.legacy_definitions()?;
        for candidate in legacy {
            let source = candidate
                .source
                .as_ref()
                .map(|(bytes, extension)| (bytes.as_slice(), extension.as_str()));
            let installed = self.inner.definitions.install(
                &candidate.manifest,
                &candidate.provenance.source_reference,
                candidate.provenance.imported_at.as_deref(),
                source,
            )?;
            self.inner
                .connections
                .rebind_definition(&candidate.old_digest, &installed.compiled)?;
            self.inner.schedules.rebind_definition(
                &candidate.old_digest,
                installed.compiled.semantic_digest.as_str(),
            )?;
            if !self
                .inner
                .connections
                .references_definition(&candidate.old_digest)?
                && !self
                    .inner
                    .schedules
                    .references_definition(&candidate.old_digest)?
            {
                self.inner
                    .definitions
                    .quarantine_legacy(&candidate.old_digest)?;
            }
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

fn oauth_authority(human_id: &str, descriptor: &AdapterConnectionV2) -> AdapterOAuthAuthorityV1 {
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
