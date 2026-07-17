use std::{
    collections::BTreeMap,
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use noema_capabilities::{CapabilityBindingSource, CapabilityInvocation};
use noema_home::NoemaPaths;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Semaphore;

use crate::{
    FilesystemMcpSecretStore, LocalMcpService, LocalMcpServiceConfig, McpCalibrationStatus,
    McpClientError, McpConnectionReplacement, McpControlPlaneServer, McpControlPlaneTool,
    McpDeleteTicket, McpDiagnosticEvent, McpDiagnosticSink, McpDiscoveredTool, McpDiscoveryCommit,
    McpFailureStatus, McpInitialDiscoveryCommit, McpInvocationSnapshot, McpOAuthStoredCredentials,
    McpPreparedSession, McpRepository, McpRepositoryError, McpRepositoryErrorKind,
    McpRepositoryFuture, McpRepositoryResult, McpRequestContext, McpSecretCommit,
    McpSecretMaterial, McpSecretStage, McpSecretStore, McpSecretStoreError, McpServerAuthStatus,
    McpServerHealthStatus, McpServerRecord, McpSessionFactory, McpSessionPreparation,
    McpToolCallOutput, McpToolRecord, McpTransportKind, McpTrustClassification, NewToolCalibration,
    ToolCalibrationRecord,
};

pub(crate) type EventLog = Arc<Mutex<Vec<&'static str>>>;

#[derive(Debug, Default)]
pub(crate) struct RecordingDiagnostics {
    pub(crate) events: Mutex<Vec<McpDiagnosticEvent>>,
}

impl RecordingDiagnostics {
    pub(crate) fn take(&self) -> Vec<McpDiagnosticEvent> {
        std::mem::take(&mut *self.events.lock().expect("diagnostics"))
    }
}

impl McpDiagnosticSink for RecordingDiagnostics {
    fn record(&self, event: McpDiagnosticEvent) {
        self.events.lock().expect("diagnostics").push(event);
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
        self.state
            .lock()
            .expect("repository")
            .snapshot
            .clone()
            .expect("snapshot")
    }

    pub(crate) fn set_snapshot(&self, snapshot: McpInvocationSnapshot) {
        self.state.lock().expect("repository").snapshot = Some(snapshot);
    }

    pub(crate) fn set_safe_config(&self, safe_config: serde_json::Value) {
        let mut state = self.state.lock().expect("repository");
        if let Some(joined) = state.joined.as_mut() {
            joined.server.safe_config = safe_config.clone();
        }
        if let Some(snapshot) = state.snapshot.as_mut() {
            snapshot.server.safe_config = safe_config;
        }
    }

    pub(crate) fn fail_replacement(&self, detail: &str) {
        self.state.lock().expect("repository").replace_error = Some(McpRepositoryError::new(
            McpRepositoryErrorKind::Conflict,
            detail,
        ));
    }

    pub(crate) fn events(&self) -> Vec<&'static str> {
        self.state.lock().expect("repository").events.clone()
    }
}

