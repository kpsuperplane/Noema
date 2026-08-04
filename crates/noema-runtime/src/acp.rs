//! Stable ACP v1 process execution for Work Executor runs.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::Duration,
};

use agent_client_protocol::schema::{
    ProtocolVersion,
    v1::{
        AuthenticateRequest, CancelNotification, ContentBlock, EnvVariable, Implementation,
        InitializeRequest, McpServer, McpServerStdio, NewSessionRequest, PermissionOptionKind,
        PromptRequest, RequestPermissionOutcome, RequestPermissionRequest,
        RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
        TextContent,
    },
};
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, ConnectionTo};
use noema_store::{
    AcpAgentAuthStatus, AcpAgentHealthStatus, AcpAgentRecord, ExecutionReviewRoute,
    GovernedActionState, GovernedAssessmentStatus, NewGovernedAction, NewGovernedActionAssessment,
    StoredToolBehavior, WorkRunExecutionContext, WorkRunFence,
};
use noema_tasks::{AgentRunItemKind, AgentRunItemStatus, NewAgentRunItem};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    acp_terminal_bridge::{AcpTerminalBridge, helper_executable},
    daemon::task_run_context::{TaskRolePrompt, build_task_role_prompt},
};

const ACP_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Safe initialization metadata returned by an ACP connection test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpProbeResult {
    /// Process health.
    pub health: AcpAgentHealthStatus,
    /// Whether authentication is advertised.
    pub auth: AcpAgentAuthStatus,
    /// Agent implementation name.
    pub implementation_name: Option<String>,
    /// Agent implementation version.
    pub implementation_version: Option<String>,
    /// Advertised capabilities and authentication methods.
    pub capabilities: serde_json::Value,
}

/// Result of one ACP executor connection.
pub(crate) enum AcpRunOutcome {
    Terminal(Box<noema_store::WorkRunTerminal>),
    WaitingForApproval,
}

/// Initialize one configured process without creating a session.
///
/// # Errors
///
/// Returns a safe error when the process cannot initialize before the timeout.
pub async fn probe_acp_agent(agent: &AcpAgentRecord) -> Result<AcpProbeResult, String> {
    probe_acp_agent_with_timeout(agent, ACP_HANDSHAKE_TIMEOUT).await
}

async fn probe_acp_agent_with_timeout(
    agent: &AcpAgentRecord,
    timeout: Duration,
) -> Result<AcpProbeResult, String> {
    let process = configured_process(&agent.command, &agent.arguments);
    tokio::time::timeout(
        timeout,
        agent_client_protocol::Client.connect_with(
            process,
            |connection: ConnectionTo<Agent>| async move {
                let response = connection
                    .send_request(initialize_request())
                    .block_task()
                    .await?;
                Ok(probe_from_initialize(response))
            },
        ),
    )
    .await
    .map_err(|_| "ACP initialization timed out".to_string())?
    .map_err(|_| "ACP initialization failed".to_string())
}

/// Run one advertised agent-managed ACP authentication method.
///
/// # Errors
///
/// Returns a safe error when authentication fails or exceeds its timeout.
pub async fn authenticate_acp_agent(agent: &AcpAgentRecord, method_id: &str) -> Result<(), String> {
    let process = configured_process(&agent.command, &agent.arguments);
    let method_id = method_id.to_string();
    tokio::time::timeout(
        Duration::from_secs(5 * 60),
        agent_client_protocol::Client.connect_with(
            process,
            |connection: ConnectionTo<Agent>| async move {
                connection
                    .send_request(initialize_request())
                    .block_task()
                    .await?;
                connection
                    .send_request(AuthenticateRequest::new(method_id))
                    .block_task()
                    .await?;
                Ok(())
            },
        ),
    )
    .await
    .map_err(|_| "ACP authentication timed out".to_string())?
    .map_err(|_| "ACP authentication failed".to_string())
}

