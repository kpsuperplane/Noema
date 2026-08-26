//! Local loopback callback server for desktop OAuth setup.

use noema_api::graphql::GraphqlState;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use crate::loopback_http;

const MCP_CALLBACK_PATH: &str = "/mcp/oauth/callback";
const PROVIDER_CALLBACK_PATH: &str = "/provider/oauth/callback";
const ADAPTER_CALLBACK_PATH: &str = "/adapter/oauth/callback";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_QUERY_BYTES: usize = 8 * 1024;

pub(crate) struct OAuthCallbackUrls {
    pub(crate) mcp: String,
    pub(crate) adapter: String,
    pub(crate) provider: String,
}

pub(crate) async fn start(
    graphql_state: GraphqlState,
) -> Result<
    (
        GraphqlState,
        OAuthCallbackUrls,
        tauri::async_runtime::JoinHandle<()>,
    ),
    String,
> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| "Noema could not start its MCP OAuth callback listener.".to_string())?;
    let address = listener
        .local_addr()
        .map_err(|_| "Noema could not read its MCP OAuth callback listener.".to_string())?;
    let urls = OAuthCallbackUrls {
        mcp: format!("http://{address}{MCP_CALLBACK_PATH}"),
        adapter: format!("http://{address}{ADAPTER_CALLBACK_PATH}"),
        provider: format!("http://{address}{PROVIDER_CALLBACK_PATH}"),
    };
    let graphql_state = graphql_state
        .with_mcp_oauth_callback_url(urls.mcp.clone())
        .with_provider_oauth_callback_url(urls.provider.clone())
        .with_adapter_oauth_callback_url(urls.adapter.clone());
    let callback_state = graphql_state.clone();
    let handle = tauri::async_runtime::spawn(async move {
        while let Ok((stream, _peer)) = listener.accept().await {
            let state = callback_state.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle_connection(stream, state, &address.to_string()).await;
            });
        }
    });
    Ok((graphql_state, urls, handle))
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
    if request.method != "GET" {
        reject!("404 Not Found", "not found");
    }
    if request.host.as_deref() != Some(expected_authority) {
        reject!("400 Bad Request", "Invalid MCP OAuth callback authority.");
    }
    let Some(query) = request.query() else {
        reject!("400 Bad Request", "Missing OAuth callback query.");
    };
    if query.len() > MAX_QUERY_BYTES {
        reject!("400 Bad Request", "Invalid OAuth callback query.");
    }
    let callback_url = request.callback_url(expected_authority);
    match request.path() {
        MCP_CALLBACK_PATH => {
            let Some(attempt_id) = query_value(query, "attemptId") else {
                reject!("400 Bad Request", "Missing MCP OAuth attempt id.");
            };
            let result = noema_api::graphql::complete_mcp_server_oauth_setup(
                &state,
                &attempt_id,
                &callback_url,
            )
            .await;
            let (status, message) = match result {
                Ok(attempt) if attempt.status == "completed" => (
                    "200 OK",
                    "Authentication completed. You can return to Noema.",
                ),
                Ok(_) => (
                    "200 OK",
                    "Authentication finished, but Noema could not list tools. Return to Noema to retry.",
                ),
                Err(_) => (
                    "400 Bad Request",
                    "Noema could not complete this MCP OAuth setup attempt.",
                ),
            };
            write_response(&mut stream, status, message).await?;
        }
        ADAPTER_CALLBACK_PATH => {
            let (status, message) =
                match noema_api::graphql::complete_adapter_oauth_setup(&state, &callback_url).await
                {
                    Ok(_) => (
                        "200 OK",
                        "Authentication completed. You can return to Noema.",
                    ),
                    Err(error) => (
                        "400 Bad Request",
                        noema_api::graphql::adapter_oauth_failure_message(error),
                    ),
                };
            write_response(&mut stream, status, message).await?;
        }
        path if path
            .strip_prefix(PROVIDER_CALLBACK_PATH)
            .is_some_and(|suffix| suffix.starts_with('/')) =>
        {
            if query_value(query, "code").is_none() {
                reject!("400 Bad Request", "Invalid provider OAuth callback.");
            }
            let (status, message) =
                match noema_api::graphql::complete_provider_oauth_callback(&state, &callback_url)
                    .await
                {
                    Ok(_) => (
                        "200 OK",
                        "Authentication completed. You can return to Noema.",
                    ),
                    Err(_) => (
                        "400 Bad Request",
                        "Noema could not complete this provider connection.",
                    ),
                };
            write_response(&mut stream, status, message).await?;
        }
        _ => reject!("404 Not Found", "not found"),
    }
    Ok(())
}

async fn read_request(stream: &mut TcpStream) -> Result<loopback_http::RequestHead, String> {
    loopback_http::read_request_head(stream, MAX_REQUEST_BYTES).await
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
    let body = format!("<!doctype html><title>Noema OAuth</title><p>{message}</p>");
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
        let request = loopback_http::parse_request_head(
            "GET /mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123 HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n",
        )
        .expect("request");

        assert_eq!(request.method, "GET");
        assert_eq!(request.path(), MCP_CALLBACK_PATH);
        assert_eq!(
            query_value(request.query().expect("query"), "attemptId").as_deref(),
            Some("mcp_oauth:abc")
        );
        assert_eq!(
            request.callback_url("127.0.0.1:4444"),
            "http://127.0.0.1:4444/mcp/oauth/callback?attemptId=mcp_oauth%3Aabc&code=123"
        );
        assert_ne!(request.host.as_deref(), Some("127.0.0.1:5555"));

        let adapter = loopback_http::parse_request_head(
            "GET /adapter/oauth/callback?code=123&state=abc HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n",
        )
        .expect("adapter request");
        assert_eq!(adapter.path(), ADAPTER_CALLBACK_PATH);
        assert_eq!(
            adapter.callback_url("127.0.0.1:4444"),
            "http://127.0.0.1:4444/adapter/oauth/callback?code=123&state=abc"
        );

        let provider = loopback_http::parse_request_head(
            "GET /provider/oauth/callback/abcdEFGH01234567ijklMNOP89012345?code=123 HTTP/1.1\r\nHost: 127.0.0.1:4444\r\n\r\n",
        )
        .expect("provider request");
        assert_eq!(
            provider.callback_url("127.0.0.1:4444"),
            "http://127.0.0.1:4444/provider/oauth/callback/abcdEFGH01234567ijklMNOP89012345?code=123"
        );
    }
}
