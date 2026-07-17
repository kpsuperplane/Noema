use super::*;

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
        ConfigOverrides::default(),
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
    let codex = load_codex_config(None, ConfigOverrides::default(), None, &[])
        .expect("codex config should resolve");

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
}

#[test]
fn old_openai_api_key_env_is_ignored() {
    let error = load_resolved(
        None,
        ConfigOverrides::default(),
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
        ConfigOverrides::default(),
        None,
        &[],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::ParseConfig { .. }));
}

#[test]
fn missing_explicit_config_file_is_an_error() {
    let missing = PathBuf::from("/tmp/noema-missing-config.yaml");
    let error = Config::load(Some(missing.clone()), ConfigOverrides::default()).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::ConfigFileNotFound { path } if path == missing
    ));
}

#[test]
fn invalid_codex_timeout_env_is_an_error() {
    let error = load_resolved(
        None,
        ConfigOverrides::default(),
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
        ConfigOverrides::default(),
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
        ConfigOverrides::default(),
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
        ConfigOverrides::default(),
        None,
        &[],
    )
    .expect("generated config should parse");

    assert_eq!(codex.base_url, DEFAULT_CODEX_BASE_URL);
    assert_eq!(codex.default_model, None);
    assert_eq!(codex.timeout_seconds, DEFAULT_CODEX_TIMEOUT_SECONDS);
}
