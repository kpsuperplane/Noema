//! Write-only secret-input provider account storage.

use super::account_service::filesystem::{
    FileSnapshot, atomic_write_private, remove_file_if_exists, restore_file, snapshot_file,
};
use crate::ProviderError;
use serde::{Deserialize, Serialize};
use std::{fmt, fs, path::PathBuf};

const API_KEY_FILE_NAME: &str = "api_key.json";

/// File-backed API key store for a single provider account.
#[derive(Clone)]
pub struct SecretInputStore {
    account_home: PathBuf,
}

impl fmt::Debug for SecretInputStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretInputStore")
            .field("account_home", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct ApiKeyFile {
    api_key: String,
}

impl SecretInputStore {
    /// Build a secret store under the provider account home.
    #[must_use]
    pub fn new(account_home: impl Into<PathBuf>) -> Self {
        Self {
            account_home: account_home.into(),
        }
    }

    /// Return the API key file path.
    #[must_use]
    pub fn secret_path(&self) -> PathBuf {
        self.account_home.join(API_KEY_FILE_NAME)
    }

    /// Save a non-empty API key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the key is blank or the secret file
    /// cannot be written.
    pub fn save_api_key(&self, api_key: &str) -> Result<(), ProviderError> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::InvalidRequest {
                message: "api key is required".to_string(),
            });
        }
        let payload = serde_json::to_vec(&ApiKeyFile {
            api_key: api_key.to_string(),
        })
        .map_err(|source| ProviderError::InvalidRequest {
            message: source.to_string(),
        })?;
        atomic_write_private(&self.secret_path(), &payload).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: "secret_input".to_string(),
                message: format!("provider secret could not be written: {source}"),
            }
        })
    }

    /// Load the saved API key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::MissingCredentials`] when no usable key exists.
    pub fn load_api_key(&self) -> Result<String, ProviderError> {
        let text = fs::read_to_string(self.secret_path()).map_err(|_| {
            ProviderError::MissingCredentials {
                provider: "secret_input".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            }
        })?;
        let file: ApiKeyFile =
            serde_json::from_str(&text).map_err(|_| ProviderError::MissingCredentials {
                provider: "secret_input".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            })?;
        let api_key = file.api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::MissingCredentials {
                provider: "secret_input".to_string(),
                credential: API_KEY_FILE_NAME.to_string(),
            });
        }
        Ok(api_key.to_string())
    }

    /// Remove the saved API key, if present.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when removal fails for a reason other than a
    /// missing file.
    pub fn clear_api_key(&self) -> Result<(), ProviderError> {
        remove_file_if_exists(&self.secret_path()).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: "secret_input".to_string(),
                message: format!("provider secret could not be removed: {source}"),
            }
        })
    }

    pub(crate) fn snapshot(&self) -> Result<FileSnapshot, ProviderError> {
        snapshot_file(&self.secret_path()).map_err(|source| ProviderError::ProviderUnavailable {
            provider: "secret_input".to_string(),
            message: format!("provider secret could not be snapshotted: {source}"),
        })
    }

    pub(crate) fn restore(&self, snapshot: &FileSnapshot) -> Result<(), ProviderError> {
        restore_file(&self.secret_path(), snapshot).map_err(|source| {
            ProviderError::ProviderUnavailable {
                provider: "secret_input".to_string(),
                message: format!("provider secret could not be restored: {source}"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_clear_and_protect_api_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SecretInputStore::new(dir.path().join("providers/exa/acct_one"));

        store.save_api_key("secret-key").expect("save");
        assert_eq!(store.load_api_key().expect("load"), "secret-key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                (
                    fs::metadata(&store.account_home)
                        .expect("account metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                    fs::metadata(store.secret_path())
                        .expect("secret metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                ),
                (0o700, 0o600)
            );
        }
        store.clear_api_key().expect("clear");
        assert!(matches!(
            store.load_api_key(),
            Err(crate::ProviderError::MissingCredentials { .. })
        ));
    }

    #[test]
    fn rejects_blank_api_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SecretInputStore::new(dir.path().join("providers/exa/acct_one"));

        assert!(matches!(
            store.save_api_key("  "),
            Err(crate::ProviderError::InvalidRequest { .. })
        ));
        assert!(!store.secret_path().exists());
    }

    #[test]
    fn secret_input_store_debug_redacts_account_path() {
        let store = SecretInputStore::new("/private/provider-secret-path");
        let debug = format!("{store:?}");

        assert!(!debug.contains("provider-secret-path"));
        assert!(debug.contains("[REDACTED]"));
    }
}