pub(crate) async fn execute_acp_run(
    store: noema_store::NoemaStore,
    run: &noema_tasks::AgentRunRecord,
    fence: &WorkRunFence,
    context: &WorkRunExecutionContext,
    cancellation: &CancellationToken,
) -> Result<AcpRunOutcome, crate::daemon::RuntimeError> {
    let snapshot = run.executor.acp.as_ref().ok_or_else(|| {
        crate::daemon::RuntimeError::Protocol(
            "ACP run has no immutable launch snapshot".to_string(),
        )
    })?;
    let cwd = run.effective_cwd.as_ref().ok_or_else(|| {
        crate::daemon::RuntimeError::Protocol(
            "ACP run has no effective working directory".to_string(),
        )
    })?;
    let bridge = AcpTerminalBridge::start(fence, cancellation.child_token())
        .await
        .map_err(crate::daemon::RuntimeError::Protocol)?;
    let helper = helper_executable().map_err(crate::daemon::RuntimeError::Protocol)?;
    let process = configured_process(&snapshot.command, &snapshot.arguments);
    let sequence = Arc::new(AtomicI64::new(0));
    let notification_store = store.clone();
    let notification_fence = fence.clone();
    let notification_run_id = run.run_id.clone();
    let permission_store = store.clone();
    let permission_fence = fence.clone();
    let session_store = store.clone();
    let session_fence = fence.clone();
    let permission_run = run.clone();
    let permission_context = context.clone();
    let approval_sent = Arc::new(AtomicBool::new(false));
    let permission_approval_sent = approval_sent.clone();
    let prompt = render_prompt(build_task_role_prompt(context));
    let cwd = PathBuf::from(cwd);
    let cancellation = cancellation.clone();
    let bridge_address = bridge.address.clone();
    let bridge_token = bridge.token.clone();

    let result = agent_client_protocol::Client
        .builder()
        .on_receive_notification(
            async move |notification: SessionNotification, _connection| {
                append_session_update(
                    &notification_store,
                    &notification_fence,
                    &notification_run_id,
                    &sequence,
                    notification.update,
                )
                .await;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _connection| {
                let response = handle_permission_request(
                    &permission_store,
                    &permission_fence,
                    &permission_run,
                    &permission_context,
                    &request,
                    &permission_approval_sent,
                )
                .await
                .unwrap_or_else(|_| {
                    RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
                });
                responder.respond(response)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(process, |connection: ConnectionTo<Agent>| async move {
            let initialize = tokio::time::timeout(
                ACP_HANDSHAKE_TIMEOUT,
                connection.send_request(initialize_request()).block_task(),
            )
            .await
            .map_err(|_| acp_error("ACP initialization timed out"))??;
            if initialize.protocol_version != ProtocolVersion::V1 {
                return Err(acp_error("ACP agent did not negotiate protocol v1"));
            }
            let terminal_server =
                McpServer::Stdio(McpServerStdio::new("noema-work-terminal", helper).env(vec![
                    EnvVariable::new("NOEMA_ACP_TASK_BRIDGE_ADDR", bridge_address),
                    EnvVariable::new("NOEMA_ACP_TASK_TOKEN", bridge_token),
                ]));
            let session = connection
                .send_request(NewSessionRequest::new(cwd).mcp_servers(vec![terminal_server]))
                .block_task()
                .await?;
            session_store
                .record_acp_session_id(&session_fence, &session.session_id.to_string())
                .await
                .map_err(acp_error)?;
            let session_id = session.session_id;
            let prompt_request = connection
                .send_request(PromptRequest::new(
                    session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(prompt))],
                ))
                .block_task();
            tokio::pin!(prompt_request);
            tokio::select! {
                _ = cancellation.cancelled() => {
                    let _ = connection.send_notification(CancelNotification::new(session_id));
                    Err(acp_error("ACP run was cancelled"))
                }
                terminal = bridge.receive() => {
                    let terminal = terminal.map_err(acp_error)?;
                    Ok(terminal)
                }
                response = &mut prompt_request => {
                    response?;
                    Err(acp_error("ACP prompt completed without a terminal Work tool call"))
                }
            }
        })
        .await
        .map_err(|_| "ACP execution failed".to_string());

    let current = store
        .get_work_run_record(&run.run_id)
        .await
        .map_err(|error| crate::daemon::RuntimeError::Protocol(error.to_string()))?;
    if current.is_some_and(|run| run.status == noema_tasks::RunStatus::WaitingForApproval) {
        return Ok(AcpRunOutcome::WaitingForApproval);
    }
    let terminal =
        result.map_err(|error| map_process_loss(error, approval_sent.load(Ordering::Acquire)))?;
    let call = noema_providers::GenerateToolCall {
        id: None,
        provider_call_id: None,
        provider_name: Some("acp".to_string()),
        name: terminal.tool,
        payload: terminal.arguments,
    };
    super::daemon::task_runtime::execution::parse_terminal(run, context, &[call], fence.clone())
        .map(|terminal| AcpRunOutcome::Terminal(Box::new(terminal)))
        .map_err(crate::daemon::RuntimeError::Protocol)
}

fn map_process_loss(error: String, approval_was_sent: bool) -> crate::daemon::RuntimeError {
    if approval_was_sent {
        crate::daemon::RuntimeError::OutcomeUncertain
    } else {
        crate::daemon::RuntimeError::Protocol(error)
    }
}

fn configured_process(command: &str, arguments: &[String]) -> AcpAgent {
    AcpAgent::new(AcpAgentConfig::new(command).args(arguments.iter().cloned()))
}

fn initialize_request() -> InitializeRequest {
    InitializeRequest::new(ProtocolVersion::V1)
        .client_info(Implementation::new("noema", env!("CARGO_PKG_VERSION")))
}

fn probe_from_initialize(
    response: agent_client_protocol::schema::v1::InitializeResponse,
) -> AcpProbeResult {
    let info = response.agent_info;
    AcpProbeResult {
        health: AcpAgentHealthStatus::Healthy,
        auth: if response.auth_methods.is_empty() {
            AcpAgentAuthStatus::None
        } else {
            AcpAgentAuthStatus::Required
        },
        implementation_name: info.as_ref().map(|value| value.name.clone()),
        implementation_version: info.as_ref().map(|value| value.version.clone()),
        capabilities: serde_json::json!({
            "agent": response.agent_capabilities,
            "authMethods": response.auth_methods,
        }),
    }
}

async fn append_session_update(
    store: &noema_store::NoemaStore,
    fence: &WorkRunFence,
    run_id: &str,
    sequence: &AtomicI64,
    update: SessionUpdate,
) {
    let index = sequence.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    let payload = serde_json::to_value(&update)
        .unwrap_or_else(|_| serde_json::json!({"diagnostic":"unserializable ACP update"}));
    let (kind, status) = update_kind_status(&update);
    let correlation_id = match &update {
        SessionUpdate::ToolCall(value) => value.tool_call_id.to_string(),
        SessionUpdate::ToolCallUpdate(value) => value.tool_call_id.to_string(),
        _ => format!("acp:{index}"),
    };
    let content_text = update_title(&update)
        .or_else(|| first_text(&payload))
        .map(|text| text.chars().take(20_000).collect());
    let _ = store
        .append_agent_run_item(
            NewAgentRunItem {
                item_id: Some(format!("run_item:acp:{run_id}:{index}")),
                run_id: run_id.to_string(),
                round_index: 0,
                kind,
                status,
                correlation_id: Some(correlation_id),
                parent_item_id: None,
                content_text,
                payload,
            },
            fence,
        )
        .await;
}

fn update_kind_status(update: &SessionUpdate) -> (AgentRunItemKind, AgentRunItemStatus) {
    match update {
        SessionUpdate::AgentMessageChunk(_) | SessionUpdate::AgentThoughtChunk(_) => (
            AgentRunItemKind::AssistantOutput,
            AgentRunItemStatus::Completed,
        ),
        SessionUpdate::ToolCall(value) => {
            (AgentRunItemKind::ToolCall, tool_call_status(&value.status))
        }
        SessionUpdate::ToolCallUpdate(value) => (
            AgentRunItemKind::ToolResult,
            value
                .fields
                .status
                .as_ref()
                .map_or(AgentRunItemStatus::Running, tool_call_status),
        ),
        _ => (
            AgentRunItemKind::ProgressNotice,
            AgentRunItemStatus::Completed,
        ),
    }
}

fn tool_call_status(
    status: &agent_client_protocol::schema::v1::ToolCallStatus,
) -> AgentRunItemStatus {
    match status {
        agent_client_protocol::schema::v1::ToolCallStatus::Completed => {
            AgentRunItemStatus::Completed
        }
        agent_client_protocol::schema::v1::ToolCallStatus::Failed => AgentRunItemStatus::Failed,
        agent_client_protocol::schema::v1::ToolCallStatus::Pending => AgentRunItemStatus::Pending,
        agent_client_protocol::schema::v1::ToolCallStatus::InProgress => {
            AgentRunItemStatus::Running
        }
        _ => AgentRunItemStatus::Running,
    }
}

fn update_title(update: &SessionUpdate) -> Option<&str> {
    match update {
        SessionUpdate::ToolCall(value) => Some(value.title.as_str()),
        SessionUpdate::ToolCallUpdate(value) => value.fields.title.as_deref(),
        _ => None,
    }
}

fn first_text(value: &serde_json::Value) -> Option<&str> {
    match value {
        serde_json::Value::Object(map) => map
            .get("text")
            .and_then(serde_json::Value::as_str)
            .or_else(|| map.values().find_map(first_text)),
        serde_json::Value::Array(values) => values.iter().find_map(first_text),
        _ => None,
    }
}

async fn handle_permission_request(
    store: &noema_store::NoemaStore,
    fence: &WorkRunFence,
    run: &noema_tasks::AgentRunRecord,
    context: &WorkRunExecutionContext,
    request: &RequestPermissionRequest,
    approval_sent: &AtomicBool,
) -> Result<RequestPermissionResponse, String> {
    let Some(allow_once) = request
        .options
        .iter()
        .find(|option| option.kind == PermissionOptionKind::AllowOnce)
    else {
        return Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Cancelled,
        ));
    };
    let arguments = serde_json::json!({
        "agent_id": run.agent_id,
        "task_generation": run.task_generation,
        "contract_id": run.contract_id,
        "tool_call": request.tool_call,
        "options": request.options,
        "allow_once_option_id": allow_once.option_id,
    });
    if let Some(approval) = store
        .consume_succeeded_acp_permission(&run.task_id.to_string(), &run.run_id, &arguments)
        .await
        .map_err(|error| error.to_string())?
    {
        let option_id = approval
            .output
            .as_ref()
            .and_then(|value| value.get("option_id"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "approved ACP permission has no option id".to_string())?;
        approval_sent.store(true, Ordering::Release);
        return Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option_id.to_string(),
            )),
        ));
    }
    if store
        .has_declined_acp_permission(&run.task_id.to_string(), &arguments)
        .await
        .map_err(|error| error.to_string())?
    {
        return Ok(RequestPermissionResponse::new(
            RequestPermissionOutcome::Cancelled,
        ));
    }
    let title = request
        .tool_call
        .fields
        .title
        .as_deref()
        .unwrap_or("Agent-requested operation")
        .chars()
        .take(256)
        .collect::<String>();
    let action = store
        .create_governed_action(NewGovernedAction {
            owner_human_id: "human:local".to_string(),
            conversation_id: None,
            turn_id: None,
            task_id: Some(run.task_id.to_string()),
            run_id: Some(run.run_id.clone()),
            requesting_agent_id: run.agent_id.clone(),
            capability_name: "acp.permission".to_string(),
            operation_token: "acp.permission".to_string(),
            review_route: ExecutionReviewRoute::HumanReview,
            behavior: StoredToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: true,
            },
            arguments,
            input_schema: serde_json::json!({"type":"object"}),
            authorization_context: serde_json::json!({
                "origin": "acp",
                "agent_id": run.agent_id,
                "session_id": request.session_id,
                "run_id": run.run_id,
                "task_id": run.task_id,
                "task_generation": run.task_generation,
                "contract_id": run.contract_id,
                "task_authorization": context.task.authorization_context,
                "exact_request": request,
            }),
            safe_summary: title,
        })
        .await
        .map_err(|error| error.to_string())?;
    let action = store
        .record_governed_action_assessment(
            &action.action_id,
            action.revision,
            NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                reason_codes: vec!["acp_permission_requires_approval".to_string()],
                explanation:
                    "The ACP agent requested one-time permission for an open-world operation."
                        .to_string(),
            },
            Some(fence),
        )
        .await
        .map_err(|error| error.to_string())?;
    if action.state != GovernedActionState::AwaitingApproval {
        return Err("ACP permission did not enter approval state".to_string());
    }
    Ok(RequestPermissionResponse::new(
        RequestPermissionOutcome::Cancelled,
    ))
}

