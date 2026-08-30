//! Shared bounded parsing for model-visible file tools.

use cap_std::{ambient_authority, fs::Dir};
use futures_util::StreamExt;
use noema_capabilities::file::{
    FileParseResponse, FileParseStatus, HARD_MAX_CHARS, parse_arguments, parse_download_arguments,
};
use noema_providers::{CheckedUrl, validate_public_url, validate_public_url_parsed};
use noema_store::NoemaStore;
use noema_tasks::TaskId;
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    ffi::OsStr,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{io::AsyncWriteExt, process::Command, sync::Semaphore, time::Duration};

const WORKER_ENV: &str = "NOEMA_FILE_PARSE_WORKER";
const WORKER_FORMAT_ENV: &str = "NOEMA_FILE_PARSE_FORMAT";
const WORKER_MAX_CHARS_ENV: &str = "NOEMA_FILE_PARSE_MAX_CHARS";
const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
const WORKER_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DOWNLOAD_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const DOWNLOAD_USER_AGENT: &str = "NoemaFileDownload/0.1 (+https://github.com/kpsuperplane/Noema)";

static DOCUMENT_PARSE_PERMIT: OnceLock<Semaphore> = OnceLock::new();
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

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

pub(crate) async fn execute_file_download(
    store: &NoemaStore,
    task_id: Option<&str>,
    cwd: Option<&str>,
    payload: &Value,
) -> Result<Value, String> {
    let request = parse_download_arguments(payload)?;
    let root_path = match task_id {
        Some(value) => {
            let task_id = TaskId::new(value.to_string()).map_err(|error| error.to_string())?;
            store
                .task_working_directory(&task_id)
                .await
                .map_err(|error| error.to_string())?
        }
        None => PathBuf::from(
            cwd.ok_or_else(|| "conversation working directory is unavailable".to_string())?,
        ),
    };
    let relative = normalized_relative_path(&request.path)?;
    if task_id.is_some()
        && relative
            .parent()
            .is_none_or(|parent| parent.as_os_str().is_empty())
        && matches!(
            relative.to_str(),
            Some("TASK.md" | "RESULT.md" | "REVIEW.md")
        )
    {
        return Err("destination is a reserved Task file".to_string());
    }
    let root = Dir::open_ambient_dir(&root_path, ambient_authority())
        .map_err(|_| "working directory is unavailable".to_string())?;
    prepare_download_parent(&root, &relative)?;
    if root.symlink_metadata(&relative).is_ok() {
        return Err("destination already exists".to_string());
    }
    let temp = temporary_path(&relative);
    let std_file = root
        .open_with(
            &temp,
            cap_std::fs::OpenOptions::new().write(true).create_new(true),
        )
        .map(cap_std::fs::File::into_std)
        .map_err(|_| "temporary download file could not be created".to_string())?;
    let downloaded =
        match tokio::time::timeout(IO_TIMEOUT, download_into(std_file, &request.url)).await {
            Ok(result) => result,
            Err(_) => {
                let _ = root.remove_file(&temp);
                return Err("download timed out".to_string());
            }
        };
    let (final_url, saved_bytes, media_type) = match downloaded {
        Ok(value) => value,
        Err(error) => {
            let _ = root.remove_file(&temp);
            return Err(error);
        }
    };
    commit_download(&root, &temp, &relative)?;
    let parse = if request.parse {
        let file = root
            .open(&relative)
            .map(cap_std::fs::File::into_std)
            .map_err(|_| "downloaded file could not be opened".to_string())?;
        Some(
            parse_open_file(
                file,
                &request.path,
                media_type.as_deref(),
                request.max_chars,
            )
            .await,
        )
    } else {
        None
    };
    Ok(serde_json::json!({
        "url": request.url, "final_url": final_url, "path": request.path,
        "saved_bytes": saved_bytes, "media_type": media_type, "parse": parse,
    }))
}

