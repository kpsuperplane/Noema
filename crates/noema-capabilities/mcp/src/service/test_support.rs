use std::{
    fs,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

trait TestMutex<T> {
    fn lock_test(&self) -> MutexGuard<'_, T>;
}

impl<T> TestMutex<T> for Mutex<T> {
    fn lock_test(&self) -> MutexGuard<'_, T> {
        self.lock().expect("test mutex poisoned")
    }
}

use noema_capabilities::{
    CapabilityBindingSource, CapabilityError, CapabilityInvocation, CapabilityInvoker,
    CapabilityOutput,
};
use noema_home::NoemaPaths;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Semaphore;

use crate::{
    FilesystemMcpSecretStore, LocalMcpService, LocalMcpServiceConfig, McpClientError,
    McpConnectionReplacement, McpControlPlaneServer, McpControlPlaneTool, McpDefinitionRecord,
    McpDefinitionTarget, McpDeleteTicket, McpDiagnosticEvent, McpDiagnosticSink, McpDiscoveredTool,
    McpDiscoveryCommit, McpFailureStatus, McpInitialDiscoveryCommit, McpInvocationSnapshot,
    McpOAuthStoredCredentials, McpPreparedSession, McpProviderPolicyUpdate, McpRepository,
    McpRepositoryError, McpRepositoryErrorKind, McpRepositoryFuture, McpRepositoryResult,
    McpRequestContext, McpSecretCommit, McpSecretMaterial, McpSecretStage, McpSecretStore,
    McpSecretStoreError, McpServerHealthStatus, McpServerRecord, McpSessionFactory,
    McpSessionPreparation, McpToolCallOutput, McpToolHint, McpToolPolicyOverride,
    McpToolPolicyRecord, McpToolPolicyStatus, McpToolRecord,
    test_fixture::{discovered_tool, ready_server},
};

#[derive(Debug, Default)]
pub(crate) struct EventLog(Mutex<Vec<&'static str>>);

impl EventLog {
    fn push(&self, event: &'static str) {
        self.0.lock_test().push(event);
    }

    pub(crate) fn clear(&self) {
        self.0.lock_test().clear();
    }

    pub(crate) fn snapshot(&self) -> Vec<&'static str> {
        self.0.lock_test().clone()
    }

    pub(crate) fn contains(&self, event: &'static str) -> bool {
        self.0.lock_test().contains(&event)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RecordingDiagnostics {
    pub(crate) events: Mutex<Vec<McpDiagnosticEvent>>,
}

impl RecordingDiagnostics {
    pub(crate) fn take(&self) -> Vec<McpDiagnosticEvent> {
        std::mem::take(&mut *self.events.lock_test())
    }
}

impl McpDiagnosticSink for RecordingDiagnostics {
    fn record(&self, event: McpDiagnosticEvent) {
        self.events.lock_test().push(event);
    }
}

#[derive(Debug)]
struct RepositoryState {
    joined: Option<McpControlPlaneServer>,
    snapshot: Option<McpInvocationSnapshot>,
    replace_error: Option<McpRepositoryError>,
    events: Vec<&'static str>,
}

#[derive(Debug)]
pub(crate) struct TestRepository {
    state: Mutex<RepositoryState>,
}

impl TestRepository {
    pub(crate) fn new(joined: McpControlPlaneServer) -> Self {
        Self {
            state: Mutex::new(RepositoryState {
                snapshot: Some(invocation_snapshot(&joined)),
                joined: Some(joined),
                replace_error: None,
                events: Vec::new(),
            }),
        }
    }

    pub(crate) fn snapshot(&self) -> McpInvocationSnapshot {
        self.state.lock_test().snapshot.clone().expect("snapshot")
    }

    pub(crate) fn set_snapshot(&self, snapshot: McpInvocationSnapshot) {
        self.state.lock_test().snapshot = Some(snapshot);
    }

    pub(crate) fn set_safe_config(&self, safe_config: serde_json::Value) {
        let mut state = self.state.lock_test();
        if let Some(joined) = state.joined.as_mut() {
            joined.server.safe_config = safe_config.clone();
        }
        if let Some(snapshot) = state.snapshot.as_mut() {
            snapshot.server.safe_config = safe_config;
        }
    }

    pub(crate) fn fail_replacement(&self, detail: &str) {
        self.state.lock_test().replace_error = Some(McpRepositoryError::new(
            McpRepositoryErrorKind::Conflict,
            detail,
        ));
    }

    pub(crate) fn events(&self) -> Vec<&'static str> {
        self.state.lock_test().events.clone()
    }
}

macro_rules! test_repository {
    ($repository:ident; $($method:ident($($argument:ident: $type:ty),*) -> $result:ty $body:block)*) => {
        impl McpRepository for TestRepository {
            $(fn $method(
                &self,
                $($argument: $type),*
            ) -> McpRepositoryFuture<'_, McpRepositoryResult<$result>> {
                let $repository = self;
                Box::pin(async move $body)
            })*
        }
    };
}