fn render_prompt(prompt: TaskRolePrompt) -> String {
    format!(
        "{}\n\nYou are the selected ACP Work executor for this run. Perform the contract directly with your own tools; do not look for or delegate to another executor.\n\n{}\n\nUse the provided task.submit_result or task.report_blocked MCP tool exactly once to finish. Ordinary assistant text is not a terminal result.",
        prompt.instructions, prompt.input
    )
}

fn acp_error(error: impl std::fmt::Display) -> agent_client_protocol::Error {
    agent_client_protocol::Error::internal_error().data(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_store::{
        WorkCommandService, WorkRunFence, WorkRunItemOwnerScope, WorkRunItemQuery, WorkRunTerminal,
    };
    use noema_workspaces::{PERSONAL_WORKSPACE_ID, WorkspaceId};

    fn fake_agent(script: &str) -> AcpAgentRecord {
        AcpAgentRecord {
            agent_id: "agent:fake-acp".to_string(),
            display_name: "Fake ACP".to_string(),
            command: "python3".to_string(),
            arguments: vec!["-u".to_string(), "-c".to_string(), script.to_string()],
            enabled: true,
            auth_status: AcpAgentAuthStatus::Unknown,
            health_status: AcpAgentHealthStatus::Unknown,
            implementation_name: None,
            implementation_version: None,
            capabilities: serde_json::json!({}),
            connection_revision: 1,
            last_error: None,
        }
    }

    const RESPONDER: &str = r#"
import json, sys
for line in sys.stdin:
    request = json.loads(line)
    if request.get('method') == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {}, 'authMethods': [{'id': 'browser', 'name': 'Browser login'}], 'agentInfo': {'name': 'fake-acp', 'version': '1.2.3'}}
    elif request.get('method') == 'authenticate':
        result = {}
    else:
        result = {}
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
"#;

    const TERMINAL_RESPONDER: &str = r#"
import json, socket, sys, time
bridge = None
for line in sys.stdin:
    request = json.loads(line)
    method = request.get('method')
    if method == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {}, 'authMethods': [], 'agentInfo': {'name': 'fake-acp', 'version': '1.2.3'}}
    elif method == 'session/new':
        env = request['params']['mcpServers'][0]['env']
        bridge = {item['name']: item['value'] for item in env}
        result = {'sessionId': 'session:fake-terminal'}
    elif method == 'session/prompt':
        notification = {'jsonrpc': '2.0', 'method': 'session/update', 'params': {'sessionId': 'session:fake-terminal', 'update': {'sessionUpdate': 'agent_message_chunk', 'content': {'type': 'text', 'text': 'Working through ACP'}}}}
        print(json.dumps(notification), flush=True)
        tool_call = {'jsonrpc': '2.0', 'method': 'session/update', 'params': {'sessionId': 'session:fake-terminal', 'update': {'sessionUpdate': 'tool_call', 'toolCallId': 'tool:fake', 'title': 'Inspect project', 'kind': 'read', 'status': 'in_progress'}}}
        print(json.dumps(tool_call), flush=True)
        tool_result = {'jsonrpc': '2.0', 'method': 'session/update', 'params': {'sessionId': 'session:fake-terminal', 'update': {'sessionUpdate': 'tool_call_update', 'toolCallId': 'tool:fake', 'status': 'completed', 'rawOutput': {'files': 2}}}}
        print(json.dumps(tool_result), flush=True)
        time.sleep(0.1)
        host, port = bridge['NOEMA_ACP_TASK_BRIDGE_ADDR'].rsplit(':', 1)
        terminal = {'token': bridge['NOEMA_ACP_TASK_TOKEN'], 'tool': 'task.report_blocked', 'arguments': {'gate_kind': 'clarification', 'question': 'Which target should I use?', 'context_markdown': 'The ACP agent needs one exact target.', 'suggested_answers': []}}
        with socket.create_connection((host, int(port))) as stream:
            stream.sendall((json.dumps(terminal) + '\n').encode())
            stream.recv(1024)
        time.sleep(60)
        continue
    else:
        result = {}
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
"#;

    const CANCELLATION_RESPONDER: &str = r#"
