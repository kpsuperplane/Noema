//! Narrow composition seams for downstream contract tests.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use noema_capabilities::{
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityCatalogResult,
    CapabilityError, CapabilityFuture, CapabilityInvocation, CapabilityInvoker,
    CapabilityInvokerRegistration, CapabilityOutput, InvokerKey,
    web::{
        fetch::{FetchContentKind, FetchRequest, FetchResponse, FetchSummaryStrategy},
        search::{SearchRequest, SearchResponse},
    },
};
use noema_providers::{
    DIRECT_HTTP_PROVIDER_ID, DUCKDUCKGO_PUBLIC_PROVIDER_ID, EXTRACTION_READABILITYRS,
    NewProviderAccount, PersistedProviderAccountRecord, ProviderAccountPersistence,
    ProviderAccountStatus, ProviderAuthMethod, ProviderCapabilityAccountReference,
    ProviderCapabilityAssignment, ProviderCapabilityAssignmentPersistence, ProviderHandle,
    ProviderKind, ProviderReadySelection, ProviderRegistry, ProviderRegistryHandle,
    ProviderRouteResolverHandle, ProviderSelectionSnapshot, RegistryProviderRouteResolver,
    UpsertProviderCapabilityAssignmentRequest, WebFetchBackend, WebFetchBackendHandle,
    WebFetchContext, WebFetchError, WebOperationFuture, WebSearchBackend, WebSearchBackendHandle,
    WebSearchError, provider_account_instance_key,
};
use noema_store::{NoemaStore, TOOL_PROGRESS_AUDIT_TASK_ID, WEB_FETCH_SUMMARIZER_TASK_ID};
use noema_tasks::{AgentRunRecord, TaskRecord};

use crate::{
    RuntimeError, RuntimeEventRegistry, RuntimeHandle, RuntimeSpawnConfig, WebBackendFuture,
    WebBackendRequest, WebBackendResolver, WebBackendResolverError, WebBackendResolverHandle,
};

static NEXT_DIAGNOSTIC_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct FixedResponseProvider(&'static str);

impl noema_providers::ProviderOperations for FixedResponseProvider {
    fn generate_streaming<'a>(
        &'a self,
        request: noema_providers::GenerateRequest,
        _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
    ) -> noema_providers::ProviderOperationFuture<'a, noema_providers::GenerateResponse> {
        Box::pin(async move {
            Ok(noema_providers::GenerateResponse::final_text(
                self.0,
                "test",
                request.model.unwrap_or_else(|| "fake-model".to_string()),
            ))
        })
    }
}

/// Build a provider that returns one fixed final-text response.
#[must_use]
pub fn fixed_response_provider(text: &'static str) -> ProviderHandle {
    Arc::new(FixedResponseProvider(text))
}

fn isolated_system_error_logger() -> noema_home::SystemErrorLogger {
    let id = NEXT_DIAGNOSTIC_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!(
            "noema-runtime-contract-{}-{id}",
            std::process::id()
        ))
        .join("errors.log");
    noema_home::SystemErrorLogger::new(path)
}

/// Prove a test provider selection against a fresh registry.
#[must_use]
pub fn ready_provider_selection(selection: ProviderSelectionSnapshot) -> ProviderReadySelection {
    ready_provider_selection_in_registry(selection, &ProviderRegistry::new())
}

/// Register and prove a test provider selection in an existing registry.
#[must_use]
pub fn ready_provider_selection_in_registry(
    mut selection: ProviderSelectionSnapshot,
    registry: &ProviderRegistry,
) -> ProviderReadySelection {
    if selection.provider_instance_key.is_none()
        && selection.provider_kind != ProviderKind::LocalModels.as_str()
    {
        selection.provider_instance_key = Some(
            provider_account_instance_key(&selection.provider_account_id)
                .expect("hosted provider key"),
        );
    }
    let key = selection
        .provider_instance_key
        .clone()
        .expect("ready selection requires an exact key");
    noema_store::test_support::register_ready_provider(registry, key)
        .expect("register test provider");
    registry
        .prove_ready_selection(selection)
        .expect("prove ready selection")
}

/// Build a registry containing ready default hosted-provider accounts.
#[must_use]
pub fn ready_test_provider_registry() -> ProviderRegistryHandle {
    noema_store::test_support::ready_hosted_provider_registry([
        "provider_account:codex:default",
        "provider_account:openai:default",
        "provider_account:foundation_local:default",
    ])
    .expect("register hosted test providers")
}

