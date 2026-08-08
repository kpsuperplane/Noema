//! Test-only composition helpers shared across runtime and API tests.

mod artifacts;

pub(crate) use artifacts::{
    artifact_diagnostics_for_environment, artifact_operations_for_environment,
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
) -> noema_providers::ProviderAccountRecord {
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