test_repository! {
    repository;
    definition(_definition_id: String) -> Option<McpDefinitionRecord> {
        Ok(repository.state.lock_test().joined.as_ref().map(|joined| McpDefinitionRecord {
            mcp_definition_id: joined.server.mcp_definition_id.clone(),
            display_name: joined.server.display_name.clone(),
            transport_kind: joined.server.transport_kind,
            safe_config: joined.server.safe_config.clone(),
            definition_revision: joined.server.definition_revision.clone(),
        }))
    }

    commit_initial_discovery(input: McpInitialDiscoveryCommit) -> McpControlPlaneServer {
            let mut state = repository.state.lock_test();
            state.events.push("commit_initial_discovery");
            let server_id = "mcp:created".to_string();
            let tools = control_plane_tools(&server_id, input.tools);
            let (mcp_definition_id, definition_revision) = match input.definition {
                McpDefinitionTarget::New => (
                    "mcp_definition:created".to_string(),
                    "mcp_definition_revision:created".to_string(),
                ),
                McpDefinitionTarget::Existing {
                    mcp_definition_id,
                    expected_definition_revision,
                } => (mcp_definition_id, expected_definition_revision),
            };
            let server = McpServerRecord {
                mcp_definition_id,
                definition_revision,
                mcp_server_id: server_id,
                connection_label: input.connection_label,
                display_name: input.server.display_name,
                transport_kind: input.server.transport_kind,
                safe_config: input.server.safe_config,
                enabled: false,
                data_sharing_policy: None,
                unsafe_action_policy: None,
                policy_revision: 0,
                health_status: McpServerHealthStatus::Healthy,
                auth_status: input.auth_status,
                tool_count: tools.len(),
                available_tool_count: 0,
                pending_tool_count: tools.len(),
                defaulted_tool_count: 0,
                disabled_tool_count: 0,
                authority_generation: "generation:created".to_string(),
            };
            let joined = McpControlPlaneServer { server, tools };
            state.snapshot = Some(invocation_snapshot(&joined));
            state.joined = Some(joined.clone());
            Ok(joined)
    }

    control_plane_server(_server_id: String) -> Option<McpControlPlaneServer> {
        Ok(repository.state.lock_test().joined.clone())
    }

    control_plane_catalog() -> Vec<McpControlPlaneServer> {
        Ok(repository.state.lock_test().joined.clone().into_iter().collect())
    }

    invocation_snapshot(_server_id: String, _tool_id: String) -> Option<McpInvocationSnapshot> {
        Ok(repository.state.lock_test().snapshot.clone())
    }

    replace_connection(input: McpConnectionReplacement) -> McpServerRecord {
            let mut state = repository.state.lock_test();
            state.events.push("replace_connection");
            if let Some(error) = state.replace_error.take() {
                return Err(error);
            }
            let joined = state.joined.as_mut().ok_or_else(|| {
                McpRepositoryError::new(McpRepositoryErrorKind::NotFound, "missing server")
            })?;
            if joined.server.authority_generation != input.expected_authority_generation {
                return Err(McpRepositoryError::new(
                    McpRepositoryErrorKind::Conflict,
                    "stale generation",
                ));
            }
            if joined.server.safe_config != input.safe_config
                || joined.server.transport_kind != input.transport_kind
            {
                joined.server.authority_generation = "generation:replaced".to_string();
            }
            joined.server.safe_config = input.safe_config;
            joined.server.transport_kind = input.transport_kind;
            let server = joined.server.clone();
            if let Some(snapshot) = state.snapshot.as_mut() {
                snapshot.server = server.clone();
            }
            Ok(server)
    }

    commit_discovery(input: McpDiscoveryCommit) -> McpControlPlaneServer {
            let mut state = repository.state.lock_test();
            state.events.push("commit_discovery");
            let joined = state.joined.as_mut().ok_or_else(|| {
                McpRepositoryError::new(McpRepositoryErrorKind::NotFound, "missing server")
            })?;
            if joined.server.authority_generation != input.expected_authority_generation {
                return Err(McpRepositoryError::new(
                    McpRepositoryErrorKind::Conflict,
                    "stale generation",
                ));
            }
            joined.server.health_status = input.health_status;
            joined.server.auth_status = input.auth_status;
            joined.tools = control_plane_tools(&input.mcp_server_id, input.tools);
            joined.server.tool_count = joined.tools.len();
            let result = joined.clone();
            state.snapshot = Some(invocation_snapshot(&result));
            Ok(result)
    }

    record_failure_status(input: McpFailureStatus) -> bool {
            let mut state = repository.state.lock_test();
            state.events.push("record_status");
            let Some(joined) = state.joined.as_mut() else {
                return Ok(false);
            };
            if joined.server.authority_generation != input.expected_authority_generation {
                return Ok(false);
            }
            joined.server.health_status = input.health_status;
            joined.server.auth_status = input.auth_status;
            Ok(true)
    }

    save_provider_policy(update: McpProviderPolicyUpdate) -> McpServerRecord {
            let mut state = repository.state.lock_test();
            state.events.push("save_provider_policy");
            let server = &mut state.joined.as_mut().expect("joined").server;
            server.data_sharing_policy = Some(update.data_sharing_policy);
            server.unsafe_action_policy = Some(update.unsafe_action_policy);
            server.policy_revision += 1;
            let saved = server.clone();
            if let Some(snapshot) = state.snapshot.as_mut() {
                snapshot.server = saved.clone();
            }
            Ok(saved)
    }

    save_tool_override(update: McpToolPolicyOverride) -> McpToolPolicyRecord {
            let mut state = repository.state.lock_test();
            let policy = human_policy(update);
            set_policy(&mut state, policy.clone());
            Ok(policy)
    }

    reset_tool_policy(mcp_tool_id: String) -> McpToolPolicyRecord {
            let mut state = repository.state.lock_test();
            let current = state.snapshot.as_ref().and_then(|snapshot| snapshot.policy.clone())
                .ok_or_else(|| McpRepositoryError::new(McpRepositoryErrorKind::NotFound, "missing policy"))?;
            let policy = McpToolPolicyRecord { tool_id: mcp_tool_id, status: McpToolPolicyStatus::Pending, policy_revision: current.policy_revision + 1, ..current };
            set_policy(&mut state, policy.clone());
            Ok(policy)
    }

    set_tool_enabled(mcp_tool_id: String, enabled: bool) -> McpToolPolicyRecord {
            let mut state = repository.state.lock_test();
            let current = state.snapshot.as_ref().and_then(|snapshot| snapshot.policy.clone())
                .ok_or_else(|| McpRepositoryError::new(McpRepositoryErrorKind::NotFound, "missing policy"))?;
            let policy = McpToolPolicyRecord { tool_id: mcp_tool_id, status: if enabled { McpToolPolicyStatus::Ready } else { McpToolPolicyStatus::Disabled }, policy_revision: current.policy_revision + 1, ..current };
            set_policy(&mut state, policy.clone());
            Ok(policy)
    }

    complete_tool_policy(policy: McpToolPolicyRecord) -> Option<McpToolPolicyRecord> {
            let mut state = repository.state.lock_test();
            let current = state.snapshot.as_ref().and_then(|snapshot| snapshot.policy.as_ref());
            if current.is_none_or(|current| current.status != McpToolPolicyStatus::Pending || current.policy_revision != policy.policy_revision || current.source_revision != policy.source_revision) {
                return Ok(None);
            }
            let mut saved = policy;
            saved.policy_revision += 1;
            set_policy(&mut state, saved.clone());
            Ok(Some(saved))
    }

    begin_delete(mcp_server_id: String) -> Option<McpDeleteTicket> {
            let mut state = repository.state.lock_test();
            state.events.push("begin_delete");
            if state.joined.is_none() {
                return Ok(None);
            }
            Ok(Some(McpDeleteTicket {
                mcp_server_id,
                deletion_generation: "generation:deleting".to_string(),
            }))
    }

    finish_delete(_ticket: McpDeleteTicket) -> bool {
            let mut state = repository.state.lock_test();
            state.events.push("finish_delete");
            state.snapshot = None;
            Ok(state.joined.take().is_some())
    }
}

