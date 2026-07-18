//! Native Noema desktop application entrypoint.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod desktop_state;
mod external_url;
mod graphql_ipc;
mod mcp_oauth_callback;

fn main() {
    noema_desktop_main();
}

fn noema_desktop_main() {
    let app = tauri::Builder::default()
        .manage(desktop_state::DesktopState::new())
        .setup(|app| {
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
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Noema desktop app");
    let handle = app.handle().clone();
    let exit_code = app.run_return(|_, _| {});
    tauri::async_runtime::block_on(handle.state::<desktop_state::DesktopState>().shutdown());
    std::process::exit(exit_code);
}

#[tauri::command]
async fn mcp_oauth_callback_url(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<String, String> {
    state.mcp_oauth_callback_url().await
}
