//! Test-only contract fakes and runtime composition helpers.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tempfile::TempDir;

#[derive(Debug)]
struct ReadyTestProvider;

impl noema_providers::ProviderOperations for ReadyTestProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: noema_providers::GenerateRequest,
        _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
    ) -> noema_providers::ProviderOperationFuture<'a, noema_providers::GenerateResponse> {
        Box::pin(async {
            Ok(noema_providers::GenerateResponse::final_text(
                "ready",
                "runtime-test",
                "runtime-test",
            ))
        })
    }
}

pub(crate) fn test_paths() -> noema_home::NoemaPaths {
    let home = TempDir::new().expect("temp Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("test paths");
    std::mem::forget(home);
    paths
}

pub(crate) fn system_error_logger() -> noema_home::SystemErrorLogger {
    noema_home::SystemErrorLogger::from_paths(&test_paths())
}

pub(crate) async fn test_store() -> noema_store::NoemaStore {
    noema_store::test_support::open_ephemeral_store()
        .await
        .expect("open ephemeral store")
}

pub(crate) async fn test_store_for_paths(
    paths: &noema_home::NoemaPaths,
) -> noema_store::NoemaStore {
    noema_store::NoemaStore::open(&noema_store::StoreConfig::new(paths.sqlite_db_path()))
        .await
        .expect("open store")
}

pub(crate) fn ready_provider_selection(
    selection: noema_providers::ProviderSelectionSnapshot,
) -> noema_providers::ProviderReadySelection {
    let registry = noema_providers::ProviderRegistry::new();
    ready_provider_selection_in_registry(selection, &registry)
}

pub(crate) fn ready_provider_selection_in_registry(
    mut selection: noema_providers::ProviderSelectionSnapshot,
    registry: &noema_providers::ProviderRegistry,
) -> noema_providers::ProviderReadySelection {
    if selection.provider_instance_key.is_none()
        && selection.provider_kind != noema_providers::ProviderKind::LocalModels.as_str()
    {
        selection.provider_instance_key = Some(
            noema_providers::provider_account_instance_key(&selection.provider_account_id)
                .expect("hosted provider key"),
        );
    }
    let key = selection
        .provider_instance_key
        .clone()
        .expect("ready selection requires an exact key");
    registry
        .register(key, ready_test_provider())
        .expect("register test provider");
    registry
        .prove_ready_selection(selection)
        .expect("prove ready selection")
}

pub(crate) fn ready_test_provider() -> noema_providers::ProviderHandle {
    Arc::new(ReadyTestProvider)
}

pub(crate) fn ready_test_provider_registry() -> noema_providers::ProviderRegistryHandle {
    let registry = Arc::new(noema_providers::ProviderRegistry::new());
    for account_id in [
        "provider_account:codex:default",
        "provider_account:openai:default",
        "provider_account:foundation_local:default",
    ] {
        let key = noema_providers::provider_account_instance_key(account_id)
            .expect("hosted test provider key");
        registry
            .register(key, ready_test_provider())
            .expect("register hosted test provider");
    }
    registry
}

pub(crate) async fn initialize_codex_provider_selections(store: &noema_store::NoemaStore) {
    store.ensure_default_actors().await.expect("actors");
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider account");
    let configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-5.6-luna",
        None,
        Some("test_configured_default".to_string()),
    );
    let ready_selection = ready_provider_selection(configured_default);
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialized provider selections");
}

pub(crate) async fn seed_task(
    store: &noema_store::NoemaStore,
    title: &str,
) -> (noema_tasks::TaskRecord, noema_tasks::AgentRunRecord) {
    initialize_codex_provider_selections(store).await;
    let pool = store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("task model settings")
        .into_iter()
        .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
        .expect("simple task model");
    let provider_registry = ready_test_provider_registry();
    store
        .create_task_with_executor_with_readiness(
            noema_tasks::NewTask {
                task_id: None,
                title: title.to_string(),
                request_markdown: "Complete the task".to_string(),
                complexity: noema_tasks::TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: noema_tasks::TaskSource::default(),
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![noema_tasks::NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Task is complete".to_string(),
                    expected_evidence: None,
                }],
            },
            provider_registry.as_ref(),
        )
        .await
        .expect("task")
}

