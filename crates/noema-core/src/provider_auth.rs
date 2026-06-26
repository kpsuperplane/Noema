//! Provider authentication support.

use std::{fs, io, path::Path};

/// Prepare a Codex account home for portable file-backed credentials.
///
/// # Errors
///
/// Returns an error when the account directory or config file cannot be written.
pub fn ensure_codex_account_home(account_home: &Path) -> io::Result<()> {
    fs::create_dir_all(account_home)?;
    let config_path = account_home.join("config.toml");
    if !config_path.exists() {
        fs::write(config_path, "cli_auth_credentials_store = \"file\"\n")?;
    }
    Ok(())
}

/// Redact sensitive-looking provider auth output before returning it to the UI.
#[must_use]
pub fn redact_auth_output(output: &str) -> String {
    output
        .split_whitespace()
        .map(redact_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    if lower.contains("auth.json") || lower.contains("/providers/") {
        return "[redacted-path]".to_string();
    }
    if lower.starts_with("sk-")
        || lower.starts_with("codex_")
        || lower.starts_with("codex_access_token=")
        || lower.contains("access_token=")
    {
        return "[redacted-token]".to_string();
    }
    word.to_string()
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
    fn redact_auth_output_removes_secret_like_values() {
        let output =
            "token sk-secret CODEX_ACCESS_TOKEN=abc auth.json /tmp/noema/providers/codex/default";

        let redacted = redact_auth_output(output);

        assert!(!redacted.contains("sk-secret"));
        assert!(!redacted.contains("abc"));
        assert!(redacted.contains("[redacted-token]"));
        assert!(redacted.contains("[redacted-path]"));
    }
}
