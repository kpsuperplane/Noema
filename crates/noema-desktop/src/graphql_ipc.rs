//! Tauri IPC commands for GraphQL operations.

use async_graphql::{Request, Response};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, State, Window};

use crate::desktop_state::DesktopState;

#[derive(Clone, Debug, Serialize)]
struct SubscriptionEventPayload {
    #[serde(rename = "subscriptionId")]
    subscription_id: String,
    response: Value,
}

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
/// Returns a user-facing error if the subscription cannot be decoded or started.
#[tauri::command]
pub async fn graphql_subscribe(
    window: Window,
    state: State<'_, DesktopState>,
    subscription_id: String,
    request_json: Value,
) -> Result<(), String> {
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let schema = state.schema().await?;
    let stream_id = subscription_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut stream = schema.execute_stream(request);
        while let Some(response) = stream.next().await {
            let response_json = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(_) => break,
            };
            let payload = SubscriptionEventPayload {
                subscription_id: stream_id.clone(),
                response: response_json,
            };
            if window.emit("graphql_subscription_event", payload).is_err() {
                break;
            }
        }
    });
    state.insert_subscription(subscription_id, handle).await
}

/// Stop a GraphQL subscription stream.
///
/// # Errors
///
/// This command currently does not fail.
#[tauri::command]
pub async fn graphql_unsubscribe(
    state: State<'_, DesktopState>,
    subscription_id: String,
) -> Result<(), String> {
    state.remove_subscription(&subscription_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_event_payload_keeps_subscription_id() {
        let payload = SubscriptionEventPayload {
            subscription_id: "sub_1".to_string(),
            response: serde_json::json!({"data": {"ok": true}}),
        };

        assert_eq!(payload.subscription_id, "sub_1");
        assert_eq!(payload.response["data"]["ok"], true);
    }
}