impl McpRepository for TestRepository {
    fn commit_initial_discovery(
        &self,
        input: McpInitialDiscoveryCommit,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
            state.events.push("commit_initial_discovery");
            let server_id = "mcp:created".to_string();
            let tools = input
                .tools
                .into_iter()
                .enumerate()
                .map(|(index, tool)| McpControlPlaneTool {
                    tool: tool_record(&server_id, index, tool),
                    calibration: None,
                })
                .collect::<Vec<_>>();
            let server = McpServerRecord {
                mcp_server_id: server_id,
                display_name: input.server.display_name,
                transport_kind: input.server.transport_kind,
                safe_config: input.server.safe_config,
                enabled: false,
                health_status: McpServerHealthStatus::Healthy,
                auth_status: input.auth_status,
                tool_count: tools.len(),
                authority_generation: "generation:created".to_string(),
            };
            let joined = McpControlPlaneServer { server, tools };
            state.snapshot = Some(invocation_snapshot(&joined));
            state.joined = Some(joined.clone());
            Ok(joined)
        })
    }

    fn control_plane_server(
        &self,
        _: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpControlPlaneServer>>> {
        Box::pin(async move { Ok(self.state.lock().expect("repository").joined.clone()) })
    }

    fn control_plane_catalog(
        &self,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Vec<McpControlPlaneServer>>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .expect("repository")
                .joined
                .clone()
                .into_iter()
                .collect())
        })
    }

    fn invocation_snapshot(
        &self,
        _: String,
        _: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpInvocationSnapshot>>> {
        Box::pin(async move { Ok(self.state.lock().expect("repository").snapshot.clone()) })
    }

    fn replace_connection(
        &self,
        input: McpConnectionReplacement,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpServerRecord>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
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
        })
    }

    fn commit_discovery(
        &self,
        input: McpDiscoveryCommit,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
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
            joined.tools = input
                .tools
                .into_iter()
                .enumerate()
                .map(|(index, tool)| McpControlPlaneTool {
                    tool: tool_record(&input.mcp_server_id, index, tool),
                    calibration: None,
                })
                .collect();
            joined.server.tool_count = joined.tools.len();
            let result = joined.clone();
            state.snapshot = Some(invocation_snapshot(&result));
            Ok(result)
        })
    }

    fn record_failure_status(
        &self,
        input: McpFailureStatus,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<bool>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
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
        })
    }

    fn save_calibrations(
        &self,
        calibrations: Vec<NewToolCalibration>,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Vec<ToolCalibrationRecord>>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
            state.events.push("save_calibrations");
            let saved = calibrations
                .into_iter()
                .map(|calibration| ToolCalibrationRecord {
                    calibration_id: calibration.calibration_id,
                    mcp_tool_id: calibration.mcp_tool_id,
                    read_classification: calibration.read_classification,
                    write_classification: calibration.write_classification,
                    export_classification: calibration.export_classification,
                    status: calibration.status,
                    reviewed_by: calibration.reviewed_by,
                    reviewed_metadata_fingerprint: calibration.reviewed_metadata_fingerprint,
                })
                .collect::<Vec<_>>();
            if let (Some(snapshot), Some(calibration)) =
                (state.snapshot.as_mut(), saved.first().cloned())
            {
                snapshot.calibration = Some(calibration);
            }
            Ok(saved)
        })
    }

    fn begin_delete(
        &self,
        mcp_server_id: String,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<Option<McpDeleteTicket>>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
            state.events.push("begin_delete");
            if state.joined.is_none() {
                return Ok(None);
            }
            Ok(Some(McpDeleteTicket {
                mcp_server_id,
                deletion_generation: "generation:deleting".to_string(),
            }))
        })
    }

    fn finish_delete(
        &self,
        _: McpDeleteTicket,
    ) -> McpRepositoryFuture<'_, McpRepositoryResult<bool>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("repository");
            state.events.push("finish_delete");
            state.snapshot = None;
            Ok(state.joined.take().is_some())
        })
    }
}

#[derive(Debug)]
pub(crate) struct RecordingSecretStore {
    inner: FilesystemMcpSecretStore,
    paths: NoemaPaths,
    events: EventLog,
    fail_commit_id: Mutex<Option<String>>,
}

impl RecordingSecretStore {
    fn new(paths: NoemaPaths, events: EventLog) -> Self {
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
        self.events.lock().expect("events").clear();
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
        *self.fail_commit_id.lock().expect("fail commit") = Some(id.to_string());
    }
}

impl McpSecretStore for RecordingSecretStore {
    fn load(&self, id: &str) -> Result<McpSecretMaterial, McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_load");
        self.inner.load(id)
    }

    fn stage(&self, secrets: &McpSecretMaterial) -> Result<McpSecretStage, McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_stage");
        self.inner.stage(secrets)
    }

    fn commit(
        &self,
        stage: McpSecretStage,
        id: &str,
    ) -> Result<McpSecretCommit, McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_commit");
        if self
            .fail_commit_id
            .lock()
            .expect("fail commit")
            .take()
            .as_deref()
            == Some(id)
        {
            fs::create_dir_all(self.paths.mcp_dir()).expect("create mcp dir");
            fs::write(self.paths.mcp_server_home(id), b"block-directory-creation")
                .expect("write blocker");
        }
        self.inner.commit(stage, id)
    }

    fn rollback(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_rollback");
        self.inner.rollback(commit)
    }

    fn finalize(&self, commit: McpSecretCommit) -> Result<(), McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_finalize");
        self.inner.finalize(commit)
    }

    fn discard(&self, stage: McpSecretStage) -> Result<(), McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_discard");
        self.inner.discard(stage)
    }

    fn remove(&self, id: &str) -> Result<(), McpSecretStoreError> {
        self.events.lock().expect("events").push("secret_remove");
        self.inner.remove(id)
    }

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
    events: EventLog,
}

impl TestSessionFactory {
    fn new(events: EventLog) -> Self {
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
        *self.state.refreshed.lock().expect("refreshed") = Some(credentials);
    }

