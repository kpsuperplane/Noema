//! Shared bounded parsing for model-visible file tools.

use cap_std::{ambient_authority, fs::Dir};
use noema_capabilities::file::{
    FileParseResponse, FileParseStatus, HARD_MAX_CHARS, parse_arguments,
};
use noema_store::NoemaStore;
use noema_tasks::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    ffi::OsStr,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
};
use tokio::{io::AsyncWriteExt, process::Command, sync::Semaphore, time::Duration};

const WORKER_ENV: &str = "NOEMA_FILE_PARSE_WORKER";
const WORKER_FORMAT_ENV: &str = "NOEMA_FILE_PARSE_FORMAT";
const WORKER_MAX_CHARS_ENV: &str = "NOEMA_FILE_PARSE_MAX_CHARS";
const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
const WORKER_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const WORKER_TIMEOUT: Duration = Duration::from_secs(30);

static DOCUMENT_PARSE_PERMIT: OnceLock<Semaphore> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
struct WorkerResponse {
    status: FileParseStatus,
    format: Option<String>,
    content: Option<String>,
    returned_chars: usize,
    truncated: bool,
    error: Option<String>,
}

pub(crate) async fn execute_file_parse(
    store: &NoemaStore,
    task_id: Option<&str>,
    cwd: Option<&str>,
    payload: &Value,
) -> Result<Value, String> {
    let request = parse_arguments(payload)?;
    let file = match task_id {
        Some(task_id) => {
            let task_id = TaskId::new(task_id.to_string()).map_err(|error| error.to_string())?;
            store
                .open_task_file_for_read(&task_id, &request.path)
                .await
                .map_err(|error| error.to_string())?
        }
        None => open_conversation_file(
            cwd.ok_or_else(|| "conversation working directory is unavailable".to_string())?,
            &request.path,
        )?,
    };
    let response = parse_open_file(file, &request.path, None, request.max_chars).await;
    serde_json::to_value(response).map_err(|_| "file parse result could not be encoded".to_string())
}

pub(crate) async fn parse_open_file(
    mut file: std::fs::File,
    display_path: &str,
    media_type: Option<&str>,
    max_chars: usize,
) -> FileParseResponse {
    let source_bytes = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata.len(),
        _ => return failed(display_path, 0, None, "invalid_file"),
    };
    let format_hint = format_hint(display_path, media_type);
    if is_text_format(format_hint.as_deref(), media_type) {
        return parse_text(file, display_path, source_bytes, format_hint, max_chars);
    }
    if source_bytes > MAX_DOCUMENT_BYTES {
        return failed(display_path, source_bytes, format_hint, "source_too_large");
    }
    let mut bytes = Vec::with_capacity(source_bytes as usize);
    if file.seek(SeekFrom::Start(0)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return failed(display_path, source_bytes, format_hint, "read_failed");
    }
    let worker = run_document_worker(&bytes, format_hint.as_deref(), max_chars).await;
    response_from_worker(display_path, source_bytes, worker)
}

fn open_conversation_file(cwd: &str, supplied: &str) -> Result<std::fs::File, String> {
    let supplied = Path::new(supplied.trim());
    if supplied.is_absolute()
        || supplied
            .components()
            .any(|part| part == Component::ParentDir)
    {
        return Err("file path is outside the working-directory boundary".to_string());
    }
    let mut relative = PathBuf::new();
    for component in supplied.components() {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("file path is outside the working-directory boundary".to_string());
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err("file path is required".to_string());
    }
    let root = Dir::open_ambient_dir(cwd, ambient_authority())
        .map_err(|_| "working directory is unavailable".to_string())?;
    verify_relative_file(&root, &relative)?;
    root.open(&relative)
        .map(cap_std::fs::File::into_std)
        .map_err(|_| "file could not be opened".to_string())
}

fn verify_relative_file(root: &Dir, relative: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err("file path is outside the working-directory boundary".to_string());
        };
        current.push(value);
        let metadata = root
            .symlink_metadata(&current)
            .map_err(|_| "file is unavailable".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("file path contains a symbolic link".to_string());
        }
    }
    let metadata = root
        .symlink_metadata(relative)
        .map_err(|_| "file is unavailable".to_string())?;
    if !metadata.is_file() {
        return Err("file is not a regular file".to_string());
    }
    Ok(())
}

