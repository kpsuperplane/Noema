use super::*;
use figment::{Figment, providers::Serialized};
use noema_home::NOEMA_HOME_ENV;
use noema_providers::{
    CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_CODEX_TIMEOUT_SECONDS, LocalModelBackend,
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
    overrides: ConfigOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<HostConfig, ConfigError> {
    load_raw_config_from_sources(path_override, overrides, default_config_path, test_env(env))?
        .resolve()
}

fn load_codex_config(
    path_override: Option<PathBuf>,
    overrides: ConfigOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<CodexProviderConfig, ConfigError> {
    load_raw_config_from_sources(path_override, overrides, default_config_path, test_env(env))?
        .resolve_codex_config()
}

fn load_daemon_config(
    path_override: Option<PathBuf>,
    overrides: ConfigOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<HostConfig, ConfigError> {
    load_raw_config_from_sources(path_override, overrides, default_config_path, test_env(env))?
        .resolve()
}

fn load_config_from_yaml(
    yaml: &str,
    overrides: ConfigOverrides,
) -> Result<HostConfig, ConfigError> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, yaml).expect("write config");
    Config::load(Some(path), overrides)
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
fn reads_yaml_config() {
    let file = write_config(
        r"
provider: openai
model: yaml-model
reasoning_effort: medium
tool_classification_model: yaml-tool-classifier
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 44
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("config should load");

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.default_model, "yaml-model");
    assert_eq!(
        openai.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Medium)
    );
    assert_eq!(
        openai.tool_classification_model.as_deref(),
        Some("yaml-tool-classifier")
    );
    assert_eq!(openai.base_url, "https://yaml.example/v1");
    assert_eq!(openai.organization_id.as_deref(), Some("yaml-org"));
    assert_eq!(openai.project_id.as_deref(), Some("yaml-project"));
    assert_eq!(openai.timeout_seconds, 44);
}

#[test]
fn reads_default_config_from_noema_home_env() {
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
        ConfigOverrides::default(),
        Some(noema_home.join("config.yaml")),
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("config should load");

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.default_model, "noema-home-model");
}

#[test]
fn reads_default_config_from_home_dot_noema_without_noema_home() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let noema_home = home.join(".noema");
    std::fs::create_dir_all(&noema_home).expect("create noema home");
    std::fs::write(
        noema_home.join("config.yaml"),
        r"
provider: openai
model: home-model
reasoning_effort: medium
",
    )
    .expect("write config");

    let resolved = load_resolved(
        None,
        ConfigOverrides::default(),
        Some(home.join(".noema/config.yaml")),
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("config should load");

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.default_model, "home-model");
}

#[test]
fn noema_home_env_controls_path_but_is_not_config() {
    let dir = tempfile::tempdir().expect("temp dir");
    let noema_home = dir.path().join("noema");
    std::fs::create_dir_all(&noema_home).expect("create noema home");
    std::fs::write(
        noema_home.join("config.yaml"),
        r"
provider: codex
",
    )
    .expect("write config");

    let resolved = load_resolved(
        None,
        ConfigOverrides::default(),
        Some(noema_home.join("config.yaml")),
        &[(NOEMA_HOME_ENV, "/ignored/as/config")],
    )
    .expect("config should load");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);
}

#[test]
fn codex_provider_does_not_require_openai_api_key() {
    let resolved = load_resolved(
        None,
        ConfigOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("codex config should resolve without OpenAI API key");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

    let ProviderConfig::Codex(codex) = resolved.provider else {
        panic!("expected codex config");
    };

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(codex.default_model, None);
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);
    assert_eq!(codex.account_home, None);
}

#[test]
fn foundation_local_config_does_not_require_openai_or_codex_credentials() {
    let resolved = load_resolved(
        None,
        ConfigOverrides::new(Some("foundation_local".to_string()), None, None),
        None,
        &[],
    )
    .expect("foundation local config should resolve without credentials");

    assert_eq!(resolved.provider.kind(), ProviderKind::FoundationLocal);

    let ProviderConfig::FoundationLocal(config) = resolved.provider else {
        panic!("expected foundation local config");
    };

    assert_eq!(config.default_profile, "default");
    assert_eq!(config.bridge_path, None);
}