async fn download_into(
    file: std::fs::File,
    raw_url: &str,
) -> Result<(String, u64, Option<String>), String> {
    let mut checked = validate_public_url(raw_url)
        .await
        .map_err(|error| error.to_string())?;
    let mut file = tokio::fs::File::from_std(file);
    for redirect_count in 0..=MAX_REDIRECTS {
        let client = client_for_checked_url(&checked)?;
        let response = client
            .get(checked.url.clone())
            .header(header::USER_AGENT, DOWNLOAD_USER_AGENT)
            .timeout(IO_TIMEOUT)
            .send()
            .await
            .map_err(|_| "download request failed".to_string())?;
        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err("download has too many redirects".to_string());
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "download redirect is invalid".to_string())?;
            let next = checked
                .url
                .join(location)
                .map_err(|_| "download redirect is invalid".to_string())?;
            checked = validate_public_url_parsed(next)
                .await
                .map_err(|error| error.to_string())?;
            continue;
        }
        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => "download timed out",
                _ => "download request failed",
            }
            .to_string());
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_DOWNLOAD_BYTES)
        {
            return Err("download exceeds the size limit".to_string());
        }
        let media_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);
        if matches!(
            media_type.as_deref(),
            Some("text/html" | "application/xhtml+xml")
        ) {
            return Err("HTML responses cannot be downloaded with file.download".to_string());
        }
        let mut saved = 0_u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "download request failed".to_string())?;
            saved = saved.saturating_add(chunk.len() as u64);
            if saved > MAX_DOWNLOAD_BYTES {
                return Err("download exceeds the size limit".to_string());
            }
            file.write_all(&chunk)
                .await
                .map_err(|_| "download write failed".to_string())?;
        }
        file.flush()
            .await
            .map_err(|_| "download write failed".to_string())?;
        return Ok((checked.url.to_string(), saved, media_type));
    }
    unreachable!("redirect loop returns")
}

fn client_for_checked_url(checked: &CheckedUrl) -> Result<Client, String> {
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(IO_TIMEOUT);
    if let Some(url::Host::Domain(host)) = checked.url.host() {
        builder = builder.resolve_to_addrs(host, &checked.resolved_addrs);
    }
    builder
        .build()
        .map_err(|_| "download client is unavailable".to_string())
}

fn normalized_relative_path(supplied: &str) -> Result<PathBuf, String> {
    let supplied = Path::new(supplied.trim());
    let mut relative = PathBuf::new();
    for component in supplied.components() {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            _ => return Err("file path is outside the working-directory boundary".to_string()),
        }
    }
    if relative.as_os_str().is_empty() {
        return Err("file path is required".to_string());
    }
    Ok(relative)
}

fn prepare_download_parent(root: &Dir, relative: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in relative.parent().into_iter().flat_map(Path::components) {
        current.push(component.as_os_str());
        match root.symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("file path contains a symbolic link".to_string());
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err("download parent is not a directory".to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => root
                .create_dir(&current)
                .map_err(|_| "download directory could not be created".to_string())?,
            Err(_) => return Err("download directory is unavailable".to_string()),
        }
    }
    Ok(())
}

fn temporary_path(relative: &Path) -> PathBuf {
    let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let name = relative
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("download");
    relative.with_file_name(format!(".{name}.noema-{id}.tmp"))
}

fn commit_download(root: &Dir, temp: &Path, destination: &Path) -> Result<(), String> {
    if let Err(error) = root.hard_link(temp, root, destination) {
        let _ = root.remove_file(temp);
        return Err(if root.symlink_metadata(destination).is_ok() {
            "destination already exists".to_string()
        } else {
            format!("download could not be committed: {error}")
        });
    }
    root.remove_file(temp)
        .map_err(|_| "temporary download file could not be removed".to_string())
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
    let (parser, content_format, worker) = if is_image_format(format_hint.as_deref(), media_type) {
        (
            "tesseract",
            "text",
            run_ocr_worker(&bytes, format_hint.as_deref(), max_chars).await,
        )
    } else {
        (
            "anydoc",
            "markdown",
            run_document_worker(&bytes, format_hint.as_deref(), max_chars).await,
        )
    };
    response_from_worker(display_path, source_bytes, parser, content_format, worker)
}

/// Parse bounded artifact bytes for an authorized human preview.
///
/// This uses the same text and isolated document parsers as the model file tool.
pub async fn parse_artifact_preview(
    bytes: &[u8],
    display_path: &str,
    media_type: Option<&str>,
) -> FileParseResponse {
    let source_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let format_hint = format_hint(display_path, media_type);
    if is_text_format(format_hint.as_deref(), media_type) {
        return text_response(
            bytes,
            display_path,
            source_bytes,
            format_hint,
            HARD_MAX_CHARS,
            false,
        );
    }
    if source_bytes > MAX_DOCUMENT_BYTES {
        return failed(display_path, source_bytes, format_hint, "source_too_large");
    }
    response_from_worker(
        display_path,
        source_bytes,
        "anydoc",
        "markdown",
        run_document_worker(bytes, format_hint.as_deref(), HARD_MAX_CHARS).await,
    )
}