#[derive(Debug)]
pub(crate) struct RecordingSecretStore {
    inner: FilesystemMcpSecretStore,
    paths: NoemaPaths,
    events: Arc<EventLog>,
    fail_commit_id: Mutex<Option<String>>,
}

impl RecordingSecretStore {
    fn new(paths: NoemaPaths, events: Arc<EventLog>) -> Self {
        Self {
            inner: FilesystemMcpSecretStore::new(paths.clone()),
            paths,
            events,
            fail_commit_id: Mutex::new(None),
        }
    }

    pub(crate) fn seed(&self, id: &str, material: &McpSecretMaterial) {
        let stage = self.inner.stage(material).expect("stage seed");
        let commit = self.inner.commit(stage, id).expect("commit seed");
        self.inner.finalize(commit).expect("finalize seed");
        self.events.clear();
    }

    pub(crate) fn material(&self, id: &str) -> McpSecretMaterial {
        self.inner.load(id).expect("load material")
    }

    pub(crate) fn active_file_exists(&self, id: &str) -> bool {
        self.paths.mcp_server_home(id).join("secrets.json").exists()
    }

    pub(crate) fn active_file_bytes(&self, id: &str) -> Vec<u8> {
        fs::read(self.paths.mcp_server_home(id).join("secrets.json"))
            .expect("read active secret file")
    }

