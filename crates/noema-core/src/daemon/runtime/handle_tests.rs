use super::*;

#[test]
fn codex_config_for_provider_account_uses_account_home() {
    let account_home = std::path::PathBuf::from("/noema/providers/codex/default");
    let mut config = CodexProviderConfig::default();

    apply_provider_account_home(&mut config, &account_home);

    assert_eq!(config.account_home.as_deref(), Some(account_home.as_path()));
}