#[test]
fn foundation_local_config_reads_yaml_and_env_overrides() {
    let file = write_config(
        r"
provider: foundation_local
foundation_local:
  default_profile: compact
  bridge_path: /tmp/noema-foundation-bridge
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[
            ("NOEMA_FOUNDATION_LOCAL__DEFAULT_PROFILE", "default"),
            (
                "NOEMA_FOUNDATION_LOCAL__BRIDGE_PATH",
                "/tmp/env-noema-foundation-bridge",
            ),
        ],
    )
    .expect("foundation local config should resolve");

    let ProviderConfig::FoundationLocal(config) = resolved.provider else {
        panic!("expected foundation local config");
    };

    assert_eq!(config.default_profile, "default");
    assert_eq!(
        config.bridge_path.as_deref(),
        Some(std::path::Path::new("/tmp/env-noema-foundation-bridge"))
    );
}

#[test]
fn local_models_config_resolves_without_cloud_credentials() {
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

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[],
    )
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
}

#[test]
fn local_models_config_rejects_unknown_backend() {
    let file = write_config(
        r"
provider: local_models
local_models:
  preferred_backend: neural_engine
",
    );

    let error = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[],
    )
    .expect_err("unknown local backend should fail");

    assert!(error.to_string().contains("preferred_backend"));
}

#[test]
fn provider_config_does_not_require_database_url() {
    let resolved = load_resolved(
        None,
        ConfigOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("provider config should resolve without database storage");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);
}

#[test]
fn resolves_default_web_config() {
    let resolved = load_resolved(
        None,
        ConfigOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("config should load");

    assert_eq!(
        resolved.web,
        WebConfig {
            host: DEFAULT_WEB_HOST.to_string(),
            port: DEFAULT_WEB_PORT,
        }
    );
}

#[test]
fn resolves_web_config_from_yaml_and_env() {
    let file = write_config(
        r"
provider: codex
web:
  host: 127.0.0.2
  port: 4747
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[
            ("NOEMA_WEB__HOST", "127.0.0.3"),
            ("NOEMA_WEB__PORT", "5757"),
        ],
    )
    .expect("config should load");

    assert_eq!(
        resolved.web,
        WebConfig {
            host: "127.0.0.3".to_string(),
            port: 5757,
        }
    );
}

#[test]
fn daemon_config_resolves_codex_and_web_without_openai_credentials() {
    let config = load_daemon_config(
        None,
        ConfigOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("daemon config");

    let ProviderConfig::Codex(codex) = config.provider else {
        panic!("expected codex provider config");
    };
    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(config.web.port, DEFAULT_WEB_PORT);
}

#[test]
fn daemon_config_does_not_require_database_url() {
    let file = write_config(
        r"
provider: codex
",
    );

    let config = load_daemon_config(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
        None,
        &[("NOEMA_LEGACY_DATABASE_URL", "ignored")],
    )
    .expect("daemon config should use embedded store");

    let ProviderConfig::Codex(codex) = config.provider else {
        panic!("expected codex provider config");
    };
    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(config.web, WebConfig::default());
}

#[test]
fn production_env_normalization_ignores_unknown_flat_keys() {
    assert_eq!(normalize_env_key("NOEMA_LEGACY_DATABASE_URL"), None);
    assert_eq!(
        normalize_config_env_key("LEGACY_DATABASE_URL").as_deref(),
        None
    );
}

#[test]
fn codex_config_reads_yaml_and_normalized_env_overrides() {
    let file = write_config(
        r"
provider: codex
model: yaml-model
reasoning_effort: medium
codex:
  base_url: https://yaml.example/codex
  model: yaml-codex-model
  tool_classification_model: yaml-codex-tool-classifier
  timeout_seconds: 120
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::default(),
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
    assert_eq!(codex.account_home, None);
}

mod validation;
