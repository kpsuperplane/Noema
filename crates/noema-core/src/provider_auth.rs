//! Provider authentication support.

use std::{fs, io, path::Path};

/// Prepare a Codex account home for portable file-backed credentials.
///
/// # Errors
///
/// Returns an error when the account directory or config file cannot be written.
pub fn ensure_codex_account_home(account_home: &Path) -> io::Result<()> {
    create_private_account_dir_all(account_home)?;
    let config_path = account_home.join("config.toml");
    if !config_path.exists() {
        fs::write(config_path, "cli_auth_credentials_store = \"file\"\n")?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_account_dir_all(account_home: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut missing_dirs = Vec::new();
    let mut current = Some(account_home);
    while let Some(path) = current {
        if path.exists() {
            break;
        }
        missing_dirs.push(path.to_path_buf());
        current = path.parent();
    }

    fs::create_dir_all(account_home)?;
    for path in missing_dirs.iter().rev() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(account_home, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_account_dir_all(account_home: &Path) -> io::Result<()> {
    fs::create_dir_all(account_home)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn ensure_codex_account_home_writes_file_credential_config() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_codex_account_home(&account_home).expect("account home");

        let config = fs::read_to_string(account_home.join("config.toml")).expect("config");
        assert!(config.contains("cli_auth_credentials_store = \"file\""));
    }

    #[test]
    fn ensure_codex_account_home_preserves_existing_config() {
        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");
        fs::create_dir_all(&account_home).expect("account home dir");
        let config_path = account_home.join("config.toml");
        let existing = "model = \"gpt-5\"\ncli_auth_credentials_store = \"file\"\n";
        fs::write(&config_path, existing).expect("existing config");

        ensure_codex_account_home(&account_home).expect("account home");

        let config = fs::read_to_string(config_path).expect("config");
        assert_eq!(config, existing);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_codex_account_home_sets_private_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().expect("temp dir");
        let account_home = dir.path().join("providers/codex/default");

        ensure_codex_account_home(&account_home).expect("account home");

        let mode = fs::metadata(&account_home)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }
}