/// Initialize the durable default Codex selections used by contract tests.
pub async fn initialize_codex_provider_selections(store: &NoemaStore) {
    noema_store::test_support::initialize_codex_provider_selections(store)
        .await
        .expect("initialized provider selections");
}

/// Seed a simple task and its executor run.
pub async fn seed_task(store: &NoemaStore, title: &str) -> (TaskRecord, AgentRunRecord) {
    noema_store::test_support::seed_task(store, title)
        .await
        .expect("task")
}

/// Create a persisted Exa account for a contract test.
pub async fn create_exa_provider_account_for_tests(
    store: &NoemaStore,
    display_name: &str,
    status: ProviderAccountStatus,
    metadata: serde_json::Value,
) -> PersistedProviderAccountRecord {
    ProviderAccountPersistence::create_provider_account(
        store,
        NewProviderAccount {
            provider_kind: "exa".to_string(),
            display_name: Some(display_name.to_string()),
            auth_method: ProviderAuthMethod::SecretInput,
            status,
            metadata,
        },
    )
    .await
    .expect("create Exa provider account")
}

/// Persist one provider capability assignment for a contract test.
pub async fn save_provider_capability_assignment_for_tests(
    store: &NoemaStore,
    tool_name: &str,
    capability_id: &str,
    account_reference: ProviderCapabilityAccountReference,
) -> ProviderCapabilityAssignment {
    let request = UpsertProviderCapabilityAssignmentRequest::from_storage_values(
        tool_name,
        capability_id,
        account_reference,
    )
    .expect("valid provider capability assignment");
    ProviderCapabilityAssignmentPersistence::upsert_provider_capability_assignment(store, request)
        .await
        .expect("save provider capability assignment")
}

#[derive(Debug)]
struct EmptyCapabilityBindingSource;

impl CapabilityBindingSource for EmptyCapabilityBindingSource {
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>> {
        Box::pin(async { Ok(CapabilityCatalogResult::default()) })
    }
}

#[derive(Debug)]
struct EmptyCapabilityInvoker;

impl CapabilityInvoker for EmptyCapabilityInvoker {
    fn invoke(
        &self,
        _invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async { Err(CapabilityError::UnknownOperation) })
    }
}

#[derive(Debug)]
struct EmptySearchBackend;

impl WebSearchBackend for EmptySearchBackend {
    fn backend_id(&self) -> &str {
        DUCKDUCKGO_PUBLIC_PROVIDER_ID
    }