    pub(crate) fn overwrite_active_file(&self, id: &str, bytes: &[u8]) {
        let home = self.paths.mcp_server_home(id);
        fs::create_dir_all(&home).expect("create server secret home");
        fs::write(home.join("secrets.json"), bytes).expect("overwrite active secret file");
    }

    pub(crate) fn staged_file_count(&self) -> usize {
        fs::read_dir(self.paths.mcp_dir().join(".staging"))
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    pub(crate) fn fail_commit_for(&self, id: &str) {
        *self.fail_commit_id.lock_test() = Some(id.to_string());
    }
}

macro_rules! record_secret_delegate {
    ($method:ident($argument:ident: $type:ty), $event:literal, $result:ty) => {
        fn $method(&self, $argument: $type) -> Result<$result, McpSecretStoreError> {
            self.events.push($event);
            self.inner.$method($argument)
        }
    };
}

impl McpSecretStore for RecordingSecretStore {
    record_secret_delegate!(load(id: &str), "secret_load", McpSecretMaterial);
    record_secret_delegate!(stage(secrets: &McpSecretMaterial), "secret_stage", McpSecretStage);

    fn commit(
        &self,
        stage: McpSecretStage,
        id: &str,
    ) -> Result<McpSecretCommit, McpSecretStoreError> {
        self.events.push("secret_commit");
        if self.fail_commit_id.lock_test().take().as_deref() == Some(id) {
            fs::create_dir_all(self.paths.mcp_dir()).expect("create mcp dir");
            fs::write(self.paths.mcp_server_home(id), b"block-directory-creation")
                .expect("write blocker");
        }
        self.inner.commit(stage, id)
    }

    record_secret_delegate!(rollback(commit: McpSecretCommit), "secret_rollback", ());
    record_secret_delegate!(finalize(commit: McpSecretCommit), "secret_finalize", ());
    record_secret_delegate!(discard(stage: McpSecretStage), "secret_discard", ());
    record_secret_delegate!(remove(id: &str), "secret_remove", ());

