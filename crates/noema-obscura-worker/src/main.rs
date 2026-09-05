//! Process-isolated Obscura browser worker for the Go server.
mod browser;
mod error;
mod protocol;
mod public_url;
mod snapshot;
mod url_policy;
use browser::WorkerState;
use error::WebBrowseError;
use futures_util::StreamExt;
use protocol::{BrowseCommand, BrowseResponse, parse_command};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_util::codec::LinesCodec;
const OBSCURA_BROWSER_PROVIDER_ID: &str = "obscura";
const PROTOCOL_VERSION: u8 = 1;
const WORKER_ARGUMENT: &str = "--noema-browser-worker-v1";
const MAX_FRAME_BYTES: usize = 2 * 1_024 * 1_024;
fn main() {
    std::process::exit(run_if_requested().unwrap_or(2));
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseFrame {
    version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<BrowseResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diagnostic: Option<ProviderDiagnostic>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderDiagnostic {
    provider: String,
    stage: String,
    detail: String,
}

impl ResponseFrame {
    fn from_result(result: Result<BrowseResponse, WebBrowseError>) -> Self {
        match result {
            Ok(response) => Self {
                version: PROTOCOL_VERSION,
                response: Some(response),
                error: None,
                diagnostic: None,
            },
            Err(WebBrowseError::ProviderFailure {
                kind,
                provider,
                stage,
                detail,
            }) => Self {
                version: PROTOCOL_VERSION,
                response: None,
                error: Some(encode_error(*kind).to_string()),
                diagnostic: Some(ProviderDiagnostic {
                    provider,
                    stage,
                    detail,
                }),
            },
            Err(error) => Self {
                version: PROTOCOL_VERSION,
                response: None,
                error: Some(encode_error(error).to_string()),
                diagnostic: None,
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

fn encode_error(error: WebBrowseError) -> &'static str {
    match error {
        WebBrowseError::InvalidUrl => "invalid_url",
        WebBrowseError::BlockedTarget => "blocked_target",
        WebBrowseError::StaleSnapshot => "stale_snapshot",
        WebBrowseError::ElementNotFound => "element_not_found",
        WebBrowseError::Timeout => "timeout",
        WebBrowseError::HistoryUnavailable => "history_unavailable",
        WebBrowseError::NavigationFailed => "navigation_failed",
        WebBrowseError::Unavailable => "unavailable",
        WebBrowseError::OutcomeUncertain => "outcome_uncertain",
        WebBrowseError::ProviderFailure { .. } => "unavailable",
    }
}

fn unavailable(_: impl std::fmt::Debug) -> WebBrowseError {
    WebBrowseError::Unavailable
}

fn run_if_requested() -> Option<i32> {
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
    async fn protocol_preserves_bounds_and_provider_diagnostics() {
        let (mut writer, reader) = tokio::io::duplex(MAX_FRAME_BYTES + 2);
        let write = tokio::spawn(async move {
            writer
                .write_all(&vec![b'x'; MAX_FRAME_BYTES + 1])
                .await
                .expect("oversized frame");
            writer.write_all(b"\n").await.expect("delimiter");
        });
        let mut lines = tokio_util::codec::FramedRead::new(
            reader,
            LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
        );
        assert!(lines.next().await.expect("frame result").is_err());
        write.await.expect("writer task");
        let frame = ResponseFrame::from_result(Err(WebBrowseError::OutcomeUncertain
            .with_provider_detail("obscura", "navigation", "navigation exceeded its deadline")));
        let encoded = serde_json::to_value(frame).expect("wire response");
        assert_eq!(encoded["error"], "outcome_uncertain");
        assert_eq!(
            encoded["diagnostic"]["detail"],
            "navigation exceeded its deadline"
        );
        assert!(encoded.get("response").is_none());
    }
}
