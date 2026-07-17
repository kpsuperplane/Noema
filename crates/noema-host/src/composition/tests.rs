use super::*;
use noema_providers::ProviderOperations;
use std::{future::Future, pin::Pin, sync::Arc};

async fn test_store() -> NoemaStore {
    noema_store::test_support::open_ephemeral_store()
        .await
        .expect("open ephemeral store")
}

fn ready_provider_selection(
    mut selection: ProviderSelectionSnapshot,
) -> noema_providers::ProviderReadySelection {
    if selection.provider_instance_key.is_none()
        && selection.provider_kind != ProviderKind::LocalModels.as_str()
    {
        selection.provider_instance_key = Some(
            provider_account_instance_key(&selection.provider_account_id)
                .expect("hosted provider key"),
        );
    }
    let key = selection
        .provider_instance_key
        .clone()
        .expect("ready selection needs exact key");
    let registry = ProviderRegistry::new();
    registry
        .register(key, Arc::new(DefaultModelProvider("test-default")))
        .expect("register ready provider");
    registry
        .prove_ready_selection(selection)
        .expect("prove ready selection")
}

#[test]
fn startup_entrypoints_preserve_first_run_config_bytes() {
    let home = tempfile::tempdir().expect("temporary Noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("Noema paths");

    initialize_home(&paths).expect("initialize empty home");
    assert_eq!(
        std::fs::read(paths.config_path()).expect("read generated config"),
        DEFAULT_NOEMA_CONFIG_YAML.as_bytes()
    );

    let existing = b"provider: foundation_local\n";
    std::fs::write(paths.config_path(), existing).expect("replace config");
    initialize_home(&paths).expect("reinitialize existing home");
    assert_eq!(
        std::fs::read(paths.config_path()).expect("read preserved config"),
        existing
    );
}

#[test]
fn process_env_entrypoint_initializes_and_loads_first_run_home() {
    run_startup_entrypoint_child("process_env");
}

#[test]
fn loaded_config_entrypoint_preserves_existing_config_bytes() {
    run_startup_entrypoint_child("loaded_config");
}

fn run_startup_entrypoint_child(mode: &str) {
    let home = tempfile::tempdir().expect("temporary child Noema home");
    let output = std::process::Command::new(std::env::current_exe().expect("current test binary"))
        .arg("--exact")
        .arg("composition::tests::startup_entrypoint_child")
        .arg("--nocapture")
        .env("NOEMA_HOST_STARTUP_TEST_MODE", mode)
        .env(noema_home::NOEMA_HOME_ENV, home.path())
        .output()
        .expect("run isolated startup test");
    assert!(
        output.status.success(),
        "isolated {mode} startup failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn startup_entrypoint_child() {
    let Ok(mode) = std::env::var("NOEMA_HOST_STARTUP_TEST_MODE") else {
        return;
    };
    let paths = NoemaPaths::from_process_env().expect("child Noema paths");

    match mode.as_str() {
        "process_env" => {
            let host = crate::start_from_process_env()
                .await
                .expect("start host from process environment");
            assert_eq!(host.web_config(), &crate::WebConfig::default());
            assert_eq!(
                std::fs::read(paths.config_path()).expect("read generated config"),
                DEFAULT_NOEMA_CONFIG_YAML.as_bytes()
            );
            host.shutdown().await;
        }
        "loaded_config" => {
            std::fs::create_dir_all(paths.root()).expect("create existing home");
            let existing = b"provider: deliberately-not-loaded\n";
            std::fs::write(paths.config_path(), existing).expect("write existing config");
            let web = crate::WebConfig {
                host: "127.0.0.1".to_string(),
                port: 4_848,
            };
            let config = HostConfig::new(
                ProviderConfig::Codex(CodexProviderConfig::default()),
                web.clone(),
            );
            let host = crate::start_from_loaded_config(config)
                .await
                .expect("start host from loaded config");
            assert_eq!(host.web_config(), &web);
            assert_eq!(
                std::fs::read(paths.config_path()).expect("read preserved config"),
                existing
            );
            host.shutdown().await;
        }
        other => panic!("unexpected startup test mode: {other}"),
    }
}

#[tokio::test]
async fn fresh_unresolvable_local_default_returns_typed_initialization_error() {
    let home = tempfile::tempdir().expect("temporary Noema home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("Noema paths");
    initialize_home(&paths).expect("initialize home");
    let config = HostConfig::new(
        ProviderConfig::LocalModels(noema_providers::LocalModelsProviderConfig {
            default_model: "missing-local-model".to_string(),
            model_path: None,
            preferred_backend: None,
            runtime_root: None,
            context_window_tokens: noema_providers::DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
            timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
            startup_timeout_seconds: noema_providers::DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
            system_errors: None,
        }),
        crate::WebConfig::default(),
    );

    let error = match assemble(config, paths).await {
        Ok(host) => {
            host.shutdown().await;
            panic!("missing configured local model unexpectedly started")
        }
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RuntimeHostError::ConfiguredDefault(
            noema_store::StoreError::ConfiguredDefaultUnresolvable { .. }
        )
    ));
}

#[test]
fn runtime_host_error_messages_are_plain_language() {
    assert_eq!(
        RuntimeHostError::Home(noema_home::NoemaHomeError::CreateDirectory {
            path: std::path::PathBuf::from("/unavailable"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        })
        .user_message(),
        "Noema could not open its data folder."
    );
    assert_eq!(
        RuntimeHostError::Store(noema_store::StoreError::Schema("sqlite failed".to_string()))
            .user_message(),
        "Noema could not start its local memory store."
    );
    assert_eq!(
        RuntimeHostError::Composition("provider failed".to_string()).user_message(),
        "Noema could not start the local assistant service."
    );
}

#[tokio::test]
async fn memory_proxy_config_prefers_memory_model_provider() {
    let providers = std::collections::HashMap::from([
        (
            "codex".to_string(),
            Arc::new(DefaultModelProvider("codex-default")) as noema_providers::ProviderHandle,
        ),
        (
            "foundation_local".to_string(),
            Arc::new(DefaultModelProvider("foundation-default")) as noema_providers::ProviderHandle,
        ),
    ]);
    let settings = MemoryServiceSettingsRecord {
        settings_id: "default".to_string(),
        mode: MemoryServiceMode::Managed,
        base_url: None,
        port: None,
        provider_account_id: Some("provider_account:foundation_local:default".to_string()),
        provider_kind: Some("foundation_local".to_string()),
        provider_instance_key: None,
        model_profile: Some(noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE.to_string()),
        reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
    };
    let store = test_store().await;
    store
        .ensure_default_foundation_local_provider_account()
        .await
        .expect("foundation account");
    store
        .update_provider_account_status(
            "provider_account:foundation_local:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate foundation account");
    let ready_selection = ready_provider_selection(ProviderSelectionSnapshot::explicit(
        "foundation_local",
        "provider_account:foundation_local:default",
        noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE,
        Some(noema_providers::ReasoningEffort::Low),
        Some("memory_proxy_test".to_string()),
    ));
    store
        .save_memory_service_settings_with_ready_selection(
            SaveMemoryServiceSettings {
                mode: settings.mode,
                base_url: settings.base_url.clone(),
                port: settings.port,
                provider_account_id: settings.provider_account_id.clone(),
                provider_kind: settings.provider_kind.clone(),
                model_profile: settings.model_profile.clone(),
                reasoning_effort: settings.reasoning_effort,
            },
            &ready_selection,
        )
        .await
        .expect("memory settings");
    let registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
    register_hosted_providers(&registry, &providers).expect("register providers");

    let config = memory_model_proxy_config_from_settings(
        registry,
        Arc::new(store.clone()),
        "secret".to_string(),
        test_system_error_logger(),
    )
    .await
    .expect("proxy config");
    let route = config
        .route_resolver
        .resolve_route()
        .await
        .expect("memory route");

    assert_eq!(
        config.model_profile,
        noema_providers::DEFAULT_FOUNDATION_LOCAL_PROFILE
    );
    assert_eq!(
        route.selection().reasoning_effort,
        Some(noema_providers::ReasoningEffort::Low)
    );
    assert_eq!(
        route
            .operations()
            .default_tool_classification_model()
            .as_deref(),
        Some("foundation-default")
    );

    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticate codex account");
    let ready_selection = ready_provider_selection(ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "codex-memory",
        None,
        Some("memory_proxy_test".to_string()),
    ));
    store
        .save_memory_service_settings_with_ready_selection(
            SaveMemoryServiceSettings {
                mode: MemoryServiceMode::Managed,
                base_url: None,
                port: None,
                provider_account_id: Some("provider_account:codex:default".to_string()),
                provider_kind: Some("codex".to_string()),
                model_profile: Some("codex-memory".to_string()),
                reasoning_effort: None,
            },
            &ready_selection,
        )
        .await
        .expect("updated memory settings");
    let refreshed = config
        .route_resolver
        .resolve_route()
        .await
        .expect("refreshed memory route");
    assert_eq!(refreshed.selection().provider_kind, "codex");
    assert_eq!(
        refreshed.selection().model_profile.as_deref(),
        Some("codex-memory")
    );
    assert_eq!(
        refreshed
            .operations()
            .default_tool_classification_model()
            .as_deref(),
        Some("codex-default")
    );
}

#[tokio::test]
async fn memory_proxy_config_uses_default_seeded_during_initialization() {
    let providers = std::collections::HashMap::from([(
        "codex".to_string(),
        Arc::new(DefaultModelProvider("codex-default")) as noema_providers::ProviderHandle,
    )]);
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let mut configured_default = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "codex-default",
        None,
        Some("test_configured_default".to_string()),
    );
    configured_default.provider_instance_key = Some(
        provider_account_instance_key("provider_account:codex:default").expect("provider key"),
    );
    let registry: ProviderRegistryHandle = Arc::new(ProviderRegistry::new());
    register_hosted_providers(&registry, &providers).expect("register providers");
    let ready_selection = registry
        .prove_ready_selection(configured_default.clone())
        .expect("ready configured selection");
    store
        .initialize_missing_provider_selections(&configured_default, Some(&ready_selection))
        .await
        .expect("initialize selections");

    let config = memory_model_proxy_config_from_settings(
        registry,
        Arc::new(store),
        "secret".to_string(),
        test_system_error_logger(),
    )
    .await
    .expect("proxy config");
    let route = config
        .route_resolver
        .resolve_route()
        .await
        .expect("memory route");

    assert_eq!(config.model_profile, "codex-default");
    assert_eq!(
        route
            .operations()
            .default_tool_classification_model()
            .as_deref(),
        Some("codex-default")
    );
}

#[tokio::test]
async fn memory_proxy_config_does_not_require_provider_readiness_at_startup() {
    let store = test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("codex account");
    let configured_default = ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "codex-default",
        None,
        Some("test_configured_default".to_string()),
    );
    let ready_selection = ready_provider_selection(configured_default.clone());
    store
        .initialize_missing_provider_selections(ready_selection.selection(), Some(&ready_selection))
        .await
        .expect("initialize selections");

    let config = memory_model_proxy_config_from_settings(
        Arc::new(ProviderRegistry::new()),
        Arc::new(store),
        "secret".to_string(),
        test_system_error_logger(),
    )
    .await
    .expect("proxy config");

    assert_eq!(config.model_profile, "codex-default");
    assert!(matches!(
        config.route_resolver.resolve_route().await,
        Err(noema_providers::ProviderRouteError::InstanceMissing { .. })
    ));
}

#[derive(Debug)]
struct DefaultModelProvider(&'static str);

impl ProviderOperations for DefaultModelProvider {
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(self.0.to_string())
    }

    fn generate_streaming<'a>(
        &'a self,
        _request: noema_providers::GenerateRequest,
        _on_event: &'a mut (dyn FnMut(noema_providers::GenerateStreamEvent) + Send),
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        noema_providers::GenerateResponse,
                        noema_providers::ProviderError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Ok(noema_providers::GenerateResponse::final_text(
                "ok", "test", "model",
            ))
        })
    }
}

fn test_system_error_logger() -> SystemErrorLogger {
    SystemErrorLogger::new(std::env::temp_dir().join("noema-runtime-host-test-errors.jsonl"))
}