pub(crate) async fn create_exa_provider_account_for_tests(
    store: &noema_store::NoemaStore,
    display_name: &str,
    status: noema_providers::ProviderAccountStatus,
    metadata: serde_json::Value,
) -> noema_providers::PersistedProviderAccountRecord {
    noema_providers::ProviderAccountPersistence::create_provider_account(
        store,
        noema_providers::NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some(display_name.to_string()),
            auth_method: noema_providers::ProviderAuthMethod::SecretInput,
            status,
            metadata,
        },
    )
    .await
    .expect("create Exa provider account")
}

pub(crate) async fn save_provider_capability_assignment_for_tests(
    store: &noema_store::NoemaStore,
    tool_name: &str,
    capability_id: &str,
    account_reference: noema_providers::ProviderCapabilityAccountReference,
) -> noema_providers::ProviderCapabilityAssignment {
    let request = noema_providers::UpsertProviderCapabilityAssignmentRequest::from_storage_values(
        tool_name,
        capability_id,
        account_reference,
    )
    .expect("valid provider capability assignment");
    noema_providers::ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(
        store, request,
    )
    .await
    .expect("save provider capability assignment")
}

pub(crate) fn provider_route(
    mut selection: noema_providers::ProviderSelectionSnapshot,
    provider: noema_providers::ProviderHandle,
) -> Arc<noema_providers::ProviderRouteLease> {
    let key = noema_providers::provider_account_instance_key(&selection.provider_account_id)
        .expect("hosted test provider key");
    selection.provider_instance_key = Some(key.clone());
    let registry = noema_providers::ProviderRegistry::new();
    registry
        .register(key.clone(), provider)
        .expect("register test provider");
    let lease = registry.lease(&key).expect("lease test provider");
    Arc::new(
        noema_providers::ProviderRouteLease::try_new(selection, lease)
            .expect("exact test provider route"),
    )
}

#[derive(Debug)]
struct InMemoryArtifactOperations {
    metadata: noema_artifacts::ArtifactMetadataStoreHandle,
    bytes: Mutex<HashMap<String, (String, Vec<u8>)>>,
}

