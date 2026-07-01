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
    tauri::Builder::default()
        .manage(desktop_state::DesktopState::new())
        .setup(|app| {
            let state = app.state::<desktop_state::DesktopState>();
            let provider = noema_core::Config::load_daemon(
                None,
                noema_core::CliOverrides::default(),
            )
            .map(|config| config.provider)
            .unwrap_or_else(|_| {
                noema_core::ProviderConfig::Codex(noema_core::CodexProviderConfig::default())
            });
            tauri::async_runtime::block_on(state.initialize(provider)).map_err(|error| {
                std::io::Error::other(format!(
                    "{} {}",
                    error.user_message(),
                    error.technical_details()
                ))
            })?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<desktop_state::DesktopState>();
                    state.shutdown().await;
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
        .run(tauri::generate_context!())
        .expect("failed to run Noema desktop app");
}

#[tauri::command]
async fn mcp_oauth_callback_url(
    state: tauri::State<'_, desktop_state::DesktopState>,
) -> Result<String, String> {
    state.mcp_oauth_callback_url().await
}
