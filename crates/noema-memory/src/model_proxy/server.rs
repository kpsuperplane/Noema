//! Loopback listener, connection lifecycle, and request routing.

use noema_home::{SystemErrorEvent, SystemErrorLogger};
use noema_providers::GenerateStreamEvent;
use serde_json::json;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinSet,
};

use super::{
    MemoryModelProxyError, RunningMemoryModelProxyConfig, log_proxy_error,
    protocol::{
        HttpRequest, HttpResponse, json_error, json_error_message, json_response, read_http_request,
    },
    translation::{
        OpenAiChatCompletionRequest, openai_response_from_generate_response,
        resolved_model_profile, unix_timestamp,
    },
};

pub(super) async fn run_proxy(
    listener: TcpListener,
    config: RunningMemoryModelProxyConfig,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                break;
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = result {
                    log_connection_result(config.system_errors.as_ref(), result);
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _peer)) => {
                        let config = config.clone();
                        connections.spawn(handle_connection(stream, config));
                    }
                    Err(error) => {
                        log_proxy_error(
                            config.system_errors.as_ref(),
                            "memory_model_proxy_accept_failed",
                            &error.to_string(),
                        );
                        break;
                    }
                }
            }
        }
    }

    drop(listener);
    while let Some(result) = connections.join_next().await {
        log_connection_result(config.system_errors.as_ref(), result);
    }
}

fn log_connection_result(
    system_errors: Option<&SystemErrorLogger>,
    result: Result<Result<(), MemoryModelProxyError>, tokio::task::JoinError>,
) {
    match result {
        Ok(Err(error)) => log_proxy_error(
            system_errors,
            "memory_model_proxy_request_failed",
            &error.to_string(),
        ),
        Err(error) => log_proxy_error(
            system_errors,
            "memory_model_proxy_request_join_failed",
            &error.to_string(),
        ),
        Ok(Ok(())) => {}
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    config: RunningMemoryModelProxyConfig,
) -> Result<(), MemoryModelProxyError> {
    let request = read_http_request(&mut stream).await?;
    let response = route_request(request, config).await;
    stream.write_all(&response.to_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn route_request(
    request: HttpRequest,
    config: RunningMemoryModelProxyConfig,
) -> HttpResponse {
    if request.method != "GET" && request.method != "POST" {
        return json_error(http::StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    }
    if request.path == "/v1/models" && request.method == "GET" {
        let route = match config.route_resolver.resolve_route().await {
            Ok(route) => route,
            Err(error) => {
                return json_error_message(
                    http::StatusCode::BAD_GATEWAY,
                    "provider_unavailable",
                    error.to_string(),
                );
            }
        };
        return json_response(
            http::StatusCode::OK,
            json!({
                "object": "list",
                "data": [{
                    "id": resolved_model_profile(&route),
                    "object": "model",
                    "created": unix_timestamp(),
                    "owned_by": "noema"
                }]
            }),
        );
    }
    if request.path != "/v1/chat/completions" || request.method != "POST" {
        return json_error(http::StatusCode::NOT_FOUND, "not_found");
    }
    let Some(authorization) = request.header("authorization") else {
        return json_error(http::StatusCode::UNAUTHORIZED, "missing_authorization");
    };
    if authorization != format!("Bearer {}", config.api_key) {
        return json_error(http::StatusCode::UNAUTHORIZED, "invalid_authorization");
    }
    let openai_request = match serde_json::from_slice::<OpenAiChatCompletionRequest>(&request.body)
    {
        Ok(request) => request,
        Err(error) => {
            return json_error_message(
                http::StatusCode::BAD_REQUEST,
                "invalid_json",
                error.to_string(),
            );
        }
    };
    if openai_request.stream.unwrap_or(false) {
        if let Some(system_errors) = &config.system_errors {
            system_errors.try_append(SystemErrorEvent::new(
                "memory_model_proxy_streaming_unsupported",
                "Memory service requested streaming from the private model proxy",
            ));
        }
        return json_error(http::StatusCode::NOT_IMPLEMENTED, "streaming_not_supported");
    }
    let route = match config.route_resolver.resolve_route().await {
        Ok(route) => route,
        Err(error) => {
            return json_error_message(
                http::StatusCode::BAD_GATEWAY,
                "provider_unavailable",
                error.to_string(),
            );
        }
    };
    let generate_request = match openai_request.into_generate_request(&route) {
        Ok(request) => request,
        Err(error) => {
            return json_error_message(
                http::StatusCode::BAD_REQUEST,
                "invalid_request",
                error.to_string(),
            );
        }
    };
    let mut ignore_event = |_event: GenerateStreamEvent| {};
    match route
        .operations()
        .generate_streaming(generate_request, &mut ignore_event)
        .await
    {
        Ok(response) => json_response(
            http::StatusCode::OK,
            openai_response_from_generate_response(response),
        ),
        Err(error) => {
            if let Some(system_errors) = &config.system_errors {
                system_errors.try_append(
                    SystemErrorEvent::new(
                        "memory_model_proxy_provider_failed",
                        "Memory model proxy provider request failed",
                    )
                    .with_error_chain([error.to_string()]),
                );
            }
            json_error_message(
                http::StatusCode::BAD_GATEWAY,
                "provider_failed",
                error.to_string(),
            )
        }
    }
}
