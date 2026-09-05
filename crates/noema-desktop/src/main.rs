//! Native Noema desktop application entrypoint.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

mod desktop_profile;
mod desktop_state;
mod external_url;
mod graphql_ipc;
mod remote_graphql;
mod remote_oauth;

fn main() {
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
            let resource_dir = app
                .path()
                .resource_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let local_model_runtime_root = resource_dir
                .join("binaries")
                .join("runtime")
                .join(env!("TAURI_ENV_TARGET_TRIPLE"));
            let server_path = resource_dir.join("binaries").join(if cfg!(windows) {
                "noema-server.exe"
            } else {
                "noema-server"
            });
            tauri::async_runtime::block_on(state.initialize(server_path, local_model_runtime_root))
                .map_err(std::io::Error::other)?;
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
            desktop_stage_connection,
            desktop_cancel_connection,
            desktop_complete_connection,
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

fn stage_deep_link(app: tauri::AppHandle, connection_uri: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<desktop_state::DesktopState>();
        if let Ok(stage) = state.stage_connection(&connection_uri).await {
            let _ = app.emit("desktop_connection_pending", stage);
        }
    });
}

#[tauri::command]
async fn mcp_oauth_callback_url(
    window: tauri::Window,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<String, String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.mcp_oauth_callback_url().await
}

#[tauri::command]
async fn desktop_connection_status(
    window: tauri::Window,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<desktop_state::DesktopConnectionStatus, String> {
    graphql_ipc::require_main_window_label(window.label())?;
    Ok(state.connection_status().await)
}

#[tauri::command]
async fn desktop_stage_connection(
    window: tauri::Window,
    state: tauri::State<'_, desktop_state::DesktopState>,
    server: String,
) -> Result<remote_oauth::ConnectionStage, String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.stage_connection(&server).await
}

#[tauri::command]
async fn desktop_cancel_connection(
    window: tauri::Window,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.cancel_connection().await;
    Ok(())
}

#[tauri::command]
async fn desktop_complete_connection(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.complete_connection().await?;
    app.restart();
}

#[tauri::command]
async fn desktop_use_local(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.disconnect_remote().await?;
    app.restart();
}

#[tauri::command]
async fn desktop_forget_remote(
    window: tauri::Window,
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<(), String> {
    graphql_ipc::require_main_window_label(window.label())?;
    state.forget_remote()?;
    app.restart();
}