    pub(crate) fn set_prepare_error(&self, error: McpClientError) {
        *self.state.prepare_error.lock().expect("prepare error") = Some(error);
    }

    pub(crate) fn set_call_error(&self, error: McpClientError) {
        *self.state.call_result.lock().expect("call result") = Err(error);
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
            self.state
                .events
                .lock()
                .expect("events")
                .push("session_prepare");
            if let Some(error) = self
                .state
                .prepare_error
                .lock()
                .expect("prepare error")
                .clone()
            {
                return Err(error);
            }
            Ok(McpSessionPreparation::new(
                Box::new(TestSession {
                    state: self.state.clone(),
                }),
                self.state.refreshed.lock().expect("refreshed").clone(),
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
        Box::pin(async move { Ok(self.state.discovered.lock().expect("discovered").clone()) })
    }

    fn call_tool<'a>(
        &'a mut self,
        _: &'a str,
        _: serde_json::Value,
        context: &'a McpRequestContext,
    ) -> crate::McpClientFuture<'a, McpToolCallOutput> {
        Box::pin(async move {
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
            self.state.events.lock().expect("events").push("tool_call");
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
            self.state.call_result.lock().expect("call result").clone()
        })
    }

    fn close(self: Box<Self>) -> crate::McpClientFuture<'static, ()> {
        self.state
            .events
            .lock()
            .expect("events")
            .push("session_close");
        Box::pin(async { Ok(()) })
    }
}

pub(crate) struct TestHarness {
    pub(crate) service: LocalMcpService,
    pub(crate) repository: Arc<TestRepository>,
    pub(crate) secrets: Arc<RecordingSecretStore>,
    pub(crate) sessions: Arc<TestSessionFactory>,
    pub(crate) diagnostics: Arc<RecordingDiagnostics>,
    pub(crate) events: EventLog,
    _home: TempDir,
}

impl TestHarness {
    pub(crate) fn new() -> Self {
        let home = tempfile::tempdir().expect("temporary home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let events = Arc::new(Mutex::new(Vec::new()));
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
        events.lock().expect("events").clear();
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
}

pub(crate) fn ready_server() -> McpControlPlaneServer {
    let tool = McpToolRecord {
        mcp_tool_id: "mcp_tool:docs:read".to_string(),
        mcp_server_id: "mcp:docs".to_string(),
        name: "read".to_string(),
        description: Some("Read documents".to_string()),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({"readOnlyHint": true}),
        metadata_fingerprint: "fingerprint:v1".to_string(),
        discovered_at: "now".to_string(),
    };
    let calibration = ToolCalibrationRecord {
        calibration_id: "calibration:read".to_string(),
        mcp_tool_id: tool.mcp_tool_id.clone(),
        read_classification: McpTrustClassification::Trusted,
        write_classification: McpTrustClassification::None,
        export_classification: McpTrustClassification::None,
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(tool.metadata_fingerprint.clone()),
    };
    McpControlPlaneServer {
        server: McpServerRecord {
            mcp_server_id: "mcp:docs".to_string(),
            display_name: "Docs".to_string(),
            transport_kind: McpTransportKind::Stdio,
            safe_config: json!({"command": "docs-server", "args": [], "cwd": null, "env": {}}),
            enabled: true,
            health_status: McpServerHealthStatus::Healthy,
            auth_status: McpServerAuthStatus::None,
            tool_count: 1,
            authority_generation: "generation:v1".to_string(),
        },
        tools: vec![McpControlPlaneTool {
            tool,
            calibration: Some(calibration),
        }],
    }
}

fn invocation_snapshot(server: &McpControlPlaneServer) -> McpInvocationSnapshot {
    let entry = &server.tools[0];
    McpInvocationSnapshot {
        server: server.server.clone(),
        tool: entry.tool.clone(),
        calibration: entry.calibration.clone(),
    }
}

pub(crate) fn discovered_tool() -> McpDiscoveredTool {
    McpDiscoveredTool {
        name: "read".to_string(),
        description: Some("Read documents".to_string()),
        input_schema: json!({"type": "object"}),
        output_schema: Some(json!({"type": "object"})),
        annotations: json!({"readOnlyHint": true}),
        metadata_fingerprint: "ignored".to_string(),
    }
}

pub(crate) fn secret_material(value: &str) -> McpSecretMaterial {
    McpSecretMaterial {
        env: BTreeMap::from([("TOKEN".to_string(), value.to_string())]),
        ..McpSecretMaterial::default()
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