import json, subprocess, sys, time
for line in sys.stdin:
    request = json.loads(line)
    method = request.get('method')
    if method == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {}, 'authMethods': [], 'agentInfo': {'name': 'fake-acp', 'version': '1.2.3'}}
    elif method == 'session/new':
        result = {'sessionId': 'session:fake-cancellation'}
    elif method == 'session/prompt':
        child = "import pathlib,sys,time; time.sleep(.5); pathlib.Path(sys.argv[1]).write_text('survived')"
        subprocess.Popen([sys.executable, '-c', child, sys.argv[1]])
        time.sleep(60)
        continue
    else:
        result = {}
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
"#;

    async fn acp_execution_fixture(
        script: &str,
        extra_arguments: Vec<String>,
    ) -> (
        noema_store::NoemaStore,
        noema_tasks::AgentRunRecord,
        WorkRunFence,
        noema_store::WorkRunExecutionContext,
    ) {
        let store = noema_store::test_support::open_ephemeral_store()
            .await
            .expect("open ACP test store");
        let mut arguments = vec!["-u".to_string(), "-c".to_string(), script.to_string()];
        arguments.extend(extra_arguments);
        let agent = store
            .create_acp_agent("Fake ACP", "python3", &arguments)
            .await
            .expect("create fake ACP agent");
        let cwd = tempfile::tempdir().expect("create ACP cwd").keep();
        let (_task, _queued_run) = crate::contract_test_support::seed_task_with_executor(
            &store,
            "ACP runtime contract",
            Some(agent.agent_id),
            Some(cwd.to_string_lossy().into_owned()),
        )
        .await;
        let service = WorkCommandService::new(
            store.clone(),
            crate::contract_test_support::ready_test_provider_registry(),
        );
        let claimed = service
            .claim_next_work_run("worker:test:acp", 60, &[])
            .await
            .expect("claim ACP run")
            .expect("queued ACP run");
        let fence = WorkRunFence {
            run_id: claimed.run.run_id.clone(),
            lease_token: claimed.lease_token,
            task_generation: claimed.run.task_generation,
            contract_id: claimed.run.contract_id.clone(),
        };
        service
            .start_work_run(&fence, "actor:test", None, "correlation:test:acp")
            .await
            .expect("start ACP run");
        let admission = service
            .admit_work_run_execution_context(&fence, "actor:test", None, "correlation:test:acp")
            .await
            .expect("admit ACP context");
        (store, claimed.run, fence, admission.context)
    }

    #[tokio::test]
    async fn fake_stdio_acp_covers_probe_auth_malformed_crash_and_timeout() {
        let agent = fake_agent(RESPONDER);
        let probe = probe_acp_agent(&agent).await.expect("initialize fake ACP");
        assert_eq!(probe.implementation_name.as_deref(), Some("fake-acp"));
        assert_eq!(probe.implementation_version.as_deref(), Some("1.2.3"));
        assert_eq!(probe.auth, AcpAgentAuthStatus::Required);
        authenticate_acp_agent(&agent, "browser")
            .await
            .expect("authenticate fake ACP");

        let malformed =
            fake_agent("import sys; sys.stdin.readline(); print('not-json', flush=True)");
        assert!(
            probe_acp_agent(&malformed)
                .await
                .unwrap_err()
                .contains("failed")
        );
        let crash = fake_agent("raise SystemExit(17)");
        assert!(
            probe_acp_agent(&crash)
                .await
                .unwrap_err()
                .contains("failed")
        );
        let stderr = fake_agent(
            "import sys; sys.stderr.write('supersecret=' + ('x' * 100000)); sys.stderr.flush(); raise SystemExit(17)",
        );
        assert_eq!(
            probe_acp_agent(&stderr).await.unwrap_err(),
            "ACP initialization failed"
        );
        let hanging = fake_agent("import time; time.sleep(60)");
        assert_eq!(
            probe_acp_agent_with_timeout(&hanging, Duration::from_millis(50))
                .await
                .unwrap_err(),
            "ACP initialization timed out"
        );
        assert!(matches!(
            map_process_loss("lost after approval".to_string(), true),
            crate::daemon::RuntimeError::OutcomeUncertain
        ));
        assert!(matches!(
            map_process_loss("lost before approval".to_string(), false),
            crate::daemon::RuntimeError::Protocol(_)
        ));
    }

    #[tokio::test]
    async fn fake_stdio_acp_covers_streaming_terminal_launch_and_cancellation() {
        let marker =
            std::env::temp_dir().join(format!("noema-acp-shell-marker-{}", std::process::id()));
        let shell_argument = format!("; touch {}", marker.display());
        let (store, run, fence, context) =
            acp_execution_fixture(TERMINAL_RESPONDER, vec![shell_argument]).await;
        let cancellation = CancellationToken::new();
        let outcome = execute_acp_run(store.clone(), &run, &fence, &context, &cancellation)
            .await
            .expect("execute fake ACP run");
        let AcpRunOutcome::Terminal(terminal) = outcome else {
            panic!("expected ACP blocking terminal");
        };
        let WorkRunTerminal::Blocked(blocked) = *terminal else {
            panic!("expected ACP blocking terminal");
        };
        assert_eq!(blocked.prompt_markdown, "Which target should I use?");
        assert!(
            !marker.exists(),
            "ACP arguments must never be shell-evaluated"
        );

        let stored_run = store
            .get_work_run_record(&run.run_id)
            .await
            .expect("read ACP run")
            .expect("ACP run exists");
        assert_eq!(
            stored_run.acp_session_id.as_deref(),
            Some("session:fake-terminal")
        );
        let transcript = store
            .list_work_run_items(WorkRunItemQuery {
                owner: WorkRunItemOwnerScope {
                    workspace_id: WorkspaceId::new(PERSONAL_WORKSPACE_ID).unwrap(),
                    task_id: Some(run.task_id.clone()),
                },
                run_id: run.run_id.clone(),
                first: noema_store::WorkPageSize::new(20).unwrap(),
                before: None,
            })
            .await
            .expect("read ACP transcript");
        assert!(
            transcript
                .edges
                .iter()
                .any(
                    |edge| edge.node.content_text.as_deref() == Some("Working through ACP")
                        && edge.node.status == AgentRunItemStatus::Completed
                )
        );
        let tool_items = transcript
            .edges
            .iter()
            .filter(|edge| edge.node.correlation_id.as_deref() == Some("tool:fake"))
            .map(|edge| (edge.node.kind, edge.node.status))
            .collect::<Vec<_>>();
        assert!(tool_items.contains(&(AgentRunItemKind::ToolCall, AgentRunItemStatus::Running)));
        assert!(
            tool_items.contains(&(AgentRunItemKind::ToolResult, AgentRunItemStatus::Completed))
        );

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let marker = std::env::temp_dir().join(format!(
            "noema-acp-cancel-marker-{}-{unique}",
            std::process::id()
        ));
        let (store, run, fence, context) = acp_execution_fixture(
            CANCELLATION_RESPONDER,
            vec![marker.to_string_lossy().into_owned()],
        )
        .await;
        let cancellation = CancellationToken::new();
        let cancellation_signal = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancellation_signal.cancel();
        });
        let Err(error) = execute_acp_run(store, &run, &fence, &context, &cancellation).await else {
            panic!("cancelled ACP run must stop");
        };
        assert!(matches!(error, crate::daemon::RuntimeError::Protocol(_)));
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(
            !marker.exists(),
            "ACP cancellation must terminate descendants in the process group"
        );
    }
}