impl noema_artifacts::ArtifactOperations for InMemoryArtifactOperations {
    fn create_local_file(
        &self,
        request: noema_artifacts::CreateLocalArtifactRequest,
    ) -> noema_artifacts::ArtifactOperationFuture<'_, noema_artifacts::ArtifactWithVersions> {
        Box::pin(async move {
            let filename = noema_artifacts::safe_artifact_filename(&request.filename)?.to_string();
            let artifact_id = self.metadata.new_artifact_id();
            let version_id = self.metadata.new_artifact_version_id();
            let bytes = request.bytes;
            let result = self
                .metadata
                .create_artifact_with_initial_version(
                    noema_artifacts::NewArtifact {
                        artifact_id: Some(artifact_id),
                        owner: request.owner,
                        title: request.title,
                        description: request.description,
                        artifact_kind: request.artifact_kind,
                        storage_kind: noema_artifacts::ArtifactStorageKind::LocalFile,
                        created_by_actor_id: request.created_by_actor_id.clone(),
                        source: request.source.clone(),
                        metadata: request.metadata,
                    },
                    noema_artifacts::NewArtifactVersion {
                        artifact_version_id: Some(version_id.clone()),
                        title: None,
                        storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                            relative_path: format!("test/{version_id}/{filename}"),
                        },
                        media_type: request.media_type,
                        byte_size: Some(i64::try_from(bytes.len()).unwrap_or(i64::MAX)),
                        content_sha256: None,
                        created_by_actor_id: request.created_by_actor_id,
                        source: request.source,
                        metadata: serde_json::json!({}),
                    },
                )
                .await?;
            self.bytes
                .lock()
                .expect("artifact bytes")
                .insert(version_id, (filename, bytes));
            Ok(result)
        })
    }

    fn append_local_file_version(
        &self,
        request: noema_artifacts::AppendLocalArtifactVersionRequest,
    ) -> noema_artifacts::ArtifactOperationFuture<'_, noema_artifacts::ArtifactVersionRecord> {
        Box::pin(async move {
            let filename = noema_artifacts::safe_artifact_filename(&request.filename)?.to_string();
            let target = self
                .metadata
                .load_append_target(&request.artifact_id)
                .await?
                .ok_or_else(|| noema_artifacts::ArtifactMetadataError::NotFound {
                    artifact_id: request.artifact_id.clone(),
                })?;
            let version_id = self.metadata.new_artifact_version_id();
            let bytes = request.bytes;
            let result = self
                .metadata
                .append_artifact_version(
                    &request.artifact_id,
                    target.expected_next_version_index,
                    noema_artifacts::NewArtifactVersion {
                        artifact_version_id: Some(version_id.clone()),
                        title: request.title,
                        storage: noema_artifacts::ArtifactVersionStorage::LocalFile {
                            relative_path: format!("test/{version_id}/{filename}"),
                        },
                        media_type: request.media_type,
                        byte_size: Some(i64::try_from(bytes.len()).unwrap_or(i64::MAX)),
                        content_sha256: None,
                        created_by_actor_id: request.created_by_actor_id,
                        source: request.source,
                        metadata: request.metadata,
                    },
                )
                .await?;
            self.bytes
                .lock()
                .expect("artifact bytes")
                .insert(version_id, (filename, bytes));
            Ok(result)
        })
    }

    fn read_local_file(
        &self,
        request: noema_artifacts::ReadLocalArtifactRequest,
    ) -> noema_artifacts::ArtifactOperationFuture<'_, noema_artifacts::ArtifactFileContent> {
        Box::pin(async move {
            let (filename, bytes) = self
                .bytes
                .lock()
                .expect("artifact bytes")
                .get(&request.version.artifact_version_id)
                .cloned()
                .ok_or(noema_artifacts::ArtifactMetadataError::NotFound {
                    artifact_id: request.artifact.artifact_id,
                })?;
            Ok(noema_artifacts::ArtifactFileContent { filename, bytes })
        })
    }
}

pub(crate) fn artifact_operations(
    store: &noema_store::NoemaStore,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let metadata: noema_artifacts::ArtifactMetadataStoreHandle = Arc::new(store.clone());
    Ok(Arc::new(InMemoryArtifactOperations {
        metadata,
        bytes: Mutex::new(HashMap::new()),
    }))
}

#[derive(Debug)]
struct TestHttpMemoryOperations {
    base_url: String,
}

impl TestHttpMemoryOperations {
    async fn request(
        &self,
        method: &str,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, noema_memory::MemoryOperationError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let address = self
            .base_url
            .strip_prefix("http://")
            .unwrap_or(&self.base_url)
            .trim_end_matches('/');
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
        let body = payload
            .map(|value| serde_json::to_vec(&value))
            .transpose()
            .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?
            .unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nhost: {address}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
        stream
            .write_all(&body)
            .await
            .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
        let split = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or(noema_memory::MemoryOperationError::UnreadableResponse)?;
        let head = std::str::from_utf8(&response[..split])
            .map_err(|_| noema_memory::MemoryOperationError::UnreadableResponse)?;
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or(noema_memory::MemoryOperationError::UnreadableResponse)?;
        if !(200..300).contains(&status) {
            return Err(match status {
                401 | 403 => noema_memory::MemoryOperationError::AuthenticationRejected,
                _ => noema_memory::MemoryOperationError::UnsuccessfulStatus(status),
            });
        }
        let body = &response[split + 4..];
        if body.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(body)
            .map_err(|_| noema_memory::MemoryOperationError::UnreadableResponse)
    }
}

