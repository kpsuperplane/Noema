use super::*;

#[test]
fn configuration_validation_contracts() {
    // Case: top_level_tool_classification_model_overrides_provider_specific_model.
    let file = write_config(
        r"
provider: codex
tool_classification_model: yaml-tool-classifier
codex:
  tool_classification_model: yaml-codex-tool-classifier
",
    );

    let codex = load_codex_config(Some(file.path().to_path_buf()), None, &[])
        .expect("codex config should resolve");

    assert_eq!(
        codex.tool_classification_model.as_deref(),
        Some("yaml-tool-classifier")
    );

    // Case: old_openai_api_key_env_is_ignored.
    let error = load_resolved(None, None, &[("OPENAI_API_KEY", "old-key")]).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::MissingCredential { credential, .. } if credential == OPENAI_API_KEY_ENV
    ));

    // Case: yaml_openai_api_key_is_rejected.
    let file = write_config(
        r"
provider: openai
openai:
  api_key: not-allowed
",
    );

    let error = load_resolved(Some(file.path().to_path_buf()), None, &[]).unwrap_err();

    assert!(matches!(error, ConfigError::InvalidConfig { .. }));

    // Case: missing_explicit_config_file_is_an_error.
    let missing = PathBuf::from("/tmp/noema-missing-config.yaml");
    let error = Config::load(Some(missing.clone())).unwrap_err();

    assert!(matches!(
        error,
        ConfigError::ConfigFileNotFound { path } if path == missing
    ));

    // Case: zero_codex_timeout_is_an_error.
    let error = load_resolved(
        None,
        None,
        &[
            ("NOEMA_PROVIDER", "codex"),
            ("NOEMA_CODEX__TIMEOUT_SECONDS", "0"),
        ],
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::InvalidInteger { .. }));

    // Case: browser_limits_accept_configured_values_and_reject_out_of_range_values.
    let resolved = load_resolved(
        None,
        None,
        &[
            ("NOEMA_PROVIDER", "codex"),
            ("NOEMA_BROWSER__MAX_SESSIONS", "4"),
            ("NOEMA_BROWSER__MAX_OLD_SPACE_MB", "2048"),
        ],
    )
    .expect("browser limits should resolve");
    assert_eq!(
        resolved.browser,
        BrowserConfig {
            max_sessions: 4,
            max_old_space_mb: 2_048,
        }
    );
    for (name, value) in [
        ("NOEMA_BROWSER__MAX_SESSIONS", "9"),
        ("NOEMA_BROWSER__MAX_OLD_SPACE_MB", "255"),
    ] {
        let error =
            load_resolved(None, None, &[("NOEMA_PROVIDER", "codex"), (name, value)]).unwrap_err();
        assert!(matches!(
            error,
            ConfigError::InvalidInteger { name: invalid, .. } if invalid == name
        ));
    }

    // Case: unsupported_provider_is_an_error.
    let error = load_resolved(None, None, &[("NOEMA_PROVIDER", "unknown")]).unwrap_err();

    assert!(matches!(error, ConfigError::UnsupportedProvider { .. }));
}
