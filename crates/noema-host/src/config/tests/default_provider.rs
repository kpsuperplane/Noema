use super::*;

#[test]
fn generated_default_config_omits_codex_model() {
    assert!(!crate::config::DEFAULT_NOEMA_CONFIG_YAML.contains("model: gpt-5.5"));
    assert!(!crate::config::DEFAULT_NOEMA_CONFIG_YAML.contains("codex:\n  model:"));
}

#[test]
fn codex_explicit_model_requires_reasoning_effort() {
    let yaml = r#"
provider: codex
codex:
  model: gpt-5.5
"#;
    let error = load_config_from_yaml(yaml, ConfigOverrides::default())
        .expect_err("explicit reasoning-capable model without effort should fail");
    assert!(error.to_string().contains("reasoning_effort"));
}

#[test]
fn reasoning_effort_without_explicit_model_is_rejected() {
    let yaml = r#"
provider: codex
codex:
  reasoning_effort: medium
"#;
    let error = load_config_from_yaml(yaml, ConfigOverrides::default())
        .expect_err("effort without model should fail");
    assert!(error.to_string().contains("reasoning_effort"));
}

#[test]
fn codex_explicit_model_with_reasoning_effort_resolves() {
    let yaml = r#"
provider: codex
codex:
  model: gpt-5.5
  reasoning_effort: medium
"#;
    let resolved = load_config_from_yaml(yaml, ConfigOverrides::default()).expect("config");
    let ProviderConfig::Codex(config) = resolved.provider else {
        panic!("expected codex provider");
    };
    assert_eq!(config.default_model.as_deref(), Some("gpt-5.5"));
    assert_eq!(
        config.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Medium)
    );
}

#[test]
fn default_openai_config_requires_normalized_api_key() {
    let error = load_resolved(None, ConfigOverrides::default(), None, &[]).unwrap_err();

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
reasoning_effort: medium
openai:
  base_url: https://yaml.example/v1
  organization_id: yaml-org
  project_id: yaml-project
  timeout_seconds: 22
",
    );

    let resolved = load_resolved(
        Some(file.path().to_path_buf()),
        ConfigOverrides::new(
            None,
            Some("override-model".to_string()),
            Some("https://override.example/v1".to_string()),
        ),
        None,
        &[
            (OPENAI_API_KEY_ENV, "env-key"),
            ("NOEMA_OPENAI__BASE_URL", "https://env.example/v1"),
            ("NOEMA_OPENAI__ORGANIZATION_ID", "env-org"),
            ("NOEMA_OPENAI__PROJECT_ID", "env-project"),
            ("NOEMA_OPENAI__TIMEOUT_SECONDS", "33"),
            ("NOEMA_REASONING_EFFORT", "high"),
        ],
    )
    .expect("config should resolve");

    assert_eq!(resolved.provider.kind(), ProviderKind::OpenAi);

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.api_key, "env-key");
    assert_eq!(openai.default_model, "override-model");
    assert_eq!(
        openai.reasoning_effort,
        Some(noema_providers::ReasoningEffort::High)
    );
    assert_eq!(openai.base_url, "https://override.example/v1");
    assert_eq!(openai.organization_id.as_deref(), Some("env-org"));
    assert_eq!(openai.project_id.as_deref(), Some("env-project"));
    assert_eq!(openai.timeout_seconds, 33);
}

#[test]
fn resolved_config_debug_redacts_openai_env_credential() {
    const SENTINEL: &str = "noema-debug-secret-sentinel";
    let resolved = load_resolved(
        None,
        ConfigOverrides::default(),
        None,
        &[(OPENAI_API_KEY_ENV, SENTINEL)],
    )
    .expect("OpenAI config should resolve");

    let debug = format!("{resolved:?}");
    assert!(!debug.contains(SENTINEL));
    assert!(debug.contains("[REDACTED]"));
}