fn parse_text(
    mut file: std::fs::File,
    path: &str,
    source_bytes: u64,
    format: Option<String>,
    max_chars: usize,
) -> FileParseResponse {
    let byte_limit = max_chars
        .saturating_add(1)
        .saturating_mul(4)
        .saturating_add(3);
    let mut bytes = Vec::with_capacity(byte_limit.min(64 * 1024));
    if Read::by_ref(&mut file)
        .take(byte_limit as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return failed(path, source_bytes, format, "read_failed");
    }
    let source_has_more = source_bytes > bytes.len() as u64;
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if error.error_len().is_none() && source_has_more => {
            match std::str::from_utf8(&bytes[..error.valid_up_to()]) {
                Ok(text) => text,
                Err(_) => return failed(path, source_bytes, format, "invalid_utf8"),
            }
        }
        Err(_) => return failed(path, source_bytes, format, "invalid_utf8"),
    };
    let observed_chars = text.chars().count();
    let content: String = text.chars().take(max_chars).collect();
    let returned_chars = content.chars().count();
    FileParseResponse {
        path: path.to_string(),
        source_bytes,
        status: FileParseStatus::Converted,
        parser: Some("utf8".to_string()),
        content_format: Some(if format.as_deref() == Some("csv") {
            "csv".to_string()
        } else {
            "text".to_string()
        }),
        format,
        content: Some(content),
        returned_chars,
        truncated: source_has_more || observed_chars > returned_chars,
        error: None,
    }
}

async fn run_document_worker(
    bytes: &[u8],
    format_hint: Option<&str>,
    max_chars: usize,
) -> Result<WorkerResponse, &'static str> {
    let permit = DOCUMENT_PARSE_PERMIT
        .get_or_init(|| Semaphore::new(1))
        .acquire()
        .await
        .map_err(|_| "worker_unavailable")?;
    let executable = env::current_exe().map_err(|_| "worker_unavailable")?;
    let mut child = Command::new(executable)
        .env(WORKER_ENV, "1")
        .env(WORKER_FORMAT_ENV, format_hint.unwrap_or(""))
        .env(WORKER_MAX_CHARS_ENV, max_chars.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "worker_unavailable")?;
    let mut stdin = child.stdin.take().ok_or("worker_unavailable")?;
    stdin.write_all(bytes).await.map_err(|_| "worker_failed")?;
    stdin.shutdown().await.map_err(|_| "worker_failed")?;
    drop(stdin);
    let output = tokio::time::timeout(WORKER_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "worker_timeout")?
        .map_err(|_| "worker_failed")?;
    drop(permit);
    if !output.status.success() {
        return Err("worker_failed");
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "worker_failed")
}

fn response_from_worker(
    path: &str,
    source_bytes: u64,
    worker: Result<WorkerResponse, &'static str>,
) -> FileParseResponse {
    match worker {
        Ok(worker) => FileParseResponse {
            path: path.to_string(),
            source_bytes,
            status: worker.status,
            parser: Some("anydoc".to_string()),
            format: worker.format,
            content_format: worker.content.as_ref().map(|_| "markdown".to_string()),
            content: worker.content,
            returned_chars: worker.returned_chars,
            truncated: worker.truncated,
            error: worker.error,
        },
        Err(error) => failed(path, source_bytes, None, error),
    }
}

fn failed(path: &str, source_bytes: u64, format: Option<String>, error: &str) -> FileParseResponse {
    FileParseResponse {
        path: path.to_string(),
        source_bytes,
        status: FileParseStatus::Failed,
        parser: None,
        format,
        content_format: None,
        content: None,
        returned_chars: 0,
        truncated: false,
        error: Some(error.to_string()),
    }
}

fn format_hint(path: &str, media_type: Option<&str>) -> Option<String> {
    let media_type = media_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let from_media = match media_type {
        Some("text/csv") => Some("csv"),
        Some("text/plain") => Some("txt"),
        Some("text/markdown" | "application/markdown") => Some("md"),
        Some("application/json") => Some("json"),
        Some("application/xml" | "text/xml") => Some("xml"),
        _ => None,
    };
    from_media.map(str::to_string).or_else(|| {
        Path::new(path)
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
    })
}

fn is_text_format(format: Option<&str>, media_type: Option<&str>) -> bool {
    matches!(
        format,
        Some("csv" | "txt" | "text" | "md" | "markdown" | "json" | "xml")
    ) || media_type.is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("text/"))
}

