use super::*;
use serde_json::{Number, Value};
use tempfile::NamedTempFile;

fn write_config(contents: &str) -> NamedTempFile {
    let file = NamedTempFile::new().expect("temp config");
    std::fs::write(file.path(), contents).expect("write config");
    file
}

fn load_resolved(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<ResolvedConfig, ConfigError> {
    load_raw_config_from_sources(path_override, cli, default_config_path, test_env(env))?.resolve()
}

fn load_codex_config(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<CodexProviderConfig, ConfigError> {
    load_raw_config_from_sources(path_override, cli, default_config_path, test_env(env))?
        .resolve_codex_config()
}

fn load_daemon_config(
    path_override: Option<PathBuf>,
    cli: CliOverrides,
    default_config_path: Option<PathBuf>,
    env: &[(&str, &str)],
) -> Result<DaemonResolvedConfig, ConfigError> {
    load_raw_config_from_sources(path_override, cli, default_config_path, test_env(env))?
        .resolve_daemon_config()
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

#[test]
fn default_openai_config_requires_normalized_api_key() {
    let error = load_resolved(None, CliOverrides::default(), None, &[]).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::MissingCredential { provider, credential }
            if provider == "openai" && credential == OPENAI_API_KEY_ENV
    ));
}

#[test]
fn resolves_openai_from_figment_layers_in_precedence_order() {
    let file = write_config(
        r"
provider: openai
model: yaml-model
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 22
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        CliOverrides::new(
            None,
            Some("cli-model".to_string()),
            Some("https://cli.example/v1".to_string()),
        ),
        None,
        &[
            (OPENAI_API_KEY_ENV, "env-key"),
            ("NOEMA_OPENAI__BASE_URL", "https://env.example/v1"),
            ("NOEMA_OPENAI__ORGANIZATION_ID", "env-org"),
            ("NOEMA_OPENAI__PROJECT_ID", "env-project"),
            ("NOEMA_OPENAI__TIMEOUT_SECONDS", "33"),
        ],
    )
    .expect("config should resolve");

    assert_eq!(resolved.provider.kind(), ProviderKind::OpenAi);

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.api_key, "env-key");
    assert_eq!(openai.default_model, "cli-model");
    assert_eq!(openai.base_url, "https://cli.example/v1");
    assert_eq!(openai.organization_id.as_deref(), Some("env-org"));
    assert_eq!(openai.project_id.as_deref(), Some("env-project"));
    assert_eq!(openai.timeout_seconds, 33);
}

#[test]
fn reads_yaml_config() {
    let file = write_config(
        r"
provider: openai
model: yaml-model
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
        CliOverrides::default(),
        None,
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("config should load");

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.default_model, "yaml-model");
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
",
    )
    .expect("write config");

    let resolved = load_resolved(
        None,
        CliOverrides::default(),
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
",
    )
    .expect("write config");

    let resolved = load_resolved(
        None,
        CliOverrides::default(),
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
        CliOverrides::default(),
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
        CliOverrides::new(Some("codex".to_string()), None, None),
        None,
        &[],
    )
    .expect("codex config should resolve without OpenAI API key");

    assert_eq!(resolved.provider.kind(), ProviderKind::Codex);

    let ProviderConfig::Codex(codex) = resolved.provider else {
        panic!("expected codex config");
    };

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(codex.default_model.as_deref(), Some(DEFAULT_CODEX_MODEL));
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);
    assert_eq!(codex.account_home, None);
}

#[test]
fn foundation_local_config_does_not_require_openai_or_codex_credentials() {
    let resolved = load_resolved(
        None,
        CliOverrides::new(Some("foundation_local".to_string()), None, None),
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
        CliOverrides::default(),
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
fn provider_config_does_not_require_database_url() {
    let resolved = load_resolved(
        None,
        CliOverrides::new(Some("codex".to_string()), None, None),
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
        CliOverrides::new(Some("codex".to_string()), None, None),
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
        CliOverrides::default(),
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
        CliOverrides::new(Some("codex".to_string()), None, None),
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
        CliOverrides::default(),
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
codex:
  base_url: https://yaml.example/codex
  model: yaml-codex-model
  tool_classification_model: yaml-codex-tool-classifier
  timeout_seconds: 120
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        CliOverrides::default(),
        None,
        &[
            ("NOEMA_MODEL", "env-model"),
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
        codex.tool_classification_model.as_deref(),
        Some("env-codex-tool-classifier")
    );
    assert_eq!(codex.base_url, "https://env.example/codex");
    assert_eq!(codex.timeout_seconds, 123);
    assert_eq!(codex.account_home, None);
}

#[test]
fn top_level_tool_classification_model_overrides_provider_specific_model() {
    let file = write_config(
        r"
provider: codex
tool_classification_model: yaml-tool-classifier
codex:
  tool_classification_model: yaml-codex-tool-classifier
",
    );

    let codex = load_codex_config(
        Some(file.path().to_path_buf()),
        CliOverrides::default(),
        None,
        &[],
    )
    .expect("codex config should resolve");

    assert_eq!(
        codex.tool_classification_model.as_deref(),
        Some("yaml-tool-classifier")
    );
}

#[test]
fn load_codex_ignores_default_openai_provider_credentials() {
    let codex = load_codex_config(None, CliOverrides::default(), None, &[])
        .expect("codex config should resolve");

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
}

#[test]
fn old_openai_api_key_env_is_ignored() {
    let error = load_resolved(
        None,
        CliOverrides::default(),
        None,
        &[("OPENAI_API_KEY", "old-key")],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ConfigError::MissingCredential { credential, .. } if credential == OPENAI_API_KEY_ENV
    ));
}

#[test]
fn yaml_openai_api_key_is_rejected() {
    let file = write_config(
        r"
provider: openai
openai:
  api_key: not-allowed
",
    );

    let error = load_resolved(
        Some(file.path().to_path_buf()),
        CliOverrides::default(),
        None,
        &[],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::ParseConfig { .. }));
}

#[test]
fn missing_explicit_config_file_is_an_error() {
    let missing = PathBuf::from("/tmp/noema-missing-config.yaml");
    let error = Config::load(Some(missing.clone()), CliOverrides::default()).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::ConfigFileNotFound { path } if path == missing
    ));
}

#[test]
fn invalid_codex_timeout_env_is_an_error() {
    let error = load_resolved(
        None,
        CliOverrides::default(),
        None,
        &[
            ("NOEMA_PROVIDER", "codex"),
            ("NOEMA_CODEX__TIMEOUT_SECONDS", "abc"),
        ],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::Load(_)));
}

#[test]
fn zero_codex_timeout_is_an_error() {
    let error = load_resolved(
        None,
        CliOverrides::default(),
        None,
        &[
            ("NOEMA_PROVIDER", "codex"),
            ("NOEMA_CODEX__TIMEOUT_SECONDS", "0"),
        ],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::InvalidInteger { .. }));
}

#[test]
fn unsupported_provider_is_an_error() {
    let error = load_resolved(
        None,
        CliOverrides::default(),
        None,
        &[("NOEMA_PROVIDER", "unknown")],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::UnsupportedProvider { .. }));
}

#[test]
fn generated_config_template_parses() {
    let file = write_config(crate::DEFAULT_NOEMA_CONFIG_YAML);

    let codex = load_codex_config(
        Some(file.path().to_path_buf()),
        CliOverrides::default(),
        None,
        &[],
    )
    .expect("generated config should parse");

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(codex.default_model.as_deref(), Some(DEFAULT_CODEX_MODEL));
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);
}
