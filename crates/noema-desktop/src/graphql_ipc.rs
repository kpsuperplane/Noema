//! Tauri IPC commands for GraphQL operations.

use async_graphql::{Request, Response};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager, State, Window};
use tokio::sync::oneshot;

use crate::{
    desktop_state::{DesktopState, GraphqlTarget},
    remote_graphql::{RemoteError, RemoteSubscriptionEvent},
};

#[derive(Clone, Debug, Serialize)]
struct SubscriptionEventPayload {
    #[serde(rename = "subscriptionId")]
    subscription_id: String,
    response: Value,
}

#[derive(Clone, Debug, Serialize)]
struct ConnectionChangedPayload {
    state: &'static str,
}

pub(crate) fn require_main_window_label(label: &str) -> Result<(), String> {
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
pub(crate) async fn graphql_execute(
    window: Window,
    state: State<'_, DesktopState>,
    request_json: Value,
) -> Result<Value, String> {
    require_main_window_label(window.label())?;
    match state.graphql_target().await? {
        GraphqlTarget::Local(schema) => {
            let request: Request = serde_json::from_value(request_json)
                .map_err(|_| "Noema lost connection to its local app service.".to_string())?;
            let response: Response = schema
                .execute(request.data(noema_api::RequestPrincipal::local()))
                .await;
            serde_json::to_value(response)
                .map_err(|_| "Noema lost connection to its local app service.".to_string())
        }
        GraphqlTarget::Remote(remote) => remote.execute(request_json).await.map_err(|error| {
            emit_connection_change(&window, &error);
            remote_error_message(error)
        }),
    }
}

/// Start a GraphQL subscription stream.
///
/// # Errors
///
/// Returns a user-facing error if the subscription cannot be decoded or started.
#[tauri::command]
pub(crate) async fn graphql_subscribe(
    window: Window,
    state: State<'_, DesktopState>,
    subscription_id: String,
    request_json: Value,
) -> Result<(), String> {
    require_main_window_label(window.label())?;
    let target = state.graphql_target().await?;
    let (generation_tx, generation_rx) = oneshot::channel();
    let event_id = subscription_id.clone();
    let task_cleanup_id = subscription_id.clone();
    let registration_cleanup_id = subscription_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let Ok(generation) = generation_rx.await else {
            return;
        };
        match target {
            GraphqlTarget::Local(schema) => {
                if let Ok(request) = serde_json::from_value::<Request>(request_json) {
                    let request = request.data(noema_api::RequestPrincipal::local());
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
                }
            }
            GraphqlTarget::Remote(remote) => {
                remote
                    .subscribe(request_json, |event| match event {
                        RemoteSubscriptionEvent::Next(response) => window
                            .emit(
                                "graphql_subscription_event",
                                SubscriptionEventPayload {
                                    subscription_id: event_id.clone(),
                                    response,
                                },
                            )
                            .is_ok(),
                        RemoteSubscriptionEvent::Offline => {
                            let _ = window.emit(
                                "desktop_connection_changed",
                                ConnectionChangedPayload { state: "offline" },
                            );
                            true
                        }
                        RemoteSubscriptionEvent::Unauthorized => {
                            let _ = window.emit(
                                "desktop_connection_changed",
                                ConnectionChangedPayload {
                                    state: "unauthorized",
                                },
                            );
                            false
                        }
                        RemoteSubscriptionEvent::Complete => false,
                    })
                    .await;
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

fn emit_connection_change(window: &Window, error: &RemoteError) {
    let state = match error {
        RemoteError::Unauthorized => "unauthorized",
        RemoteError::Offline => "offline",
        RemoteError::InvalidResponse => "unavailable",
    };
    let _ = window.emit(
        "desktop_connection_changed",
        ConnectionChangedPayload { state },
    );
}

fn remote_error_message(error: RemoteError) -> String {
    match error {
        RemoteError::Unauthorized => {
            "This desktop client no longer has access to the server.".to_string()
        }
        RemoteError::Offline => "Noema could not reach the remote server.".to_string(),
        RemoteError::InvalidResponse => {
            "The remote server returned an invalid response.".to_string()
        }
    }
}

/// Stop a GraphQL subscription stream.
///
/// # Errors
///
/// This command currently does not fail.
#[tauri::command]
pub(crate) async fn graphql_unsubscribe(
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
    fn ipc_boundary_preserves_frontend_wire_key_and_main_window_authority() {
        let payload = SubscriptionEventPayload {
            subscription_id: "sub_1".to_string(),
            response: serde_json::json!({"data": {"ok": true}}),
        };

        let serialized =
            serde_json::to_value(&payload).expect("serialize subscription event payload");

        assert_eq!(serialized["subscriptionId"], "sub_1");
        assert!(serialized.get("subscription_id").is_none());
        assert_eq!(serialized["response"]["data"]["ok"], true);
        assert!(require_main_window_label("main").is_ok());
        assert!(require_main_window_label("secondary").is_err());
        assert!(require_main_window_label("").is_err());

        let changed = serde_json::to_value(ConnectionChangedPayload { state: "offline" })
            .expect("serialize connection change");
        assert_eq!(changed, serde_json::json!({"state": "offline"}));
    }
}
