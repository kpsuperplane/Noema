//! Tauri IPC commands for GraphQL operations.

use async_graphql::{Request, Response};
use serde_json::Value;
use tauri::State;

use crate::desktop_state::DesktopState;

/// Execute one GraphQL operation through Tauri IPC.
///
/// # Errors
///
/// Returns a user-facing error if the request cannot be decoded or executed.
#[tauri::command]
pub async fn graphql_execute(
    state: State<'_, DesktopState>,
    request_json: Value,
) -> Result<Value, String> {
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let schema = state.schema().await?;
    let response: Response = schema.execute(request).await;
    serde_json::to_value(response)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())
}

/// Start a GraphQL subscription stream.
///
/// # Errors
///
/// Returns a controlled error until streaming is implemented.
#[tauri::command]
pub async fn graphql_subscribe(
    _state: State<'_, DesktopState>,
    _subscription_id: String,
    _request_json: Value,
) -> Result<(), String> {
    Err("Noema lost connection to its local app service.".to_string())
}

/// Stop a GraphQL subscription stream.
///
/// # Errors
///
/// This placeholder currently does not fail.
#[tauri::command]
pub async fn graphql_unsubscribe(
    _state: State<'_, DesktopState>,
    _subscription_id: String,
) -> Result<(), String> {
    Ok(())
}