impl noema_memory::MemoryOperations for TestHttpMemoryOperations {
    fn check_readiness(
        &self,
    ) -> noema_memory::MemoryOperationFuture<'_, noema_memory::MemoryServiceReadiness> {
        Box::pin(async move {
            self.request("GET", "/health", None).await?;
            Ok(noema_memory::MemoryServiceReadiness { ready: true })
        })
    }

    fn add_memory(
        &self,
        request: noema_memory::AddMemoryRequest,
    ) -> noema_memory::MemoryOperationFuture<'_, ()> {
        Box::pin(async move {
            let payload = serde_json::to_value(request)
                .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
            self.request("POST", "/v1/memories/add", Some(payload))
                .await?;
            Ok(())
        })
    }

    fn search_memories(
        &self,
        request: noema_memory::SearchMemoriesRequest,
    ) -> noema_memory::MemoryOperationFuture<'_, noema_memory::SearchMemoriesResponse> {
        Box::pin(async move {
            let payload = serde_json::to_value(request)
                .map_err(|_| noema_memory::MemoryOperationError::RequestFailed)?;
            let response = self
                .request("POST", "/v1/memories/search", Some(payload))
                .await?;
            serde_json::from_value(response)
                .map_err(|_| noema_memory::MemoryOperationError::UnreadableResponse)
        })
    }

    fn list_memories(
        &self,
        request: noema_memory::ListMemoriesRequest,
    ) -> noema_memory::MemoryOperationFuture<'_, noema_memory::ListMemoriesResponse> {
        Box::pin(async move {
            let user_id = request.user_id.replace(':', "%3A");
            let path = format!("/v1/memories?user_id={user_id}&limit={}", request.limit);
            let response = self.request("GET", &path, None).await?;
            serde_json::from_value(response)
                .map_err(|_| noema_memory::MemoryOperationError::UnreadableResponse)
        })
    }
}

pub(crate) fn mnemosyne_operations_for_base_url(
    base_url: String,
) -> noema_memory::MemoryOperationsHandle {
    Arc::new(TestHttpMemoryOperations { base_url })
}

#[derive(Debug)]
struct EmptySearchBackend;

impl noema_providers::WebSearchBackend for EmptySearchBackend {
    fn backend_id(&self) -> &str {
        noema_providers::DUCKDUCKGO_PUBLIC_PROVIDER_ID
    }

    fn search<'a>(
        &'a self,
        request: &'a noema_capabilities::web::search::SearchRequest,
    ) -> noema_providers::WebOperationFuture<
        'a,
        noema_capabilities::web::search::SearchResponse,
        noema_providers::WebSearchError,
    > {
        Box::pin(async move {
            Ok(noema_capabilities::web::search::SearchResponse {
                provider: self.backend_id().to_string(),
                query: request.query.clone(),
                results: Vec::new(),
                summary: "No web results found".to_string(),
                provider_contract: "test".to_string(),
            })
        })
    }
}

#[derive(Debug)]
struct EmptyFetchBackend;

impl noema_providers::WebFetchBackend for EmptyFetchBackend {
    fn backend_id(&self) -> &str {
        noema_providers::DIRECT_HTTP_PROVIDER_ID
    }

    fn fetch<'a>(
        &'a self,
        _request: &'a noema_capabilities::web::fetch::FetchRequest,
        _context: &'a noema_providers::WebFetchContext,
    ) -> noema_providers::WebOperationFuture<
        'a,
        noema_capabilities::web::fetch::FetchResponse,
        noema_providers::WebFetchError,
    > {
        Box::pin(async { Err(noema_providers::WebFetchError::Http) })
    }
}

