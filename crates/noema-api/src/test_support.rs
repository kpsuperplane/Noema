//! Test-only composition helpers shared across runtime and API tests.

use std::sync::Arc;

mod artifacts;

pub(crate) use artifacts::{
    artifact_diagnostics_for_environment, artifact_operations, artifact_operations_for_environment,
};
pub(crate) use noema_runtime::contract_test_support::{
    create_exa_provider_account_for_tests, ready_provider_selection_in_registry,
    ready_test_provider_registry, save_provider_capability_assignment_for_tests,
};

#[derive(Clone, Debug)]
pub(crate) struct TestEnvironment {
    root: std::path::PathBuf,
}

impl TestEnvironment {
    pub(crate) fn new() -> Self {
        let root = tempfile::tempdir()
            .expect("temporary API test directory")
            .keep();
        Self::from_root(root).expect("initialize API test directory")
    }

    pub(crate) fn from_root(root: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(root.join("db"))?;
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub(crate) fn sqlite_db_path(&self) -> std::path::PathBuf {
        self.root.join("db/noema.sqlite3")
    }

    pub(crate) fn providers_dir(&self) -> std::path::PathBuf {
        self.root.join("providers")
    }

    pub(crate) fn errors_log_path(&self) -> std::path::PathBuf {
        self.root.join("system/errors.log")
    }
}

pub(crate) fn test_environment() -> TestEnvironment {
    TestEnvironment::new()
}

pub(crate) async fn test_store() -> noema_store::NoemaStore {
    noema_store::test_support::open_ephemeral_store()
        .await
        .expect("open ephemeral store")
}

pub(crate) async fn authenticated_default_provider(
    store: &noema_store::NoemaStore,
) -> noema_providers::PersistedProviderAccountRecord {
    store.ensure_default_actors().await.expect("default actors");
    let account = store
        .ensure_default_provider_account()
        .await
        .expect("default provider account");
    store
        .update_provider_account_status(
            &account.provider_account_id,
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate default provider");
    account
}

pub(crate) async fn test_store_for_environment(
    environment: &TestEnvironment,
) -> noema_store::NoemaStore {
    noema_store::NoemaStore::open(&noema_store::StoreConfig::new(environment.sqlite_db_path()))
        .await
        .expect("open store")
}

pub(crate) async fn spawn_runtime_with_provider_registry(
    provider_registry: noema_providers::ProviderRegistryHandle,
    store: noema_store::NoemaStore,
) -> Result<noema_runtime::RuntimeHandle, noema_runtime::RuntimeError> {
    let artifact_operations =
        artifact_operations(&store).map_err(noema_runtime::RuntimeError::Protocol)?;
    let bind = |loader| -> noema_providers::ProviderRouteResolverHandle {
        Arc::new(noema_providers::RegistryProviderRouteResolver::new(
            loader,
            Arc::clone(&provider_registry),
        ))
    };

    noema_runtime::RuntimeHandle::spawn(noema_runtime::RuntimeSpawnConfig {
        primary_provider: bind(store.agent_provider_selection_loader("agent:primary")),
        default_provider: bind(store.default_provider_selection_loader()),
        progress_audit_provider: bind(
            store.auxiliary_provider_selection_loader(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID),
        ),
        action_reviewer_provider: bind(
            store.auxiliary_provider_selection_loader(noema_store::ACTION_REVIEWER_TASK_ID),
        ),
        web_summary_provider: bind(
            store.auxiliary_provider_selection_loader(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID),
        ),
        provider_registry,
        store,
        artifact_operations,
        system_errors: noema_home::SystemErrorLogger::new(test_environment().errors_log_path()),
        native_memory: None,
        runtime_events: noema_runtime::RuntimeEventRegistry::default(),
        web_backends: Arc::new(UnavailableWebBackends),
        capability_bindings: Arc::new(EmptyCapabilityBindings),
        capability_invokers: Arc::from([]),
    })
    .await
}

#[derive(Debug)]
struct EmptyCapabilityBindings;

impl noema_capabilities::CapabilityBindingSource for EmptyCapabilityBindings {
    fn catalog(
        &self,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<
            noema_capabilities::CapabilityCatalogResult,
            noema_capabilities::CapabilityBindingSourceError,
        >,
    > {
        Box::pin(async { Ok(noema_capabilities::CapabilityCatalogResult::default()) })
    }
}

#[derive(Debug)]
struct UnavailableWebBackends;

impl noema_runtime::WebBackendResolver for UnavailableWebBackends {
    fn resolve_search(
        &self,
        _request: noema_runtime::WebBackendRequest,
    ) -> noema_runtime::WebBackendFuture<'_, noema_providers::WebSearchBackendHandle> {
        Box::pin(async { Err(noema_runtime::WebBackendResolverError::Unavailable) })
    }

    fn resolve_fetch(
        &self,
        _request: noema_runtime::WebBackendRequest,
    ) -> noema_runtime::WebBackendFuture<'_, noema_providers::WebFetchBackendHandle> {
        Box::pin(async { Err(noema_runtime::WebBackendResolverError::Unavailable) })
    }

    fn record_auth_failure(
        &self,
        _provider_account_id: String,
        _credential_revision: u64,
    ) -> noema_runtime::WebBackendFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}
