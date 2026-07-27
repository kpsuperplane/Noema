//! Root-contract adapter capability service.

use crate::{
    AdapterCatalogCompiler, AdapterConnectionStore, AdapterDefinitionStore,
    network::{AdapterHttpExecutor, ReqwestAdapterHttpExecutor},
};
use noema_capabilities::{
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogResult, CapabilityFuture, CapabilityInvokerRegistration, InvokerKey,
};
use noema_home::NoemaPaths;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::RwLock;

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