fn convert_document_bytes(
    bytes: &[u8],
    format_hint: Option<&str>,
    max_chars: usize,
) -> WorkerResponse {
    let format = anydoc::Format::from_bytes(bytes)
        .or_else(|| format_hint.and_then(anydoc::Format::from_extension));
    let Some(format) = format else {
        return WorkerResponse {
            status: FileParseStatus::Unsupported,
            format: format_hint.map(str::to_string),
            content: None,
            returned_chars: 0,
            truncated: false,
            error: Some("unsupported_format".to_string()),
        };
    };
    let format_name = anydoc_format_name(format).to_string();
    match anydoc::to_markdown_bytes(bytes, format) {
        Ok(markdown) => {
            let observed = markdown.chars().count();
            let content: String = markdown.chars().take(max_chars).collect();
            let returned_chars = content.chars().count();
            WorkerResponse {
                status: FileParseStatus::Converted,
                format: Some(format_name),
                content: Some(content),
                returned_chars,
                truncated: observed > returned_chars,
                error: None,
            }
        }
        Err(error) => WorkerResponse {
            status: if matches!(error, anydoc::ConvertError::Unsupported(_)) {
                FileParseStatus::Unsupported
            } else {
                FileParseStatus::Failed
            },
            format: Some(format_name),
            content: None,
            returned_chars: 0,
            truncated: false,
            error: Some(error.code().to_ascii_lowercase()),
        },
    }
}

fn anydoc_format_name(format: anydoc::Format) -> &'static str {
    match format {
        anydoc::Format::Doc => "doc",
        anydoc::Format::Docx => "docx",
        anydoc::Format::Odt => "odt",
        anydoc::Format::Pdf => "pdf",
        anydoc::Format::Ppt => "ppt",
        anydoc::Format::Pptx => "pptx",
        anydoc::Format::Rtf => "rtf",
        anydoc::Format::Epub => "epub",
        anydoc::Format::Excel => "excel",
        anydoc::Format::Ods => "ods",
        anydoc::Format::Odp => "odp",
        anydoc::Format::Csv => "csv",
    }
}

/// Run the private document parser when the process has worker arguments.
///
/// The application entrypoint must exit with the returned status.
#[must_use]
pub fn run_file_parse_worker_if_requested() -> Option<i32> {
    (env::var(WORKER_ENV).ok().as_deref() == Some("1")).then(run_worker_process)
}

fn run_worker_process() -> i32 {
    #[cfg(unix)]
    if rustix::process::setrlimit(
        rustix::process::Resource::As,
        rustix::process::Rlimit {
            current: Some(WORKER_MEMORY_BYTES),
            maximum: Some(WORKER_MEMORY_BYTES),
        },
    )
    .is_err()
    {
        return 2;
    }
    let max_chars = env::var(WORKER_MAX_CHARS_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(HARD_MAX_CHARS)
        .clamp(1000, HARD_MAX_CHARS);
    let format = env::var(WORKER_FORMAT_ENV)
        .ok()
        .filter(|value| !value.is_empty());
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_DOCUMENT_BYTES
    {
        return 2;
    }
    let response = convert_document_bytes(&bytes, format.as_deref(), max_chars);
    match serde_json::to_vec(&response) {
        Ok(output) if std::io::stdout().write_all(&output).is_ok() => 0,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn text_parser_bounds_utf8_without_using_anydoc() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("large.csv");
        std::fs::write(&path, "rank,domain\n1,éxample.com\n".repeat(200)).expect("write");
        let file = std::fs::File::open(&path).expect("open");

        let response = parse_open_file(file, "large.csv", Some("text/csv"), 1000).await;

        assert_eq!(response.status, FileParseStatus::Converted);
        assert_eq!(response.parser.as_deref(), Some("utf8"));
        assert_eq!(response.content_format.as_deref(), Some("csv"));
        assert_eq!(response.returned_chars, 1000);
        assert!(response.truncated);
    }

    #[test]
    fn unsupported_document_has_a_bounded_result() {
        let response = convert_document_bytes(b"not a document", Some("bin"), 1000);
        assert_eq!(response.status, FileParseStatus::Unsupported);
        assert_eq!(response.error.as_deref(), Some("unsupported_format"));
        assert!(response.content.is_none());
    }
}
