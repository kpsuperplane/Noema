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

use super::{MnemosyneClient, MnemosyneConnection};

/// Mnemosyne-backed operations bound to one connection snapshot.
#[derive(Debug)]
pub struct MnemosyneMemoryService {
    connection: Option<MnemosyneConnection>,
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
        Self { connection }
    }

    /// Bind operations to one resolved settings snapshot.
    ///
    /// Repository changes made after construction do not affect the selected
    /// connection. Callers that need dynamic configuration should construct a
    /// new snapshot for each independent request.
    #[must_use]
    pub fn from_settings_snapshot(
        settings: &crate::MemoryServiceSettingsRecord,
        managed_connection: Option<MnemosyneConnection>,
    ) -> Self {
        let connection = match settings.mode {
            MemoryServiceMode::Managed => managed_connection,
            MemoryServiceMode::External => settings
                .base_url
                .clone()
                .map(|base_url| MnemosyneConnection::new(base_url, None)),
        };
        Self::from_connection(connection)
    }

    /// Erase this concrete adapter behind the neutral operations contract.
    #[must_use]
    pub fn into_handle(self) -> MemoryOperationsHandle {
        Arc::new(self)
    }

    async fn client(&self) -> Result<MnemosyneClient, MemoryOperationError> {
        let connection = self
            .connection
            .clone()
            .ok_or(MemoryOperationError::ServiceUnavailable)?;

        Ok(MnemosyneClient::new(
            connection.base_url,
            connection.api_key,
        ))
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
            let service = MnemosyneMemoryService::from_settings_snapshot(
                &settings,
                self.managed_connection.clone(),
            );
            let operations = service.connection.is_some().then(|| service.into_handle());
            Ok(MemoryServiceSnapshot::new(settings, operations))
        })
    }
}

impl MemoryOperations for MnemosyneMemoryService {
    fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness> {
        Box::pin(async move {
            let client = self.client().await?;
            client
                .check_readiness()
                .await
                .map_err(MemoryOperationError::from)?;
            Ok(MemoryServiceReadiness { ready: true })
        })
    }

    fn add_memory(&self, request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()> {
        Box::pin(async move {
            let client = self.client().await?;
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
            let client = self.client().await?;
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
            let client = self.client().await?;
            client
                .list_memories(request)
                .await
                .map_err(MemoryOperationError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::{
        MemoryArticleCacheRecord, MemoryRepository, MemoryRepositoryFuture, MemoryRepositoryResult,
        MemoryServiceSettingsRecord, SaveMemoryArticleCache, SaveMemoryServiceSettings,
    };

    #[derive(Debug)]
    struct FakeRepository {
        settings: Mutex<MemoryServiceSettingsRecord>,
    }

    impl FakeRepository {
        fn set_base_url(&self, base_url: String) {
            self.settings.lock().expect("settings lock").base_url = Some(base_url);
        }

        fn set_managed(&self) {
            let mut settings = self.settings.lock().expect("settings lock");
            settings.mode = MemoryServiceMode::Managed;
            settings.base_url = None;
        }
    }

    impl MemoryRepository for FakeRepository {
        fn memory_service_settings(
            &self,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>
        {
            let settings = self.settings.lock().expect("settings lock").clone();
            Box::pin(async move { Ok(settings) })
        }

        fn save_memory_service_settings(
            &self,
            input: SaveMemoryServiceSettings,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<MemoryServiceSettingsRecord>>
        {
            let settings = MemoryServiceSettingsRecord {
                settings_id: "default".to_string(),
                mode: input.mode,
                base_url: input.base_url,
                port: input.port,
                provider_account_id: input.provider_account_id,
                provider_kind: input.provider_kind,
                model_profile: input.model_profile,
                reasoning_effort: input.reasoning_effort,
            };
            *self.settings.lock().expect("settings lock") = settings.clone();
            Box::pin(async move { Ok(settings) })
        }

        fn memory_article_cache(
            &self,
            _scope_id: String,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<Option<MemoryArticleCacheRecord>>>
        {
            Box::pin(async { Ok(None) })
        }

        fn save_memory_article_cache(
            &self,
            _input: SaveMemoryArticleCache,
        ) -> MemoryRepositoryFuture<'_, MemoryRepositoryResult<()>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn external_settings(base_url: String) -> MemoryServiceSettingsRecord {
        MemoryServiceSettingsRecord {
            settings_id: "default".to_string(),
            mode: MemoryServiceMode::External,
            base_url: Some(base_url),
            port: None,
            provider_account_id: None,
            provider_kind: None,
            model_profile: None,
            reasoning_effort: None,
        }
    }

    async fn spawn_health_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("address"));
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).await.expect("read");
            assert!(String::from_utf8_lossy(&request[..read]).starts_with("GET /health HTTP/1.1"));
            let body = br#"{"status":"ready"}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).await.expect("head");
            stream.write_all(body).await.expect("body");
        });
        base_url
    }

    #[tokio::test]
    async fn external_connection_is_reloaded_for_each_access_snapshot() {
        let first_url = spawn_health_server().await;
        let repository = Arc::new(FakeRepository {
            settings: Mutex::new(external_settings(first_url)),
        });
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

        let second_url = spawn_health_server().await;
        repository.set_base_url(second_url);
        let second = access.resolve().await.expect("second snapshot");
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

    #[tokio::test]
    async fn access_snapshot_does_not_follow_later_repository_changes() {
        let first_url = spawn_health_server().await;
        let repository = Arc::new(FakeRepository {
            settings: Mutex::new(external_settings(first_url)),
        });
        let access = MnemosyneMemoryServiceAccess::new(repository.clone(), None);
        let snapshot = access.resolve().await.expect("external snapshot");

        repository.set_managed();

        assert_eq!(snapshot.settings().mode, MemoryServiceMode::External);
        assert!(
            snapshot
                .operations()
                .expect("external operations")
                .check_readiness()
                .await
                .expect("snapshot remains ready")
                .ready
        );
    }
}
