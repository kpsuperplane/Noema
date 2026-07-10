use std::{
    future::Future,
    io::{Read, Write},
    pin::Pin,
    sync::Arc,
};

use serde::Serialize;
use tokio::time::{Duration, timeout};

use crate::{
    NoemaPaths, NoemaStore, StoreConfig,
    daemon::{CodexRuntimeHandle, RuntimeModelProvider, TestDaemonWebServer},
    provider::{GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderError},
};

const READY_PREFIX: &str = "NOEMA_BROWSER_READY ";
const MAX_SHUTDOWN_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserScenario {
    Boot,
}

#[derive(Debug)]
struct BrowserFixtureProvider;

impl RuntimeModelProvider for BrowserFixtureProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async {
            Ok(GenerateResponse::final_text(
                "fixture response",
                "fixture",
                "fixture-model",
            ))
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserReadiness<'a> {
    schema_version: u8,
    kind: &'static str,
    scenario: &'static str,
    origin: &'a str,
    start_url: String,
}

fn parse_scenario(value: Option<&str>) -> Result<BrowserScenario, &'static str> {
    match value {
        Some("boot") => Ok(BrowserScenario::Boot),
        Some(_) => Err("browser scenario is unsupported"),
        None => Err("browser scenario is unavailable"),
    }
}

fn serialize_readiness(origin: &str) -> Result<String, &'static str> {
    serde_json::to_string(&BrowserReadiness {
        schema_version: 1,
        kind: "ready",
        scenario: "boot",
        origin,
        start_url: format!("{origin}/"),
    })
    .map_err(|_| "browser readiness serialization failed")
}

fn serialize_readiness_frame(origin: &str) -> Result<String, &'static str> {
    let readiness = serialize_readiness(origin)?;
    Ok(format!("\n{READY_PREFIX}{readiness}\n"))
}

fn parse_shutdown_command(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes == b"shutdown\n" {
        Ok(())
    } else {
        Err("invalid browser fixture shutdown command")
    }
}

fn read_shutdown_command(mut reader: impl Read) -> Result<(), &'static str> {
    let mut command = Vec::with_capacity(MAX_SHUTDOWN_BYTES);
    let mut byte = [0_u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => break,
            Ok(1) => {
                if command.len() == MAX_SHUTDOWN_BYTES {
                    return Err("invalid browser fixture shutdown command");
                }
                command.push(byte[0]);
            }
            Ok(_) => return Err("browser fixture shutdown read failed"),
            Err(_) => return Err("browser fixture shutdown read failed"),
        }
    }
    parse_shutdown_command(&command)
}

#[tokio::test]
#[ignore = "launched by the deterministic browser harness"]
async fn fixture_server() -> Result<(), &'static str> {
    let scenario = std::env::var("NOEMA_BROWSER_SCENARIO").ok();
    parse_scenario(scenario.as_deref())?;
    let paths = NoemaPaths::from_process_env().map_err(|_| "browser fixture home is invalid")?;
    let store = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .map_err(|_| "browser fixture store failed to open")?;
    store
        .ensure_default_actors()
        .await
        .map_err(|_| "browser fixture actors failed to initialize")?;
    store
        .update_agent_display_name("agent:primary", "Browser Fixture")
        .await
        .map_err(|_| "browser fixture agent failed to initialize")?;

    let runtime =
        CodexRuntimeHandle::spawn_with_provider(Arc::new(BrowserFixtureProvider), store.clone())
            .await
            .map_err(|_| "browser fixture runtime failed to start")?;
    let graphql_state = crate::graphql::GraphqlState::for_tests_with_store_runtime_and_paths(
        store.clone(),
        runtime.clone(),
        paths,
    );
    let server = TestDaemonWebServer::start(graphql_state)
        .await
        .map_err(|_| "browser fixture web server failed to start")?;
    let origin = server.base_url();
    let readiness_frame = serialize_readiness_frame(&origin)?;
    {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        output
            .write_all(readiness_frame.as_bytes())
            .map_err(|_| "browser readiness write failed")?;
        output
            .flush()
            .map_err(|_| "browser readiness flush failed")?;
    }

    let shutdown = tokio::task::spawn_blocking(|| read_shutdown_command(std::io::stdin()));
    timeout(Duration::from_secs(45), shutdown)
        .await
        .map_err(|_| "browser fixture shutdown timed out")?
        .map_err(|_| "browser fixture shutdown task failed")??;
    timeout(Duration::from_secs(5), server.shutdown())
        .await
        .map_err(|_| "browser fixture web shutdown timed out")?
        .map_err(|_| "browser fixture web shutdown failed")?;
    runtime.shutdown().await;
    store
        .close()
        .await
        .map_err(|_| "browser fixture store shutdown failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_boot_scenario_without_echoing_unknown_values() {
        assert_eq!(parse_scenario(Some("boot")), Ok(BrowserScenario::Boot));
        assert_eq!(parse_scenario(None), Err("browser scenario is unavailable"));
        assert_eq!(
            parse_scenario(Some("secret-value")),
            Err("browser scenario is unsupported")
        );
    }

    #[test]
    fn serializes_the_exact_bounded_readiness_contract() {
        let serialized = serialize_readiness("http://127.0.0.1:43123").expect("readiness");
        assert_eq!(
            serialized,
            r#"{"schemaVersion":1,"kind":"ready","scenario":"boot","origin":"http://127.0.0.1:43123","startUrl":"http://127.0.0.1:43123/"}"#
        );
        assert!(serialized.len() < 2 * 1024);
    }

    #[test]
    fn readiness_frame_starts_on_its_own_line_after_libtest_status_text() {
        let frame = serialize_readiness_frame("http://127.0.0.1:43123").expect("frame");
        assert!(frame.starts_with("\nNOEMA_BROWSER_READY "));
        assert_eq!(frame.matches(READY_PREFIX).count(), 1);
        assert!(frame.ends_with('\n'));
    }

    #[test]
    fn accepts_only_the_exact_bounded_shutdown_command() {
        assert!(parse_shutdown_command(b"shutdown\n").is_ok());
        for rejected in [
            b"shutdown".as_slice(),
            b"shutdown\r\n".as_slice(),
            b" shutdown\n".as_slice(),
            b"shutdown\nextra".as_slice(),
            b"x".repeat(17).as_slice(),
        ] {
            assert_eq!(
                parse_shutdown_command(rejected),
                Err("invalid browser fixture shutdown command")
            );
        }
    }

    #[test]
    fn bounded_shutdown_reader_rejects_extra_bytes() {
        assert!(read_shutdown_command(b"shutdown\n".as_slice()).is_ok());
        assert_eq!(
            read_shutdown_command(b"shutdown\nextra".as_slice()),
            Err("invalid browser fixture shutdown command")
        );
        assert_eq!(
            read_shutdown_command(b"xxxxxxxxxxxxxxxxx".as_slice()),
            Err("invalid browser fixture shutdown command")
        );
    }

    #[tokio::test]
    async fn combined_graphql_test_state_contains_store_runtime_and_paths() {
        let store = crate::store::tests::test_store().await;
        let paths = store.noema_paths().expect("paths");
        let runtime = CodexRuntimeHandle::spawn_with_provider(
            Arc::new(BrowserFixtureProvider),
            store.clone(),
        )
        .await
        .expect("runtime");
        let state = crate::graphql::GraphqlState::for_tests_with_store_runtime_and_paths(
            store,
            runtime.clone(),
            paths,
        );

        assert!(state.store().is_ok());
        assert!(state.runtime().is_ok());
        assert!(state.paths().is_ok());

        runtime.shutdown().await;
    }
}
