use super::*;

#[test]
fn provider_resolution_precedence_and_secrecy_contracts() {
    let openai_file = write_config("provider: openai\n");
    let resolved = load_resolved(
        Some(openai_file.path().to_path_buf()),
        None,
        &[(OPENAI_API_KEY_ENV, "env-key")],
    )
    .expect("OpenAI defaults should resolve");
    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected OpenAI config");
    };
    assert_eq!(openai.default_model, "gpt-5.6-terra");
    assert_eq!(
        openai.reasoning_effort,
        Some(noema_providers::ReasoningEffort::Medium)
    );

    // Case: codex_model_and_reasoning_effort_must_be_configured_together.
    for yaml in [
        r#"
provider: codex
codex:
  model: gpt-5.5
"#,
        r#"
provider: codex
codex:
  reasoning_effort: medium
"#,
    ] {
        let error = load_config_from_yaml(yaml).expect_err("partial model selection should fail");
        assert!(error.to_string().contains("reasoning_effort"));
    }

    // Case: resolves_openai_from_figment_layers_in_precedence_order.
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
        None,
        &[
            (OPENAI_API_KEY_ENV, "env-key"),
            ("NOEMA_OPENAI__BASE_URL", "https://env.example/v1"),
            ("NOEMA_OPENAI__ORGANIZATION_ID", "env-org"),
            ("NOEMA_OPENAI__PROJECT_ID", "env-project"),
            ("NOEMA_OPENAI__TIMEOUT_SECONDS", "33"),
            ("NOEMA_MODEL", "env-model"),
            ("NOEMA_REASONING_EFFORT", "high"),
        ],
    )
    .expect("config should resolve");

    assert_eq!(resolved.provider.kind(), ProviderKind::OpenAi);

    let ProviderConfig::OpenAi(openai) = resolved.provider else {
        panic!("expected openai config");
    };

    assert_eq!(openai.api_key, "env-key");
    assert_eq!(openai.default_model, "env-model");
    assert_eq!(
        openai.reasoning_effort,
        Some(noema_providers::ReasoningEffort::High)
    );
    assert_eq!(openai.base_url, "https://env.example/v1");
    assert_eq!(openai.organization_id.as_deref(), Some("env-org"));
    assert_eq!(openai.project_id.as_deref(), Some("env-project"));
    assert_eq!(openai.timeout_seconds, 33);

    // Case: resolved_config_debug_redacts_openai_env_credential.
    const SENTINEL: &str = "noema-debug-secret-sentinel";
    let resolved = load_resolved(None, None, &[(OPENAI_API_KEY_ENV, SENTINEL)])
        .expect("OpenAI config should resolve");

    let debug = format!("{resolved:?}");
    assert!(!debug.contains(SENTINEL));
    assert!(debug.contains("[REDACTED]"));
}
