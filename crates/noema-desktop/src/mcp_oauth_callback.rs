//! Local loopback callback server for desktop MCP OAuth setup.

use noema_api::graphql::GraphqlState;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const CALLBACK_PATH: &str = "/mcp/oauth/callback";
const MAX_REQUEST_BYTES: usize = 16 * 1024;

pub(crate) async fn start(
    graphql_state: GraphqlState,
) -> Result<(String, tauri::async_runtime::JoinHandle<()>), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| "Noema could not start its MCP OAuth callback listener.".to_string())?;
    let address = listener
        .local_addr()
        .map_err(|_| "Noema could not read its MCP OAuth callback listener.".to_string())?;
    let base_url = format!("http://{}{}", address, CALLBACK_PATH);
    let handle = tauri::async_runtime::spawn(async move {
        while let Ok((stream, _peer)) = listener.accept().await {
            let state = graphql_state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle_connection(stream, state, &address.to_string()).await;
            });
        }
    });
    Ok((base_url, handle))
}

async fn handle_connection(
    mut stream: TcpStream,
    state: GraphqlState,
    expected_authority: &str,
) -> Result<(), std::io::Error> {
    macro_rules! reject {
        ($status:literal, $message:literal) => {{
            write_response(&mut stream, $status, $message).await?;
            return Ok(());
        }};
    }
    let request = match read_request(&mut stream).await {
        Ok(request) => request,
        Err(_) => reject!(
            "400 Bad Request",
            "Noema could not read this MCP OAuth callback."
        ),
    };
    if request.method != "GET" || request.path != CALLBACK_PATH {
        reject!("404 Not Found", "not found");
    }
    if request.host != expected_authority {
        reject!("400 Bad Request", "Invalid MCP OAuth callback authority.");
    }
    let Some(query) = request.query.as_deref() else {
        reject!("400 Bad Request", "Missing MCP OAuth callback query.");
    };
    let Some(attempt_id) = query_value(query, "attemptId") else {
        reject!("400 Bad Request", "Missing MCP OAuth attempt id.");
    };
    let callback_url = request.callback_url(expected_authority);
    let result =
        noema_api::graphql::complete_mcp_server_oauth_setup(&state, &attempt_id, &callback_url)
            .await;
    match result {
        Ok(attempt) if attempt.status == "completed" => {
            write_response(
                &mut stream,
                "200 OK",
                "Authentication completed. You can return to Noema.",
            )
            .await?;
        }
        Ok(_) => {
            write_response(
                &mut stream,
                "200 OK",
                "Authentication finished, but Noema could not list tools. Return to Noema to retry.",
            )
            .await?;
        }
        Err(_) => {
            write_response(
                &mut stream,
                "400 Bad Request",
                "Noema could not complete this MCP OAuth setup attempt.",
            )
            .await?;
        }
    }
    Ok(())
}

struct CallbackRequest {
    method: String,
    path: String,
    query: Option<String>,
    host: String,
}

impl CallbackRequest {
    fn callback_url(&self, authority: &str) -> String {
        format!(
            "http://{}{}{}",
            authority,
            self.path,
            self.query
                .as_deref()
                .map_or_else(String::new, |query| format!("?{query}"))
        )
    }
}

async fn read_request(stream: &mut TcpStream) -> Result<CallbackRequest, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| "invalid request".to_string())?;
        if read == 0 {
            return Err("missing request".to_string());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("request too large".to_string());
        }
    };
    let text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "invalid request headers".to_string())?;
    parse_request_head(text)
}

fn parse_request_head(text: &str) -> Result<CallbackRequest, String> {
    let mut lines = text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let target = parts.next().ok_or_else(|| "missing target".to_string())?;
    let host = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _value)| name.trim().eq_ignore_ascii_case("host"))
        .map(|(_name, value)| value.trim().to_string())
        .ok_or_else(|| "missing host".to_string())?;
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), Some(query.to_string())),
        None => (target.to_string(), None),
    };
    Ok(CallbackRequest {
        method,
        path,
        query,
        host,
    })
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _value)| name == key)
        .map(|(_name, value)| value.into_owned())
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<(), std::io::Error> {
    let body = format!("<!doctype html><title>Noema MCP OAuth</title><p>{message}</p>");
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_callback_request_head() {
        let request = parse_request_head(
            "GET /mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123 HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n",
        )
        .expect("request");

        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/mcp/oauth/callback");
        assert_eq!(
            query_value(request.query.as_deref().expect("query"), "attemptId").as_deref(),
            Some("mcp_oauth:abc")
        );
        assert_eq!(
            request.callback_url("127.0.0.1:4444"),
            "http://127.0.0.1:4444/mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123"
        );
        assert_ne!(request.host, "127.0.0.1:5555");
    }
}
