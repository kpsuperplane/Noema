//! Mnemosyne implementations of neutral memory access and operations.

use std::sync::Arc;

use crate::{
    access::{
        MemoryServiceAccess, MemoryServiceAccessFuture, MemoryServiceAccessHandle,
        MemoryServiceSnapshot,
    },
    model::{
        AddMemoryRequest, ListMemoriesRequest, ListMemoriesResponse, MemoryServiceMode,
        SearchMemoriesRequest, SearchMemoriesResponse,
    },
    operations::{
        MemoryOperationError, MemoryOperationFuture, MemoryOperations, MemoryOperationsHandle,
        MemoryServiceReadiness,
    },
    repository::MemoryRepositoryHandle,
};

use super::{MnemosyneConnection, client::MnemosyneClient};

/// Mnemosyne-backed operations bound to one connection snapshot.
#[derive(Debug)]
pub struct MnemosyneMemoryService {
    client: Option<MnemosyneClient>,
}

/// Mnemosyne service access that resolves persisted settings once per request.
#[derive(Debug)]
pub struct MnemosyneMemoryServiceAccess {
    repository: MemoryRepositoryHandle,
    managed_connection: Option<MnemosyneConnection>,
}

impl MnemosyneMemoryService {
    /// Bind operations to one connection for the lifetime of the handle.
    #[must_use]
    pub fn from_connection(connection: Option<MnemosyneConnection>) -> Self {
        Self {
            client: connection
                .map(|connection| MnemosyneClient::new(connection.base_url, connection.api_key)),
        }
    }

    /// Erase this concrete adapter behind the neutral operations contract.
    #[must_use]
    pub fn into_handle(self) -> MemoryOperationsHandle {
        Arc::new(self)
    }

    fn client(&self) -> Result<&MnemosyneClient, MemoryOperationError> {
        self.client
            .as_ref()
            .ok_or(MemoryOperationError::ServiceUnavailable)
    }
}

impl MnemosyneMemoryServiceAccess {
    /// Bind the settings repository to the admitted managed connection.
    #[must_use]
    pub fn new(
        repository: MemoryRepositoryHandle,
        managed_connection: Option<MnemosyneConnection>,
    ) -> Self {
        Self {
            repository,
            managed_connection,
        }
    }

    /// Erase this concrete adapter behind the neutral access contract.
    #[must_use]
    pub fn into_handle(self) -> MemoryServiceAccessHandle {
        Arc::new(self)
    }
}

impl MemoryServiceAccess for MnemosyneMemoryServiceAccess {
    fn resolve(&self) -> MemoryServiceAccessFuture<'_> {
        Box::pin(async move {
            let settings = self.repository.memory_service_settings().await?;
            let connection = match settings.mode {
                MemoryServiceMode::Managed => self.managed_connection.clone(),
                MemoryServiceMode::External => settings
                    .base_url
                    .clone()
                    .map(|base_url| MnemosyneConnection::new(base_url, None)),
            };
            let service = MnemosyneMemoryService::from_connection(connection);
            let operations = service.client.is_some().then(|| service.into_handle());
            Ok(MemoryServiceSnapshot::new(settings, operations))
        })
    }
}

impl MemoryOperations for MnemosyneMemoryService {
    fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness> {
        Box::pin(async move {
            let client = self.client()?;
            client
                .check_readiness()
                .await
                .map_err(MemoryOperationError::from)?;
            Ok(MemoryServiceReadiness { ready: true })
        })
    }

    fn add_memory(&self, request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()> {
        Box::pin(async move {
            let client = self.client()?;
            client
                .add_memory(request)
                .await
                .map_err(MemoryOperationError::from)
        })
    }

    fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> MemoryOperationFuture<'_, SearchMemoriesResponse> {
        Box::pin(async move {
            let client = self.client()?;
            client
                .search_memories(request)
                .await
                .map_err(MemoryOperationError::from)
        })
    }

    fn list_memories(
        &self,
        request: ListMemoriesRequest,
    ) -> MemoryOperationFuture<'_, ListMemoriesResponse> {
        Box::pin(async move {
            let client = self.client()?;
            client
                .list_memories(request)
                .await
                .map_err(MemoryOperationError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{MemoryServiceSettingsRecord, repository::test_support::FakeMemoryRepository};

    fn external_settings(base_url: String) -> MemoryServiceSettingsRecord {
        MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: MemoryServiceMode::External,
            base_url: Some(base_url),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            provider_instance_key: None,
            model_profile: None,
            reasoning_effort: None,
        }
    }

    #[tokio::test]
    async fn access_reloads_external_connections_without_mutating_prior_snapshots() {
        let first_url = super::super::tests::FakeMnemosyneServer::health().await;
        let repository = Arc::new(FakeMemoryRepository::new(external_settings(
            first_url.base_url(),
        )));
        let access = MnemosyneMemoryServiceAccess::new(repository.clone(), None);

        let first = access.resolve().await.expect("first snapshot");
        assert!(
            first
                .operations()
                .expect("first operations")
                .check_readiness()
                .await
                .expect("first ready")
                .ready
        );

        let second_url = super::super::tests::FakeMnemosyneServer::health().await;
        repository.set_external_url(second_url.base_url());
        let second = access.resolve().await.expect("second snapshot");
        repository.set_managed();

        assert_eq!(second.settings().mode, MemoryServiceMode::External);
        assert!(
            second
                .operations()
                .expect("second operations")
                .check_readiness()
                .await
                .expect("second ready")
                .ready
        );
    }
}