#[derive(Debug)]
struct TestWebBackendResolver {
    search: noema_providers::WebSearchBackendHandle,
    fetch: noema_providers::WebFetchBackendHandle,
    allow_account_search: bool,
    allow_account_fetch: bool,
    store: Option<noema_store::NoemaStore>,
}

impl crate::WebBackendResolver for TestWebBackendResolver {
    fn resolve_search(
        &self,
        request: crate::WebBackendRequest,
    ) -> crate::WebBackendFuture<'_, noema_providers::WebSearchBackendHandle> {
        let search = self.search.clone();
        let allow_account_search = self.allow_account_search;
        Box::pin(async move {
            if request.provider_kind == "exa" && !allow_account_search {
                Err(crate::WebBackendResolverError::Unauthenticated)
            } else {
                Ok(search)
            }
        })
    }

    fn resolve_fetch(
        &self,
        request: crate::WebBackendRequest,
    ) -> crate::WebBackendFuture<'_, noema_providers::WebFetchBackendHandle> {
        let fetch = self.fetch.clone();
        let allow_account_fetch = self.allow_account_fetch;
        Box::pin(async move {
            if request.provider_kind == "exa" && !allow_account_fetch {
                Err(crate::WebBackendResolverError::Unauthenticated)
            } else {
                Ok(fetch)
            }
        })
    }

    fn record_auth_failure(
        &self,
        provider_account_id: String,
        credential_revision: u64,
    ) -> crate::WebBackendFuture<'_, ()> {
        let store = self.store.clone();
        Box::pin(async move {
            let Some(store) = store else {
                return Ok(());
            };
            let account = noema_providers::ProviderAccountPersistence::provider_account(
                &store,
                &provider_account_id,
            )
            .await
            .map_err(|_| crate::WebBackendResolverError::Unavailable)?
            .ok_or(crate::WebBackendResolverError::Unavailable)?;
            let durable_revision = account
                .metadata
                .get("credentialRevision")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if durable_revision != credential_revision {
                return Err(crate::WebBackendResolverError::Unavailable);
            }
            store
                .update_provider_account_status(
                    &provider_account_id,
                    noema_providers::ProviderAccountStatus::Unauthenticated,
                    Some("auth_failed"),
                    Some("Provider rejected the configured credentials"),
                )
                .await
                .map_err(|_| crate::WebBackendResolverError::Unavailable)?;
            Ok(())
        })
    }
}

pub(crate) fn web_backends() -> crate::WebBackendResolverHandle {
    Arc::new(TestWebBackendResolver {
        search: noema_providers::WebSearchBackendHandle::new(EmptySearchBackend),
        fetch: noema_providers::WebFetchBackendHandle::new(EmptyFetchBackend),
        allow_account_search: false,
        allow_account_fetch: false,
        store: None,
    })
}

pub(crate) fn web_backends_for_store(
    store: &noema_store::NoemaStore,
) -> crate::WebBackendResolverHandle {
    Arc::new(TestWebBackendResolver {
        search: noema_providers::WebSearchBackendHandle::new(EmptySearchBackend),
        fetch: noema_providers::WebFetchBackendHandle::new(EmptyFetchBackend),
        allow_account_search: false,
        allow_account_fetch: false,
        store: Some(store.clone()),
    })
}

pub(crate) fn web_backends_with_search(
    search: noema_providers::WebSearchBackendHandle,
) -> crate::WebBackendResolverHandle {
    web_backends_with_search_and_fetch(
        search,
        noema_providers::WebFetchBackendHandle::new(EmptyFetchBackend),
    )
}

pub(crate) fn web_backends_with_search_and_fetch(
    search: noema_providers::WebSearchBackendHandle,
    fetch: noema_providers::WebFetchBackendHandle,
) -> crate::WebBackendResolverHandle {
    Arc::new(TestWebBackendResolver {
        search,
        fetch,
        allow_account_search: true,
        allow_account_fetch: true,
        store: None,
    })
}
