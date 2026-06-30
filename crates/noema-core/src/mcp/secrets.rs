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
    /// Secret headers for HTTP/SSE MCP servers.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
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
                "headers": { "Authorization": "Bearer secret" }
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

    fn map_from_pairs<const N: usize>(
        pairs: [(&str, &str); N],
    ) -> std::collections::BTreeMap<String, String> {
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }
}
