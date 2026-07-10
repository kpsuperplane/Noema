//! Tauri IPC commands for GraphQL operations.

use async_graphql::{Request, Response};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager, State, Window};
use tokio::sync::oneshot;

use crate::desktop_state::DesktopState;

#[derive(Clone, Debug, Serialize)]
struct SubscriptionEventPayload {
    #[serde(rename = "subscriptionId")]
    subscription_id: String,
    response: Value,
}

fn require_main_window_label(label: &str) -> Result<(), String> {
    if label == "main" {
        Ok(())
    } else {
        Err("Noema rejected a request from an unauthorized app window.".to_string())
    }
}

/// Execute one GraphQL operation through Tauri IPC.
///
/// # Errors
///
/// Returns a user-facing error if the request cannot be decoded or executed.
#[tauri::command]
pub async fn graphql_execute(
    window: Window,
    state: State<'_, DesktopState>,
    request_json: Value,
) -> Result<Value, String> {
    require_main_window_label(window.label())?;
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let schema = state.schema().await?;
    let response: Response = schema
        .execute(request.data(noema_core::RequestPrincipal::local()))
        .await;
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
    require_main_window_label(window.label())?;
    let request: Request = serde_json::from_value(request_json)
        .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
    let request = request.data(noema_core::RequestPrincipal::local());
    let schema = state.schema().await?;
    let (generation_tx, generation_rx) = oneshot::channel();
    let event_id = subscription_id.clone();
    let task_cleanup_id = subscription_id.clone();
    let registration_cleanup_id = subscription_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let Ok(generation) = generation_rx.await else {
            return;
        };
        let mut stream = schema.execute_stream(request);
        while let Some(response) = stream.next().await {
            let response_json = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(_) => break,
            };
            let payload = SubscriptionEventPayload {
                subscription_id: event_id.clone(),
                response: response_json,
            };
            if window.emit("graphql_subscription_event", payload).is_err() {
                break;
            }
        }
        window
            .state::<DesktopState>()
            .remove_finished_subscription(&task_cleanup_id, generation)
            .await;
    });
    let generation = state.insert_subscription(subscription_id, handle).await?;
    if generation_tx.send(generation).is_err() {
        state
            .remove_finished_subscription(&registration_cleanup_id, generation)
            .await;
    }
    Ok(())
}

/// Stop a GraphQL subscription stream.
///
/// # Errors
///
/// This command currently does not fail.
#[tauri::command]
pub async fn graphql_unsubscribe(
    window: Window,
    state: State<'_, DesktopState>,
    subscription_id: String,
) -> Result<(), String> {
    require_main_window_label(window.label())?;
    state.remove_subscription(&subscription_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_event_payload_serializes_frontend_wire_key() {
        let payload = SubscriptionEventPayload {
            subscription_id: "sub_1".to_string(),
            response: serde_json::json!({"data": {"ok": true}}),
        };

        let serialized =
            serde_json::to_value(&payload).expect("serialize subscription event payload");

        assert_eq!(serialized["subscriptionId"], "sub_1");
        assert!(serialized.get("subscription_id").is_none());
        assert_eq!(serialized["response"]["data"]["ok"], true);
    }

    #[test]
    fn only_main_window_is_authorized() {
        assert!(require_main_window_label("main").is_ok());
        assert!(require_main_window_label("secondary").is_err());
        assert!(require_main_window_label("").is_err());
    }
}
