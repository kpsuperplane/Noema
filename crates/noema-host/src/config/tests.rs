use super::*;
use figment::{Figment, providers::Serialized};
use noema_providers::{
    CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_CODEX_TIMEOUT_SECONDS,
    DEFAULT_OPENAI_MODEL, LocalModelBackend,
};
use serde_json::{Number, Value};
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn write_config(contents: &str) -> NamedTempFile {
    let file = NamedTempFile::new().expect("temp config");
    std::fs::write(file.path(), contents).expect("write config");
    file
}

fn load_resolved(
    path_override: Option<PathBuf>,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<HostConfig, ConfigError> {
    load_raw_config_from_sources(path_override, default_config_path, test_env(env))?.resolve()
}

fn load_codex_config(
    path_override: Option<PathBuf>,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<CodexProviderConfig, ConfigError> {
    load_raw_config_from_sources(path_override, default_config_path, test_env(env))?
        .resolve_codex_config()
}

fn load_config_from_yaml(yaml: &str) -> Result<HostConfig, ConfigError> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, yaml).expect("write config");
    Config::load(Some(path))
}

fn test_env(env: &[(&str, &str)]) -> Figment {
    env.iter().fold(Figment::new(), |figment, (key, value)| {
        let Some(path) = normalize_env_key(key) else {
            return figment;
        };

        if CONFIG_ENV_KEYS.contains(&path.as_str()) {
            figment.merge(Serialized::default(&path, parse_env_value(value)))
        } else {
            figment
        }
    })
}

fn normalize_env_key(key: &str) -> Option<String> {
    key.strip_prefix("NOEMA_")
        .map(|key| key.to_ascii_lowercase().replace("__", "."))
        .and_then(|key| normalize_config_env_key(&key))
}

fn parse_env_value(value: &str) -> Value {
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => value
            .parse::<u64>()
            .ok()
            .map(Number::from)
            .map_or_else(|| Value::String(value.to_string()), Value::Number),
    }
}

mod default_provider;

#[test]
fn configuration_source_and_provider_contracts() {
    // Case: reads_default_config_from_noema_home_env.
    let dir = tempfile::tempdir().expect("temp dir");
    let noema_home = dir.path().join("custom-noema");
    std::fs::create_dir_all(&noema_home).expect("create noema home");
    std::fs::write(
        noema_home.join("config.yaml"),
        r"
provider: openai
model: noema-home-model
reasoning_effort: medium
",
    )
    .expect("write config");

    let resolved = load_resolved(
        None,
        Some(noema_home.join("config.yaml")),
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("config should load");

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.default_model, "noema-home-model");

    // Case: codex_provider_does_not_require_openai_api_key.
    let resolved = load_resolved(None, None, &[("NOEMA_PROVIDER", "codex")])
        .expect("codex config should resolve without OpenAI API key");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

    let ProviderConfig::Codex(codex) = resolved.provider else {
        panic!("expected codex config");
    };

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(codex.default_model.as_deref(), Some(DEFAULT_OPENAI_MODEL));
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);

    // Case: local_models_config_resolves_without_cloud_credentials.
    let file = write_config(
        r"
provider: local_models
model: ternary-bonsai-8b
local_models:
  preferred_backend: metal
  context_window_tokens: 16384
  timeout_seconds: 900
  startup_timeout_seconds: 240
",
    );

    let resolved = load_resolved(Some(file.path().to_path_buf()), None, &[])
        .expect("local models config should resolve");

    let ProviderConfig::LocalModels(config) = resolved.provider else {
        panic!("expected local models config");
    };
    assert_eq!(config.default_model, "ternary-bonsai-8b");
    assert_eq!(config.model_path, None);
    assert_eq!(config.preferred_backend, Some(LocalModelBackend::Metal));
    assert_eq!(config.context_window_tokens, 16_384);
    assert_eq!(config.timeout_seconds, 900);
    assert_eq!(config.startup_timeout_seconds, 240);

    // Case: resolves_web_config_from_yaml_and_env.
    let file = write_config(
        r"
provider: codex
web:
  host: 127.0.0.2
  port: 4747
",
    );

    let legacy = load_resolved(Some(file.path().to_path_buf()), None, &[])
        .expect("partial web config should retain new defaults");
    assert_eq!(legacy.web.rp_id, "localhost");
    assert_eq!(legacy.web.public_origin, None);

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        None,
        &[
            ("NOEMA_WEB__HOST", "127.0.0.3"),
            ("NOEMA_WEB__PORT", "5757"),
            ("NOEMA_WEB__RP_ID", "noema.example"),
            ("NOEMA_WEB__PUBLIC_ORIGIN", "https://noema.example"),
            ("NOEMA_WEB__DEV_NO_AUTH", "true"),
            ("NOEMA_WEB__LOCAL_GRAPHQL_SOCKET", "true"),
            ("NOEMA_WEB__GRAPHIQL", "true"),
            ("NOEMA_MCP__STDIO_ENABLED", "true"),
        ],
    )
    .expect("config should load");

    assert_eq!(
        resolved.web,
        WebConfig {
            host: "127.0.0.3".to_string(),
            port: 5757,
            rp_id: "noema.example".to_string(),
            public_origin: Some("https://noema.example".to_string()),
            dev_no_auth: true,
            local_graphql_socket: true,
            graphiql: true,
        }
    );
    assert!(resolved.mcp.stdio_enabled);

    // Case: codex_config_reads_yaml_and_normalized_env_overrides.
    let file = write_config(
        r"
provider: codex
model: yaml-model
reasoning_effort: medium
codex:
  base_url: https://yaml.example/codex
  fast_mode: true
  model: yaml-codex-model
  tool_classification_model: yaml-codex-tool-classifier
  timeout_seconds: 120
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        None,
        &[
            ("NOEMA_MODEL", "env-model"),
            ("NOEMA_REASONING_EFFORT", "high"),
            (
                "NOEMA_CODEX__TOOL_CLASSIFICATION_MODEL",
                "env-codex-tool-classifier",
            ),
            ("NOEMA_CODEX__BASE_URL", "https://env.example/codex/"),
            ("NOEMA_CODEX__TIMEOUT_SECONDS", "123"),
        ],
    )
    .expect("codex config should resolve");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

    let ProviderConfig::Codex(codex) = resolved.provider else {
        panic!("expected codex config");
    };

    assert_eq!(codex.default_model.as_deref(), Some("env-model"));
    assert_eq!(
        codex.reasoning_effort,
        Some(noema_providers::ReasoningEffort::High)
    );
    assert_eq!(
        codex.tool_classification_model.as_deref(),
        Some("env-codex-tool-classifier")
    );
    assert_eq!(codex.base_url, "https://env.example/codex");
    assert_eq!(codex.timeout_seconds, 123);
    assert!(codex.fast_mode);
}

mod validation;