fn open_conversation_file(cwd: &str, supplied: &str) -> Result<std::fs::File, String> {
    let relative = normalized_relative_path(supplied)?;
    let root = Dir::open_ambient_dir(cwd, ambient_authority())
        .map_err(|_| "working directory is unavailable".to_string())?;
    let mut current = PathBuf::new();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata = root
            .symlink_metadata(&current)
            .map_err(|_| "file is unavailable".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("file path contains a symbolic link".to_string());
        }
    }
    let file = root
        .open(&relative)
        .map_err(|_| "file could not be opened".to_string())?;
    let metadata = file
        .metadata()
        .map_err(|_| "file is unavailable".to_string())?;
    if !metadata.is_file() {
        return Err("file is not a regular file".to_string());
    }
    Ok(file.into_std())
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
    text_response(
        &bytes,
        path,
        source_bytes,
        format,
        max_chars,
        source_has_more,
    )
}

fn text_response(
    bytes: &[u8],
    path: &str,
    source_bytes: u64,
    format: Option<String>,
    max_chars: usize,
    source_has_more: bool,
) -> FileParseResponse {
    let text = match std::str::from_utf8(bytes) {
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
        content_format: Some(
            match format.as_deref() {
                Some("csv") => "csv",
                Some("md" | "markdown") => "markdown",
                _ => "text",
            }
            .to_string(),
        ),
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
    let output = tokio::time::timeout(IO_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "worker_timeout")?
        .map_err(|_| "worker_failed")?;
    drop(permit);
    if !output.status.success() {
        return Err("worker_failed");
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "worker_failed")
}

async fn run_ocr_worker(
    bytes: &[u8],
    format_hint: Option<&str>,
    max_chars: usize,
) -> Result<WorkerResponse, &'static str> {
    let permit = DOCUMENT_PARSE_PERMIT
        .get_or_init(|| Semaphore::new(1))
        .acquire()
        .await
        .map_err(|_| "worker_unavailable")?;
    let mut child = Command::new("prlimit")
        .arg(format!("--as={WORKER_MEMORY_BYTES}"))
        .args(["--", "tesseract", "stdin", "stdout", "-l", "eng"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "ocr_unavailable")?;
    let mut stdin = child.stdin.take().ok_or("worker_unavailable")?;
    stdin.write_all(bytes).await.map_err(|_| "worker_failed")?;
    stdin.shutdown().await.map_err(|_| "worker_failed")?;
    drop(stdin);
    let output = tokio::time::timeout(IO_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "worker_timeout")?
        .map_err(|_| "worker_failed")?;
    drop(permit);
    if !output.status.success() {
        return Err("ocr_unavailable");
    }
    let text = String::from_utf8(output.stdout).map_err(|_| "worker_failed")?;
    let observed = text.chars().count();
    let content = text.chars().take(max_chars).collect::<String>();
    let returned_chars = content.chars().count();
    Ok(WorkerResponse {
        status: FileParseStatus::Converted,
        format: format_hint.map(str::to_string),
        content: Some(content),
        returned_chars,
        truncated: observed > returned_chars,
        error: None,
    })
}

fn response_from_worker(
    path: &str,
    source_bytes: u64,
    parser: &str,
    content_format: &str,
    worker: Result<WorkerResponse, &'static str>,
) -> FileParseResponse {
    match worker {
        Ok(worker) => FileParseResponse {
            path: path.to_string(),
            source_bytes,
            status: worker.status,
            parser: Some(parser.to_string()),
            format: worker.format,
            content_format: worker.content.as_ref().map(|_| content_format.to_string()),
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
        Some("message/rfc822") => Some("eml"),
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
    if let Some(media_type) = media_type.map(str::trim).filter(|value| !value.is_empty()) {
        return media_type.starts_with("text/")
            || matches!(
                media_type.split(';').next(),
                Some(
                    "application/json"
                        | "application/xml"
                        | "application/markdown"
                        | "message/rfc822"
                )
            );
    }
    matches!(
        format,
        Some("csv" | "txt" | "text" | "md" | "markdown" | "eml" | "json" | "xml")
    )
}

fn is_image_format(format: Option<&str>, media_type: Option<&str>) -> bool {
    media_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().starts_with("image/"))
        || matches!(
            format,
            Some("bmp" | "gif" | "jpg" | "jpeg" | "png" | "tif" | "tiff" | "webp")
        )
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
mod tests;
