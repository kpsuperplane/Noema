//! Filesystem-authenticated GraphQL access for local development tools.

use std::{io, path::PathBuf};

use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use tokio::sync::watch;
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};

use super::MAX_GRAPHQL_BODY_BYTES;
use crate::WebServerError;

/// Bound local GraphQL socket and its isolated router.
pub(crate) struct LocalGraphqlServer {
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
    #[cfg(unix)]
    router: Router,
    #[cfg(unix)]
    _socket: SocketPathGuard,
}

impl LocalGraphqlServer {
    /// Bind one private Unix socket before the public server starts serving.
    pub(crate) async fn bind(
        path: PathBuf,
        schema: noema_api::graphql::GraphqlSchema,
    ) -> Result<Self, WebServerError> {
        #[cfg(not(unix))]
        {
            let _ = (path, schema);
            return Err(WebServerError::Protocol(
                "web.local_graphql_socket requires a Unix host".to_string(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{FileTypeExt, PermissionsExt};

            let parent = path.parent().ok_or_else(|| {
                WebServerError::Protocol(
                    "local GraphQL socket path must have a parent directory".to_string(),
                )
            })?;
            noema_home::ensure_private_dir(parent)?;
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if !metadata.file_type().is_socket() {
                    return Err(WebServerError::Protocol(format!(
                        "local GraphQL socket path is not a socket: {}",
                        path.display()
                    )));
                }
                match tokio::net::UnixStream::connect(&path).await {
                    Ok(_) => {
                        return Err(WebServerError::Protocol(format!(
                            "local GraphQL socket is already active: {}",
                            path.display()
                        )));
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                        ) =>
                    {
                        if path.exists() {
                            std::fs::remove_file(&path)?;
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            let listener = tokio::net::UnixListener::bind(&path)?;
            if let Err(error) =
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            {
                let _ = std::fs::remove_file(&path);
                return Err(error.into());
            }
            Ok(Self {
                listener,
                router: local_router(schema),
                _socket: SocketPathGuard(path),
            })
        }
    }

    /// Serve until the daemon shares its shutdown signal.
    pub(crate) async fn serve(self, shutdown: watch::Receiver<bool>) -> io::Result<()> {
        #[cfg(not(unix))]
        {
            let _ = (self, shutdown);
            unreachable!("unsupported local GraphQL server cannot be constructed")
        }
        #[cfg(unix)]
        {
            let Self {
                listener,
                router,
                _socket,
            } = self;
            axum::serve(listener, router)
                .with_graceful_shutdown(wait_for_shutdown(shutdown))
                .await
        }
    }
}

fn local_router(schema: noema_api::graphql::GraphqlSchema) -> Router {
    let route = post(graphql)
        .head(method_not_found)
        .fallback(method_not_found);
    Router::new()
        .route("/graphql", route)
        .fallback(method_not_found)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(RequestBodyLimitLayer::new(MAX_GRAPHQL_BODY_BYTES))
        .with_state(schema)
}

async fn graphql(
    State(schema): State<noema_api::graphql::GraphqlSchema>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    schema
        .execute(
            request
                .into_inner()
                .data(noema_api::RequestPrincipal::local()),
        )
        .await
        .into()
}

async fn method_not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

async fn wait_for_shutdown(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow() && shutdown.changed().await.is_ok() {}
}

#[cfg(unix)]
struct SocketPathGuard(PathBuf);

#[cfg(unix)]
impl Drop for SocketPathGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(all(test, unix))]
#[path = "local_graphql/tests.rs"]
mod tests;
