//! Root-contract adapter capability service.

use crate::{
    AdapterCatalogCompiler, AdapterCompiler, AdapterConnectionRevisions, AdapterConnectionStatus,
    AdapterConnectionStore, AdapterConnectionV1, AdapterDefinitionStore,
    credential_import::import_client_json,
    network::{AdapterHttpExecutor, ReqwestAdapterHttpExecutor},
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

use crate::catalog::ADAPTER_INVOKER_KEY;

pub(crate) struct AdapterCapabilityServiceInner {
    pub(crate) definitions: AdapterDefinitionStore,
    pub(crate) connections: AdapterConnectionStore,
    connection_locks: Mutex<BTreeMap<String, Arc<RwLock<()>>>>,
    pub(crate) http: Arc<dyn AdapterHttpExecutor>,
}

/// Filesystem-backed binding source and credentialed JSON invoker.
#[derive(Clone)]
pub struct AdapterCapabilityService {
    pub(crate) inner: Arc<AdapterCapabilityServiceInner>,
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
                connections: AdapterConnectionStore::new(paths),
                connection_locks: Mutex::new(BTreeMap::new()),
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
        let descriptor = AdapterConnectionV1 {
            schema_version: 1,
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
        };
        self.inner
            .connections
            .install(&descriptor, Some(&credential), &definition)
            .map_err(|_| AdapterConnectionSetupError::Unavailable)
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
