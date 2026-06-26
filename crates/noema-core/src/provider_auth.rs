//! Provider authentication support.

use std::{fs, io, path::Path};

const REDACTED_PATH: &str = "[redacted-path]";
const REDACTED_TOKEN: &str = "[redacted-token]";

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

/// Redact sensitive-looking provider auth output before returning it to the UI.
#[must_use]
pub fn redact_auth_output(output: &str) -> String {
    let mut redacted = String::with_capacity(output.len());
    let mut redact_next_token = false;
    let mut index = 0;

    while index < output.len() {
        let Some(character) = output[index..].chars().next() else {
            break;
        };

        let token_start = index;
        if character.is_whitespace() {
            index += character.len_utf8();
            while index < output.len() {
                let next = output[index..].chars().next().expect("character");
                if !next.is_whitespace() {
                    break;
                }
                index += next.len_utf8();
            }
            redacted.push_str(&output[token_start..index]);
            continue;
        }

        index += character.len_utf8();
        while index < output.len() {
            let next = output[index..].chars().next().expect("character");
            if next.is_whitespace() {
                break;
            }
            index += next.len_utf8();
        }

        let token = &output[token_start..index];
        if redact_next_token {
            redacted.push_str(REDACTED_TOKEN);
            redact_next_token = false;
            continue;
        }

        redacted.push_str(&redact_word(token));
        if is_bearer_token(token) {
            redact_next_token = true;
        }
    }

    redacted
}

fn redact_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    if lower.contains("auth.json")
        || lower.contains("/providers/")
        || lower.contains("\\providers\\")
    {
        return REDACTED_PATH.to_string();
    }
    if let Some(redacted) = redact_keyed_secret(word) {
        return redacted;
    }
    if lower.starts_with("sk-")
        || lower.starts_with("codex_")
        || lower.starts_with("codex_access_token=")
        || lower.contains("access_token=")
    {
        return REDACTED_TOKEN.to_string();
    }
    word.to_string()
}

fn redact_keyed_secret(word: &str) -> Option<String> {
    let lower = word.to_ascii_lowercase();
    for key in [
        "openai_api_key",
        "codex_access_token",
        "refresh_token",
        "access_token",
        "api_key",
    ] {
        let mut search_from = 0;
        while let Some(relative_start) = lower[search_from..].find(key) {
            let key_start = search_from + relative_start;
            let mut separator_index = key_start + key.len();

            if matches!(word.as_bytes().get(separator_index), Some(b'"' | b'\'')) {
                separator_index += 1;
            }
            if !matches!(word.as_bytes().get(separator_index), Some(b'=' | b':')) {
                search_from = key_start + key.len();
                continue;
            }

            let mut value_start = separator_index + 1;
            let quote = match word.as_bytes().get(value_start) {
                Some(b'"') => {
                    value_start += 1;
                    Some(b'"')
                }
                Some(b'\'') => {
                    value_start += 1;
                    Some(b'\'')
                }
                _ => None,
            };
            let value_end = find_secret_value_end(word, value_start, quote);

            let mut redacted = String::with_capacity(word.len());
            redacted.push_str(&word[..value_start]);
            redacted.push_str(REDACTED_TOKEN);
            redacted.push_str(&word[value_end..]);
            return Some(redacted);
        }
    }

    None
}

fn find_secret_value_end(word: &str, value_start: usize, quote: Option<u8>) -> usize {
    let bytes = word.as_bytes();
    if let Some(quote) = quote {
        return bytes[value_start..]
            .iter()
            .position(|byte| *byte == quote)
            .map_or(word.len(), |position| value_start + position);
    }

    bytes[value_start..]
        .iter()
        .position(|byte| matches!(*byte, b',' | b';' | b'}' | b']'))
        .map_or(word.len(), |position| value_start + position)
}

fn is_bearer_token(token: &str) -> bool {
    token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .eq_ignore_ascii_case("bearer")
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

    #[test]
    fn redact_auth_output_removes_realistic_secret_shapes_without_flattening_output() {
        let output = concat!(
            "OPENAI_API_KEY=sk-secret\n",
            "api_key=sk-secret refresh_token=abc access_token=abc\n",
            "{\"access_token\":\"json-secret\"}\n",
            "Authorization: Bearer bearer-secret\n",
            "Bearer standalone-secret\n",
            "path /tmp/noema/providers/codex/default/auth.json\n",
            "win C:\\Users\\me\\.noema\\providers\\codex\\default\\auth.json\n",
        );

        let redacted = redact_auth_output(output);

        for secret in [
            "sk-secret",
            "abc",
            "json-secret",
            "bearer-secret",
            "standalone-secret",
            "/tmp/noema/providers/codex/default/auth.json",
            "C:\\Users\\me\\.noema\\providers\\codex\\default\\auth.json",
        ] {
            assert!(
                !redacted.contains(secret),
                "{secret} was not redacted in {redacted}"
            );
        }
        assert!(redacted.contains("OPENAI_API_KEY=[redacted-token]"));
        assert!(redacted.contains("api_key=[redacted-token]"));
        assert!(redacted.contains("refresh_token=[redacted-token]"));
        assert!(redacted.contains("access_token=[redacted-token]"));
        assert!(redacted.contains("\"access_token\":\"[redacted-token]\""));
        assert!(redacted.contains("Authorization: Bearer [redacted-token]"));
        assert!(redacted.contains("Bearer [redacted-token]"));
        assert!(redacted.contains("[redacted-path]"));
        assert_eq!(redacted.matches('\n').count(), output.matches('\n').count());
    }
}
