//! Native Noema desktop application entrypoint.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

mod desktop_profile;
mod desktop_state;
mod external_url;
mod graphql_ipc;
mod mcp_oauth_callback;
mod remote_graphql;
mod remote_pairing;

fn main() {
    if let Some(status) = noema_host::run_browser_worker_if_requested() {
        std::process::exit(status);
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _directory| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            },
        ))
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let config_path = app
                .path()
                .app_config_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .join("desktop.json");
            app.manage(desktop_state::DesktopState::new(config_path));
            let state = app.state::<desktop_state::DesktopState>();
            let local_model_runtime_root = app
                .path()
                .resource_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .join("binaries")
                .join("runtime");
            tauri::async_runtime::block_on(state.initialize(Some(local_model_runtime_root)))
                .map_err(|error| {
                    std::io::Error::other(format!("{} {error}", error.user_message()))
                })?;
            if let Some(urls) = app
                .deep_link()
                .get_current()
                .map_err(|error| std::io::Error::other(error.to_string()))?
            {
                for url in urls {
                    stage_deep_link(app.handle().clone(), url.to_string());
                }
            }
            let deep_link_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    stage_deep_link(deep_link_handle.clone(), url.to_string());
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let handle = window.app_handle().clone();
                let window = window.clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<desktop_state::DesktopState>();
                    state.shutdown().await;
                    let _ = window.destroy();
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            graphql_ipc::graphql_execute,
            graphql_ipc::graphql_subscribe,
            graphql_ipc::graphql_unsubscribe,
            external_url::open_external_url,
            mcp_oauth_callback_url,
            desktop_connection_status,
            desktop_retry_remote,
            desktop_stage_pairing,
            desktop_cancel_pairing,
            desktop_complete_pairing,
            desktop_use_local,
            desktop_forget_remote,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Noema desktop app");
    let handle = app.handle().clone();
    let exit_code = app.run_return(|_, _| {});
    tauri::async_runtime::block_on(handle.state::<desktop_state::DesktopState>().shutdown());
    std::process::exit(exit_code);
}

fn stage_deep_link(app: tauri::AppHandle, pairing_uri: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<desktop_state::DesktopState>();
        if let Ok(stage) = state.stage_pairing(&pairing_uri).await {
            let _ = app.emit("desktop_pairing_pending", stage);
        }
    });
}

#[tauri::command]
async fn mcp_oauth_callback_url(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<String, String> {
    state.mcp_oauth_callback_url().await
}

#[tauri::command]
async fn desktop_connection_status(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<desktop_state::DesktopConnectionStatus, String> {
    Ok(state.connection_status().await)
}

#[tauri::command]
async fn desktop_retry_remote(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<desktop_state::DesktopConnectionStatus, String> {
    Ok(state.connection_status().await)
}

#[tauri::command]
async fn desktop_stage_pairing(
    window: tauri::Window,
    state: tauri::State<'_, desktop_state::DesktopState>,
    pairing_uri: String,
) -> Result<remote_pairing::PairingStage, String> {
    if window.label() != "main" {
        return Err("Noema rejected a request from an unauthorized app window.".to_string());
    }
    state.stage_pairing(&pairing_uri).await
}

#[tauri::command]
async fn desktop_cancel_pairing(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    state.cancel_pairing().await;
    Ok(())
}

#[tauri::command]
async fn desktop_complete_pairing(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
    display_name: String,
) -> Result<(), String> {
    state.complete_pairing(&display_name).await?;
    app.restart();
}

#[tauri::command]
async fn desktop_use_local(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    state.disconnect_remote().await?;
    app.restart();
}

#[tauri::command]
async fn desktop_forget_remote(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    state.forget_remote()?;
    app.restart();
}
