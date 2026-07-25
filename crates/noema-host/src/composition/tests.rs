use super::*;

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
            let store = NoemaStore::open(&StoreConfig::new(paths.sqlite_db_path()))
                .await
                .expect("open initialized store");
            let selection = store
                .default_provider_selection()
                .await
                .expect("read initialized default");
            assert_eq!(selection.model_profile.as_deref(), Some("gpt-5.6-luna"));
            assert_eq!(
                selection.reasoning_effort,
                Some(noema_providers::ReasoningEffort::Medium)
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
                rp_id: "localhost".to_string(),
                public_origin: None,
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
    let cases = [
        (
            RuntimeHostError::Store(noema_store::StoreError::Schema("failed".to_string())),
            "Noema could not start its local memory store.",
        ),
        (
            RuntimeHostError::Composition("failed".to_string()),
            "Noema could not start the local assistant service.",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.user_message(), message);
    }
}
