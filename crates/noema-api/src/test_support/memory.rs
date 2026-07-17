use std::{sync::Arc, time::Duration};

use noema_memory::{
    AddMemoryRequest, ListMemoriesRequest, ListMemoriesResponse, MemoryOperationError,
    MemoryOperationFuture, MemoryOperations, MemoryRepositoryHandle, MemoryServiceAccess,
    MemoryServiceAccessFuture, MemoryServiceMode, MemoryServiceReadiness, MemoryServiceSnapshot,
    SearchMemoriesRequest, SearchMemoriesResponse,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TEST_MEMORY_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(super) struct TestMemoryServiceAccess {
    repository: MemoryRepositoryHandle,
}

impl TestMemoryServiceAccess {
    pub(super) fn new(repository: MemoryRepositoryHandle) -> Self {
        Self { repository }
    }
}

impl MemoryServiceAccess for TestMemoryServiceAccess {
    fn resolve(&self) -> MemoryServiceAccessFuture<'_> {
        Box::pin(async move {
            let settings = self.repository.memory_service_settings().await?;
            let operations = match (&settings.mode, &settings.base_url) {
                (MemoryServiceMode::External, Some(base_url)) => {
                    Some(Arc::new(TestHttpMemoryOperations::new(base_url.clone()))
                        as noema_memory::MemoryOperationsHandle)
                }
                _ => None,
            };
            Ok(MemoryServiceSnapshot::new(settings, operations))
        })
    }
}

#[derive(Debug)]
struct TestHttpMemoryOperations {
    base_url: String,
}

impl TestHttpMemoryOperations {
    fn new(base_url: String) -> Self {
        Self { base_url }
    }

    async fn request(
        &self,
        method: &str,
        target: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, MemoryOperationError> {
        tokio::time::timeout(
            TEST_MEMORY_REQUEST_TIMEOUT,
            self.request_without_timeout(method, target, body),
        )
        .await
        .map_err(|_| MemoryOperationError::TimedOut)?
    }

    async fn request_without_timeout(
        &self,
        method: &str,
        target: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, MemoryOperationError> {
        let endpoint = TestHttpEndpoint::parse(&self.base_url)?;
        let mut stream = tokio::net::TcpStream::connect(&endpoint.authority)
            .await
            .map_err(|_| MemoryOperationError::RequestFailed)?;
        let target = endpoint.target(target);
        let body = body.unwrap_or_default();
        let mut request = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            endpoint.authority
        );
        if !body.is_empty() {
            use std::fmt::Write as _;
            write!(
                request,
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            )
            .map_err(|_| MemoryOperationError::RequestFailed)?;
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|_| MemoryOperationError::RequestFailed)?;
        if !body.is_empty() {
            stream
                .write_all(&body)
                .await
                .map_err(|_| MemoryOperationError::RequestFailed)?;
        }
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|_| MemoryOperationError::RequestFailed)?;
        parse_http_response(&response)
    }
}

impl MemoryOperations for TestHttpMemoryOperations {
    fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness> {
        Box::pin(async move {
            self.request("GET", "/health", None).await?;
            Ok(MemoryServiceReadiness { ready: true })
        })
    }

    fn add_memory(&self, request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()> {
        Box::pin(async move {
            let body = serde_json::to_vec(&request)
                .map_err(|_| MemoryOperationError::UnreadableResponse)?;
            self.request("POST", "/v1/memories/add", Some(body)).await?;
            Ok(())
        })
    }

    fn search_memories(
        &self,
        request: SearchMemoriesRequest,
    ) -> MemoryOperationFuture<'_, SearchMemoriesResponse> {
        Box::pin(async move {
            let body = serde_json::to_vec(&request)
                .map_err(|_| MemoryOperationError::UnreadableResponse)?;
            let response = self
                .request("POST", "/v1/memories/search", Some(body))
                .await?;
            serde_json::from_slice(&response).map_err(|_| MemoryOperationError::UnreadableResponse)
        })
    }

    fn list_memories(
        &self,
        request: ListMemoriesRequest,
    ) -> MemoryOperationFuture<'_, ListMemoriesResponse> {
        Box::pin(async move {
            let target = format!(
                "/v1/memories?user_id={}&limit={}",
                percent_encode(&request.user_id),
                request.limit
            );
            let response = self.request("GET", &target, None).await?;
            serde_json::from_slice(&response).map_err(|_| MemoryOperationError::UnreadableResponse)
        })
    }
}

struct TestHttpEndpoint {
    authority: String,
    base_path: String,
}

impl TestHttpEndpoint {
    fn parse(base_url: &str) -> Result<Self, MemoryOperationError> {
        let remainder = base_url
            .strip_prefix("http://")
            .ok_or(MemoryOperationError::RequestFailed)?;
        let (authority, base_path) = remainder
            .split_once('/')
            .map_or((remainder, String::new()), |(authority, path)| {
                (authority, format!("/{path}"))
            });
        if authority.is_empty() {
            return Err(MemoryOperationError::RequestFailed);
        }
        let authority = if authority.contains(':') {
            authority.to_string()
        } else {
            format!("{authority}:80")
        };
        Ok(Self {
            authority,
            base_path: base_path.trim_end_matches('/').to_string(),
        })
    }

    fn target(&self, suffix: &str) -> String {
        format!("{}{suffix}", self.base_path)
    }
}

fn parse_http_response(response: &[u8]) -> Result<Vec<u8>, MemoryOperationError> {
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(MemoryOperationError::UnreadableResponse)?;
    let head = std::str::from_utf8(&response[..split])
        .map_err(|_| MemoryOperationError::UnreadableResponse)?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or(MemoryOperationError::UnreadableResponse)?;
    if status == 401 || status == 403 {
        return Err(MemoryOperationError::AuthenticationRejected);
    }
    if !(200..300).contains(&status) {
        return Err(MemoryOperationError::UnsuccessfulStatus(status));
    }
    Ok(response[split + 4..].to_vec())
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}
