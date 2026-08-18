use super::WorkerState;
use crate::WebBrowseError;
use futures_util::StreamExt;
use noema_capabilities::web::browse::{
    BrowseCommand, BrowseResponse, WEB_BROWSE_CLOSE_TOOL, WEB_BROWSE_HISTORY_TOOL,
    WEB_BROWSE_INTERACT_TOOL, WEB_BROWSE_OPEN_TOOL, WEB_BROWSE_SNAPSHOT_TOOL, WEB_BROWSE_WAIT_TOOL,
    parse_command,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Stdio, sync::Arc, thread, time::Duration};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};
use tokio_util::{codec::LinesCodec, sync::CancellationToken};

const PROTOCOL_VERSION: u8 = 1;
const WORKER_ARGUMENT: &str = "--noema-browser-worker-v1";
const MAX_FRAME_BYTES: usize = 2 * 1_024 * 1_024;
const START_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct WorkerRequest {
    pub(super) command: BrowseCommand,
    pub(super) response: oneshot::Sender<Result<BrowseResponse, WebBrowseError>>,
}

#[derive(Clone)]
pub(super) struct WorkerHandle(Arc<WorkerConnection>);

struct WorkerConnection {
    sender: mpsc::Sender<WorkerRequest>,
    cancellation: CancellationToken,
    alive: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for WorkerConnection {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl WorkerHandle {
    pub(super) fn sender(&self) -> &mpsc::Sender<WorkerRequest> {
        &self.0.sender
    }

    pub(super) fn is_alive(&self) -> bool {
        self.0.alive.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(super) fn cancel(&self) {
        self.0.cancellation.cancel();
    }
}

#[derive(Clone)]
pub(super) struct WorkerLaunch {
    program: Option<PathBuf>,
    arguments: Vec<String>,
    max_old_space_mb: usize,
}

impl WorkerLaunch {
    pub(super) fn current_executable(max_old_space_mb: usize) -> Self {
        Self {
            program: None,
            arguments: Vec::new(),
            max_old_space_mb,
        }
    }

    #[cfg(test)]
    pub(super) fn command(program: PathBuf, arguments: Vec<String>) -> Self {
        Self {
            program: Some(program),
            arguments,
            max_old_space_mb: 1_024,
        }
    }

    fn command_for(&self, generation: u64) -> Result<Command, WebBrowseError> {
        let program = self
            .program
            .clone()
            .map(Ok)
            .unwrap_or_else(std::env::current_exe)
            .map_err(|_| WebBrowseError::Unavailable)?;
        let mut command = Command::new(program);
        if self.arguments.is_empty() {
            command
                .arg(WORKER_ARGUMENT)
                .arg(self.max_old_space_mb.to_string())
                .arg(generation.to_string());
        } else {
            command.args(&self.arguments);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        Ok(command)
    }
}

pub(super) fn spawn_worker(
    launch: WorkerLaunch,
    generation: u64,
) -> Result<WorkerHandle, WebBrowseError> {
    let (sender, receiver) = mpsc::channel(4);
    let cancellation = CancellationToken::new();
    let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let thread_cancellation = cancellation.clone();
    let thread_alive = alive.clone();
    thread::Builder::new()
        .name(format!("noema-browser-{generation}"))
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                thread_alive.store(false, std::sync::atomic::Ordering::Release);
                return;
            };
            runtime.block_on(run_connection(
                launch,
                generation,
                receiver,
                thread_cancellation,
                thread_alive,
            ));
        })
        .map_err(|_| WebBrowseError::Unavailable)?;
    Ok(WorkerHandle(Arc::new(WorkerConnection {
        sender,
        cancellation,
        alive,
    })))
}

async fn run_connection(
    launch: WorkerLaunch,
    generation: u64,
    mut receiver: mpsc::Receiver<WorkerRequest>,
    cancellation: CancellationToken,
    alive: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut child = match launch
        .command_for(generation)
        .and_then(|mut command| command.spawn().map_err(unavailable))
    {
        Ok(child) => child,
        Err(_) => {
            alive.store(false, std::sync::atomic::Ordering::Release);
            receiver.close();
            return;
        }
    };
    let result = async {
        let stdin = child.stdin.take().ok_or(WebBrowseError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(WebBrowseError::Unavailable)?;
        let mut lines = tokio_util::codec::FramedRead::new(
            stdout,
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        let handshake = tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            handshake = tokio::time::timeout(START_TIMEOUT, lines.next()) => handshake
                .map_err(|_| WebBrowseError::Unavailable)?
                .ok_or(WebBrowseError::Unavailable)?
                .map_err(unavailable)?,
        };
        let handshake: Handshake = serde_json::from_str(&handshake).map_err(unavailable)?;
        if handshake.version != PROTOCOL_VERSION || !handshake.ready {
            return Err(WebBrowseError::Unavailable);
        }
        serve_requests(&mut child, stdin, lines, &mut receiver, &cancellation).await
    }
    .await;
    terminate(&mut child).await;
    alive.store(false, std::sync::atomic::Ordering::Release);
    if result.is_err() {
        receiver.close();
    }
}

async fn serve_requests(
    child: &mut Child,
    mut stdin: impl AsyncWrite + Unpin,
    mut lines: tokio_util::codec::FramedRead<tokio::process::ChildStdout, LinesCodec>,
    receiver: &mut mpsc::Receiver<WorkerRequest>,
    cancellation: &CancellationToken,
) -> Result<(), WebBrowseError> {
    loop {
        let request = tokio::select! {
            () = cancellation.cancelled() => break,
            _status = child.wait() => return Err(WebBrowseError::Unavailable),
            request = receiver.recv() => match request {
                Some(request) => request,
                None => break,
            },
        };
        let frame = RequestFrame::from_command(request.command.clone());
        write_frame(&mut stdin, &frame).await?;
        let line = tokio::select! {
            () = cancellation.cancelled() => break,
            line = lines.next() => line
                .ok_or(WebBrowseError::Unavailable)?
                .map_err(unavailable)?,
        };
        let response: ResponseFrame = serde_json::from_str(&line).map_err(unavailable)?;
        let result = response.into_result()?;
        let closes = matches!(request.command, BrowseCommand::Close);
        let _ = request.response.send(result);
        if closes {
            break;
        }
    }
    Ok(())
}

async fn terminate(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Handshake {
    version: u8,
    ready: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestFrame {
    version: u8,
    tool: String,
    arguments: Value,
}

impl RequestFrame {
    fn from_command(command: BrowseCommand) -> Self {
        let (tool, arguments) = command_parts(command);
        Self {
            version: PROTOCOL_VERSION,
            tool: tool.to_string(),
            arguments,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseFrame {
    version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<BrowseResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ResponseFrame {
    fn into_result(self) -> Result<Result<BrowseResponse, WebBrowseError>, WebBrowseError> {
        if self.version != PROTOCOL_VERSION {
            return Err(WebBrowseError::Unavailable);
        }
        match (self.response, self.error) {
            (Some(response), None) => Ok(Ok(response)),
            (None, Some(error)) => Ok(Err(decode_error(&error)?)),
            _ => Err(WebBrowseError::Unavailable),
        }
    }

    fn from_result(result: Result<BrowseResponse, WebBrowseError>) -> Self {
        match result {
            Ok(response) => Self {
                version: PROTOCOL_VERSION,
                response: Some(response),
                error: None,
            },
            Err(error) => Self {
                version: PROTOCOL_VERSION,
                response: None,
                error: Some(encode_error(error).to_string()),
            },
        }
    }
}

async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    frame: &impl Serialize,
) -> Result<(), WebBrowseError> {
    let mut bytes = serde_json::to_vec(frame).map_err(unavailable)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(WebBrowseError::Unavailable);
    }
    bytes.push(b'\n');
    writer.write_all(&bytes).await.map_err(unavailable)?;
    writer.flush().await.map_err(unavailable)
}

fn command_parts(command: BrowseCommand) -> (&'static str, Value) {
    match command {
        BrowseCommand::Open(request) => (WEB_BROWSE_OPEN_TOOL, navigation_arguments(request)),
        BrowseCommand::Snapshot { max_chars } => {
            (WEB_BROWSE_SNAPSHOT_TOOL, json!({"max_chars": max_chars}))
        }
        BrowseCommand::Interact(request) => (
            WEB_BROWSE_INTERACT_TOOL,
            json!({"snapshot_revision": request.snapshot_revision, "ref": request.reference, "action": request.action, "value": request.value}),
        ),
        BrowseCommand::Wait(request) => (
            WEB_BROWSE_WAIT_TOOL,
            json!({"text": request.text, "ref": request.reference, "timeout_ms": request.timeout_ms}),
        ),
        BrowseCommand::History(request) => (
            WEB_BROWSE_HISTORY_TOOL,
            json!({"snapshot_revision": request.snapshot_revision, "action": request.action}),
        ),
        BrowseCommand::Close => (WEB_BROWSE_CLOSE_TOOL, json!({})),
    }
}

fn navigation_arguments(
    request: noema_capabilities::web::browse::BrowseNavigationRequest,
) -> Value {
    json!({"url": request.url, "reason": request.reason, "wait_until": request.wait_until})
}

fn encode_error(error: WebBrowseError) -> &'static str {
    match error {
        WebBrowseError::InvalidUrl => "invalid_url",
        WebBrowseError::BlockedTarget => "blocked_target",
        WebBrowseError::SessionNotFound => "session_not_found",
        WebBrowseError::Capacity => "capacity",
        WebBrowseError::StaleSnapshot => "stale_snapshot",
        WebBrowseError::ElementNotFound => "element_not_found",
        WebBrowseError::Timeout => "timeout",
        WebBrowseError::HistoryUnavailable => "history_unavailable",
        WebBrowseError::NavigationFailed => "navigation_failed",
        WebBrowseError::Unavailable => "unavailable",
        WebBrowseError::OutcomeUncertain => "outcome_uncertain",
    }
}

fn decode_error(value: &str) -> Result<WebBrowseError, WebBrowseError> {
    match value {
        "invalid_url" => Ok(WebBrowseError::InvalidUrl),
        "blocked_target" => Ok(WebBrowseError::BlockedTarget),
        "session_not_found" => Ok(WebBrowseError::SessionNotFound),
        "capacity" => Ok(WebBrowseError::Capacity),
        "stale_snapshot" => Ok(WebBrowseError::StaleSnapshot),
        "element_not_found" => Ok(WebBrowseError::ElementNotFound),
        "timeout" => Ok(WebBrowseError::Timeout),
        "history_unavailable" => Ok(WebBrowseError::HistoryUnavailable),
        "navigation_failed" => Ok(WebBrowseError::NavigationFailed),
        "unavailable" => Ok(WebBrowseError::Unavailable),
        "outcome_uncertain" => Ok(WebBrowseError::OutcomeUncertain),
        _ => Err(WebBrowseError::Unavailable),
    }
}

fn unavailable(_: impl std::fmt::Debug) -> WebBrowseError {
    WebBrowseError::Unavailable
}

pub(crate) fn run_if_requested() -> Option<i32> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(WORKER_ARGUMENT)) {
        return None;
    }
    let max_old_space_mb = arguments
        .next()
        .and_then(|value| value.to_str().and_then(|value| value.parse().ok()));
    let generation = arguments
        .next()
        .and_then(|value| value.to_str().and_then(|value| value.parse().ok()));
    if arguments.next().is_some()
        || !max_old_space_mb.is_some_and(|value| (256..=4_096).contains(&value))
        || generation.is_none()
    {
        eprintln!("invalid Noema browser worker arguments");
        return Some(2);
    }
    let max_old_space_mb = max_old_space_mb.expect("validated browser heap limit");
    let generation = generation.expect("validated browser generation");
    obscura_js::set_v8_flags(&format!("--max-old-space-size={max_old_space_mb}"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    Some(match runtime {
        Ok(runtime) => runtime.block_on(run_worker(generation)).map_or(1, |()| 0),
        Err(error) => {
            eprintln!("failed to start Noema browser worker: {error}");
            1
        }
    })
}

async fn run_worker(generation: u64) -> Result<(), WebBrowseError> {
    let mut state = WorkerState::new(generation);
    let mut input = tokio_util::codec::FramedRead::new(
        tokio::io::stdin(),
        LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
    );
    let mut output = tokio::io::stdout();
    write_frame(
        &mut output,
        &Handshake {
            version: PROTOCOL_VERSION,
            ready: true,
        },
    )
    .await?;
    while let Some(line) = input.next().await {
        let line = line.map_err(unavailable)?;
        let request: RequestFrame = serde_json::from_str(&line).map_err(unavailable)?;
        if request.version != PROTOCOL_VERSION {
            return Err(WebBrowseError::Unavailable);
        }
        let command = parse_command(&request.tool, &request.arguments)
            .map_err(|_| WebBrowseError::Unavailable)?;
        let closes = matches!(command, BrowseCommand::Close);
        let response = ResponseFrame::from_result(state.execute(command).await);
        write_frame(&mut output, &response).await?;
        if closes {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn protocol_rejects_oversized_and_ambiguous_frames() {
        let (mut writer, reader) = tokio::io::duplex(MAX_FRAME_BYTES + 2);
        let write = tokio::spawn(async move {
            writer
                .write_all(&vec![b'x'; MAX_FRAME_BYTES + 1])
                .await
                .expect("write oversized frame");
            writer.write_all(b"\n").await.expect("write delimiter");
        });
        let mut lines = tokio_util::codec::FramedRead::new(
            reader,
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        assert!(lines.next().await.expect("oversized frame result").is_err());
        write.await.expect("writer task");

        let ambiguous = ResponseFrame {
            version: PROTOCOL_VERSION,
            response: Some(BrowseResponse {
                provider: "obscura".to_string(),
                state: "open".to_string(),
                snapshot: None,
                screenshot: None,
            }),
            error: Some("unavailable".to_string()),
        };
        assert_eq!(ambiguous.into_result(), Err(WebBrowseError::Unavailable));
        assert_eq!(
            decode_error(encode_error(WebBrowseError::NavigationFailed)),
            Ok(WebBrowseError::NavigationFailed)
        );
    }
}