    fn search<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> WebOperationFuture<'a, SearchResponse, WebSearchError> {
        Box::pin(async move {
            Ok(SearchResponse {
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

impl WebFetchBackend for EmptyFetchBackend {
    fn backend_id(&self) -> &str {
        DIRECT_HTTP_PROVIDER_ID
    }

    fn fetch<'a>(
        &'a self,
        request: &'a FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, FetchResponse, WebFetchError> {
        Box::pin(async move {
            Ok(FetchResponse {
                provider: self.backend_id().to_string(),
                url: request.url.clone(),
                final_url: request.url.clone(),
                title: Some("Test page".to_string()),
                format: "markdown".to_string(),
                extraction: EXTRACTION_READABILITYRS.to_string(),
                content_kind: FetchContentKind::RawMarkdown,
                content: "Test page content".to_string(),
                raw_excerpt: None,
                raw_chars: 17,
                returned_chars: 17,
                summary_model: None,
                summary_strategy: FetchSummaryStrategy::NotSummarized,
                truncated: false,
            })
        })
    }
}

#[derive(Debug)]
struct ContractWebBackendResolver {
    store: Option<NoemaStore>,
}

impl WebBackendResolver for ContractWebBackendResolver {
    fn resolve_search(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebSearchBackendHandle> {
        Box::pin(async move {
            match request.provider_kind.as_str() {
                "exa" => Err(WebBackendResolverError::Unauthenticated),
                DUCKDUCKGO_PUBLIC_PROVIDER_ID => {
                    Ok(WebSearchBackendHandle::new(EmptySearchBackend))
                }
                _ => Err(WebBackendResolverError::Unavailable),
            }
        })
    }

    fn resolve_fetch(
        &self,
        request: WebBackendRequest,
    ) -> WebBackendFuture<'_, WebFetchBackendHandle> {
        Box::pin(async move {
            match request.provider_kind.as_str() {
                "exa" => Err(WebBackendResolverError::Unauthenticated),
                DIRECT_HTTP_PROVIDER_ID => Ok(WebFetchBackendHandle::new(EmptyFetchBackend)),
                _ => Err(WebBackendResolverError::Unavailable),
            }
        })
    }

    fn record_auth_failure(
        &self,
        provider_account_id: String,
        credential_revision: u64,
    ) -> WebBackendFuture<'_, ()> {
        let store = self.store.clone();
        Box::pin(async move {
            let Some(store) = store else {
                return Ok(());
            };
            let account =
                ProviderAccountPersistence::provider_account(&store, &provider_account_id)
                    .await
                    .map_err(|_| WebBackendResolverError::Unavailable)?
                    .ok_or(WebBackendResolverError::Unavailable)?;
            if account
                .metadata
                .get("credentialRevision")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
                != credential_revision
            {
                return Err(WebBackendResolverError::Unavailable);
            }
            store
                .update_provider_account_status(
                    &provider_account_id,
                    ProviderAccountStatus::Unauthenticated,
                    Some("auth_failed"),
                    Some("Provider rejected the configured credentials"),
                )
                .await
                .map_err(|_| WebBackendResolverError::Unavailable)?;
            Ok(())
        })
    }
}

pub(crate) fn empty_capability_handles() -> (
    noema_capabilities::CapabilityBindingSourceHandle,
    Arc<[CapabilityInvokerRegistration]>,
) {
    (
        Arc::new(EmptyCapabilityBindingSource),
        Arc::from([CapabilityInvokerRegistration::new(
            InvokerKey::new("contract-test"),
            Arc::new(EmptyCapabilityInvoker),
        )]),
    )
}

pub(crate) fn test_web_backends(store: Option<&NoemaStore>) -> WebBackendResolverHandle {
    Arc::new(ContractWebBackendResolver {
        store: store.cloned(),
    })
}

/// Start a runtime around one provider for a downstream contract test.
///
/// # Errors
///
/// Returns provider selection, persistence, or runtime startup failures.
pub async fn spawn_runtime_with_provider(
    provider: ProviderHandle,
    store: NoemaStore,
    artifact_operations: noema_artifacts::ArtifactOperationsHandle,
) -> Result<RuntimeHandle, RuntimeError> {
    let default_account = store.ensure_default_provider_account().await?;
    store
        .update_provider_account_status(
            &default_account.provider_account_id,
            ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    store.ensure_default_actors().await?;

    let registry = Arc::new(ProviderRegistry::new());
    let key = provider_account_instance_key(&default_account.provider_account_id)
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
    registry
        .register(key.clone(), provider)
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        "codex",
        &default_account.provider_account_id,
        "gpt-5.6-luna",
        None,
        Some("contract_test_runtime_default".to_string()),
    );
    selection.provider_instance_key = Some(key);
    let ready = registry
        .prove_ready_selection(selection.clone())
        .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
    store
        .initialize_missing_provider_selections(&selection, Some(&ready))
        .await?;

    let bind = |loader| -> ProviderRouteResolverHandle {
        Arc::new(RegistryProviderRouteResolver::new(
            loader,
            Arc::clone(&registry),
        ))
    };
    let (capability_bindings, capability_invokers) = empty_capability_handles();
    RuntimeHandle::spawn(RuntimeSpawnConfig {
        primary_provider: bind(store.agent_provider_selection_loader("agent:primary")),
        default_provider: bind(store.default_provider_selection_loader()),
        progress_audit_provider: bind(
            store.auxiliary_provider_selection_loader(TOOL_PROGRESS_AUDIT_TASK_ID),
        ),
        web_summary_provider: bind(
            store.auxiliary_provider_selection_loader(WEB_FETCH_SUMMARIZER_TASK_ID),
        ),
        provider_registry: registry,
        store,
        artifact_operations,
        system_errors: isolated_system_error_logger(),
        memory_operations: None,
        runtime_events: RuntimeEventRegistry::default(),
        web_backends: test_web_backends(None),
        capability_bindings,
        capability_invokers,
    })
    .await
}
