//! Disk-backed MCP secret storage.

use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};

const MCP_SECRET_FILE: &str = "secrets.json";

/// Secret MCP setup material stored outside the canonical structured store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSecretMaterial {
    /// Secret environment variables for stdio MCP servers.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Secret headers for HTTP-based MCP servers.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// OAuth client-credentials material for HTTP-based MCP servers.
    #[serde(default)]
    pub oauth_client_credentials: Option<McpOAuthClientCredentials>,
}

impl McpSecretMaterial {
    /// Whether this setup material contains any configured secret.
    #[must_use]
    pub fn has_secret_material(&self) -> bool {
        !self.env.is_empty() || !self.headers.is_empty() || self.oauth_client_credentials.is_some()
    }
}

/// OAuth 2.0 client-secret credentials for MCP client-credentials flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOAuthClientCredentials {
    /// OAuth client id.
    pub client_id: String,
    /// OAuth client secret.
    pub client_secret: String,
    /// Requested OAuth scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Write MCP secrets under one server's private home directory.
///
/// # Errors
///
/// Returns an I/O error when directory creation, serialization, writing, or
/// permission updates fail.
pub fn write_mcp_secrets(server_home: &Path, secrets: &McpSecretMaterial) -> io::Result<()> {
    create_private_dir_all(server_home)?;
    let path = server_home.join(MCP_SECRET_FILE);
    let bytes = serde_json::to_vec_pretty(secrets).map_err(io::Error::other)?;
    fs::write(&path, bytes)?;
    set_private_file_permissions(&path)
}

/// Read MCP secrets from one server's private home directory.
///
/// # Errors
///
/// Returns an I/O error when the secret file cannot be read or decoded.
pub fn read_mcp_secrets(server_home: &Path) -> io::Result<McpSecretMaterial> {
    let path = server_home.join(MCP_SECRET_FILE);
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Remove one MCP server's private secret/configuration directory if present.
///
/// # Errors
///
/// Returns an I/O error when an existing directory cannot be removed.
pub fn remove_mcp_secrets_dir(server_home: &Path) -> io::Result<()> {
    match fs::remove_dir_all(server_home) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir_all(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_file_round_trips_env_and_headers_without_safe_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("mcp_server");
        let secrets = McpSecretMaterial {
            env: map_from_pairs([("GITHUB_TOKEN", "secret")]),
            headers: map_from_pairs([("Authorization", "Bearer secret")]),
            oauth_client_credentials: None,
        };

        write_mcp_secrets(&home, &secrets).expect("write secrets");
        let loaded = read_mcp_secrets(&home).expect("read secrets");

        assert_eq!(loaded, secrets);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(home.join("secrets.json")).expect("secret file")
            )
            .expect("json"),
            json!({
                "env": { "GITHUB_TOKEN": "secret" },
                "headers": { "Authorization": "Bearer secret" },
                "oauth_client_credentials": null
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn secret_home_and_file_are_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("mcp_server");

        write_mcp_secrets(&home, &McpSecretMaterial::default()).expect("write secrets");

        assert_eq!(
            std::fs::metadata(&home)
                .expect("home metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(home.join("secrets.json"))
                .expect("file metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn remove_secret_home_ignores_missing_directory_and_removes_existing_one() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("mcp_server");

        remove_mcp_secrets_dir(&home).expect("missing ok");
        write_mcp_secrets(&home, &McpSecretMaterial::default()).expect("write secrets");
        assert!(home.exists());

        remove_mcp_secrets_dir(&home).expect("remove secrets");

        assert!(!home.exists());
    }

    fn map_from_pairs<const N: usize>(
        pairs: [(&str, &str); N],
    ) -> std::collections::BTreeMap<String, String> {
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }
}