    fn cleanup_abandoned_staging(&self) -> Result<(), McpSecretStoreError> {
        self.inner.cleanup_abandoned_staging()
    }
}

#[derive(Debug)]
pub(crate) struct TestSessionFactory {
    state: Arc<TestSessionState>,
}

#[derive(Debug)]
struct TestSessionState {
    discovered: Mutex<Vec<McpDiscoveredTool>>,
    prepare_error: Mutex<Option<McpClientError>>,
    call_result: Mutex<Result<McpToolCallOutput, McpClientError>>,
    refreshed: Mutex<Option<McpOAuthStoredCredentials>>,
    block_calls: AtomicBool,
    call_started: Arc<Semaphore>,
    release_call: Arc<Semaphore>,
    call_count: AtomicUsize,
    events: Arc<EventLog>,
}

impl TestSessionFactory {
    fn new(events: Arc<EventLog>) -> Self {
        Self {
            state: Arc::new(TestSessionState {
                discovered: Mutex::new(vec![discovered_tool()]),
                prepare_error: Mutex::new(None),
                call_result: Mutex::new(Ok(McpToolCallOutput {
                    result: json!({"content": "ok"}),
                    is_error: false,
                })),
                refreshed: Mutex::new(None),
                block_calls: AtomicBool::new(false),
                call_started: Arc::new(Semaphore::new(0)),
                release_call: Arc::new(Semaphore::new(0)),
                call_count: AtomicUsize::new(0),
                events,
            }),
        }
    }

    pub(crate) fn set_refreshed(&self, credentials: McpOAuthStoredCredentials) {
        *self.state.refreshed.lock_test() = Some(credentials);
    }

    pub(crate) fn set_prepare_error(&self, error: McpClientError) {
        *self.state.prepare_error.lock_test() = Some(error);
    }

    pub(crate) fn set_call_error(&self, error: McpClientError) {
        *self.state.call_result.lock_test() = Err(error);
    }

    pub(crate) fn block_calls(&self) {
        self.state.block_calls.store(true, Ordering::SeqCst);
    }

    pub(crate) async fn wait_for_call(&self) {
        self.state
            .call_started
            .clone()
            .acquire_owned()
            .await
            .expect("call started")
            .forget();
    }

    pub(crate) fn release_call(&self) {
        self.state.release_call.add_permits(1);
    }

    pub(crate) fn call_count(&self) -> usize {
        self.state.call_count.load(Ordering::SeqCst)
    }
}

impl McpSessionFactory for TestSessionFactory {
    fn prepare<'a>(
        &'a self,
        _: &'a McpServerRecord,
        _: &'a McpSecretMaterial,
        _: &'a McpRequestContext,
    ) -> crate::McpClientFuture<'a, McpSessionPreparation> {
        Box::pin(async move {
            self.state.events.push("session_prepare");
            if let Some(error) = self.state.prepare_error.lock_test().clone() {
                return Err(error);
            }
            Ok(McpSessionPreparation::new(
                Box::new(TestSession {
                    state: self.state.clone(),
                }),
                self.state.refreshed.lock_test().clone(),
            ))
        })
    }
}

struct TestSession {
    state: Arc<TestSessionState>,
}

impl McpPreparedSession for TestSession {
    fn discover_tools<'a>(
        &'a mut self,
        _: &'a McpRequestContext,
    ) -> crate::McpClientFuture<'a, Vec<McpDiscoveredTool>> {
        Box::pin(async move { Ok(self.state.discovered.lock_test().clone()) })
    }

    fn call_tool<'a>(
        &'a mut self,
        _: &'a str,
        _: serde_json::Value,
        context: &'a McpRequestContext,
    ) -> crate::McpClientFuture<'a, McpToolCallOutput> {
        Box::pin(async move {
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
            self.state.events.push("tool_call");
            self.state.call_started.add_permits(1);
            if self.state.block_calls.load(Ordering::SeqCst) {
                let cancellation = context.cancellation_token();
                tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(McpClientError::Cancelled { operation: "tools/call" });
                    }
                    permit = self.state.release_call.clone().acquire_owned() => {
                        permit.expect("call release").forget();
                    }
                }
            }
            self.state.call_result.lock_test().clone()
        })
    }

    fn close(self: Box<Self>) -> crate::McpClientFuture<'static, ()> {
        self.state.events.push("session_close");
        Box::pin(async { Ok(()) })
    }
}

pub(crate) struct TestHarness {
    pub(crate) service: LocalMcpService,
    pub(crate) repository: Arc<TestRepository>,
    pub(crate) secrets: Arc<RecordingSecretStore>,
    pub(crate) sessions: Arc<TestSessionFactory>,
    pub(crate) diagnostics: Arc<RecordingDiagnostics>,
    pub(crate) events: Arc<EventLog>,
    _home: TempDir,
}

impl TestHarness {
    pub(crate) fn new() -> Self {
        let home = tempfile::tempdir().expect("temporary home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let events = Arc::new(EventLog::default());
        let repository = Arc::new(TestRepository::new(ready_server()));
        let secrets = Arc::new(RecordingSecretStore::new(paths, events.clone()));
        let sessions = Arc::new(TestSessionFactory::new(events.clone()));
        let diagnostics = Arc::new(RecordingDiagnostics::default());
        let service = LocalMcpService::new(
            repository.clone(),
            secrets.clone(),
            diagnostics.clone(),
            None,
            LocalMcpServiceConfig::default(),
            {
                let sessions = sessions.clone();
                move |_| sessions
            },
        )
        .expect("test service");
        events.clear();
        Self {
            service,
            repository,
            secrets,
            sessions,
            diagnostics,
            events,
            _home: home,
        }
    }

    pub(crate) async fn start_blocked_invocation(
        &self,
    ) -> tokio::task::JoinHandle<Result<CapabilityOutput, CapabilityError>> {
        self.sessions.block_calls();
        let invocation = advertised_invocation(self).await;
        let service = self.service.clone();
        let task =
            tokio::spawn(async move { CapabilityInvoker::invoke(&service, invocation).await });
        self.sessions.wait_for_call().await;
        task
    }
}

fn invocation_snapshot(server: &McpControlPlaneServer) -> McpInvocationSnapshot {
    let entry = &server.tools[0];
    McpInvocationSnapshot {
        server: server.server.clone(),
        tool: entry.tool.clone(),
        policy: entry.policy.clone(),
    }
}

pub(crate) async fn advertised_invocation(harness: &TestHarness) -> CapabilityInvocation {
    let catalog = CapabilityBindingSource::catalog(&harness.service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .resolve("mcp.mcp:docs.read")
        .expect("binding");
    CapabilityInvocation {
        operation: binding.spec().name.clone(),
        operation_token: binding.target().operation_token().clone(),
        arguments: json!({}),
        reviewed_authorization: None,
    }
}

fn tool_record(server_id: &str, index: usize, tool: McpDiscoveredTool) -> McpToolRecord {
    McpToolRecord {
        mcp_tool_id: format!("mcp_tool:{server_id}:{index}"),
        mcp_server_id: server_id.to_string(),
        name: tool.name,
        description: tool.description,
        input_schema: tool.input_schema,
        output_schema: tool.output_schema,
        annotations: tool.annotations,
        metadata_fingerprint: tool.metadata_fingerprint,
        discovered_at: "now".to_string(),
    }
}

fn control_plane_tools(server_id: &str, tools: Vec<McpDiscoveredTool>) -> Vec<McpControlPlaneTool> {
    tools
        .into_iter()
        .enumerate()
        .map(|(index, tool)| McpControlPlaneTool {
            policy: Some(McpToolPolicyRecord {
                tool_id: format!("mcp_tool:{server_id}:{index}"),
                read_only: McpToolHint {
                    value: None,
                    source: None,
                },
                idempotent: McpToolHint {
                    value: None,
                    source: None,
                },
                destructive: McpToolHint {
                    value: None,
                    source: None,
                },
                open_world: McpToolHint {
                    value: None,
                    source: None,
                },
                status: McpToolPolicyStatus::Pending,
                policy_revision: 1,
                source_revision: tool.metadata_fingerprint.clone(),
            }),
            tool: tool_record(server_id, index, tool),
        })
        .collect()
}

fn set_policy(state: &mut RepositoryState, policy: McpToolPolicyRecord) {
    if let Some(snapshot) = state.snapshot.as_mut() {
        snapshot.policy = Some(policy.clone());
    }
    if let Some(joined) = state.joined.as_mut()
        && let Some(tool) = joined
            .tools
            .iter_mut()
            .find(|tool| tool.tool.mcp_tool_id == policy.tool_id)
    {
        tool.policy = Some(policy);
    }
}

fn human_policy(update: McpToolPolicyOverride) -> McpToolPolicyRecord {
    let hint = |value| McpToolHint {
        value: Some(value),
        source: Some(crate::McpToolHintSource::Human),
    };
    McpToolPolicyRecord {
        tool_id: update.tool_id,
        read_only: hint(update.read_only),
        idempotent: hint(update.idempotent),
        destructive: hint(update.destructive),
        open_world: hint(update.open_world),
        status: McpToolPolicyStatus::Ready,
        policy_revision: 2,
        source_revision: update.source_revision,
    }
}
