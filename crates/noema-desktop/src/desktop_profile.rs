//! Desktop connection selection and protected remote credentials.

use std::{fs, io::Write, path::PathBuf};

use keyring::Entry;
use serde::{Deserialize, Serialize};

const CREDENTIAL_SERVICE: &str = "dev.noema.app.desktop.remote";
const CREDENTIAL_ACCOUNT: &str = "active";

pub(crate) struct DesktopProfileStore {
    config_path: PathBuf,
    credential: Result<Entry, String>,
}

pub(crate) enum DesktopSelection {
    Local,
    Remote(RemoteProfile),
    Recovery { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMetadata {
    pub(crate) origin: String,
    pub(crate) client_id: String,
}

pub(crate) struct RemoteProfile {
    pub(crate) metadata: RemoteMetadata,
    pub(crate) refresh_token: String,
    pub(crate) access_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum StoredSelection {
    Local,
    Remote,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredCredential {
    origin: String,
    client_id: String,
    refresh_token: String,
}

impl DesktopProfileStore {
    pub(crate) fn new(config_path: PathBuf) -> Self {
        let credential = Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT)
            .map_err(|_| "Noema could not open the operating system credential store.".to_string());
        Self {
            config_path,
            credential,
        }
    }

    pub(crate) fn load(&self) -> DesktopSelection {
        let selection = match fs::read(&self.config_path) {
            Ok(bytes) => match serde_json::from_slice::<StoredSelection>(&bytes) {
                Ok(selection) => selection,
                Err(_) => {
                    return DesktopSelection::Recovery {
                        message: "Noema could not read its desktop connection settings."
                            .to_string(),
                    };
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => StoredSelection::Local,
            Err(_) => {
                return DesktopSelection::Recovery {
                    message: "Noema could not read its desktop connection settings.".to_string(),
                };
            }
        };
        match selection {
            StoredSelection::Local => DesktopSelection::Local,
            StoredSelection::Remote => {
                let profile = self
                    .credential
                    .as_ref()
                    .map_err(|message| message.clone())
                    .and_then(|credential| {
                        let saved = credential.get_password().map_err(|_| {
                            "Noema could not read the remote client credential from secure storage."
                                .to_string()
                        })?;
                        let saved: StoredCredential =
                            serde_json::from_str(&saved).map_err(|_| {
                                "Noema could not read the protected remote profile.".to_string()
                            })?;
                        validate_credential(&saved)?;
                        Ok(RemoteProfile {
                            metadata: RemoteMetadata {
                                origin: saved.origin,
                                client_id: saved.client_id,
                            },
                            refresh_token: saved.refresh_token,
                            access_token: None,
                        })
                    });
                match profile {
                    Ok(profile) => DesktopSelection::Remote(profile),
                    Err(message) => DesktopSelection::Recovery { message },
                }
            }
        }
    }

    pub(crate) fn save_remote(&self, profile: &RemoteProfile) -> Result<(), String> {
        let credential = self
            .credential
            .as_ref()
            .map_err(|message| message.clone())?;
        let saved = StoredCredential {
            origin: profile.metadata.origin.clone(),
            client_id: profile.metadata.client_id.clone(),
            refresh_token: profile.refresh_token.clone(),
        };
        validate_credential(&saved)?;
        let saved = serde_json::to_string(&saved)
            .map_err(|_| "Noema could not encode the protected remote profile.".to_string())?;
        credential.set_password(&saved).map_err(|_| {
            "Noema could not store the remote client credential securely.".to_string()
        })?;
        if let Err(error) = self.write_selection(&StoredSelection::Remote) {
            let _ = credential.delete_credential();
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn save_refresh(
        &self,
        metadata: &RemoteMetadata,
        refresh_token: &str,
    ) -> Result<(), String> {
        self.save_remote(&RemoteProfile {
            metadata: metadata.clone(),
            refresh_token: refresh_token.to_string(),
            access_token: None,
        })
    }

    pub(crate) fn clear_remote(&self) -> Result<(), String> {
        let credential = self
            .credential
            .as_ref()
            .map_err(|message| message.clone())?;
        match credential.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => {
                return Err("Noema could not remove the remote client credential.".to_string());
            }
        }
        self.write_selection(&StoredSelection::Local)
    }

    fn write_selection(&self, selection: &StoredSelection) -> Result<(), String> {
        let parent = self
            .config_path
            .parent()
            .ok_or_else(|| "Noema could not resolve its desktop settings directory.".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|_| "Noema could not create its desktop settings directory.".to_string())?;
        let bytes = serde_json::to_vec_pretty(selection)
            .map_err(|_| "Noema could not encode its desktop connection settings.".to_string())?;
        let temporary_path = self.config_path.with_extension("json.tmp");
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary_path)
            .map_err(|_| "Noema could not write its desktop connection settings.".to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|_| "Noema could not write its desktop connection settings.".to_string())?;
        fs::rename(&temporary_path, &self.config_path)
            .map_err(|_| "Noema could not save its desktop connection settings.".to_string())
    }
}

fn validate_credential(credential: &StoredCredential) -> Result<(), String> {
    if credential.origin.len() > 2048
        || credential.client_id.len() > 128
        || credential.refresh_token.len() > 128
        || credential.origin.is_empty()
        || credential.client_id.is_empty()
        || credential.refresh_token.is_empty()
    {
        return Err("The protected remote profile is invalid.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use keyring::mock::MockCredential;
    use tempfile::TempDir;

    use super::*;

    fn test_store(root: &TempDir) -> DesktopProfileStore {
        DesktopProfileStore {
            config_path: root.path().join("desktop.json"),
            credential: Ok(Entry::new_with_credential(Box::<MockCredential>::default())),
        }
    }

    #[test]
    fn stored_selection_excludes_the_bearer_and_requires_secure_storage() {
        let root = TempDir::new().expect("root");
        let store = test_store(&root);
        let profile = RemoteProfile {
            metadata: RemoteMetadata {
                origin: "https://noema.example".to_string(),
                client_id: "client-one".to_string(),
            },
            refresh_token: "private-refresh-token".to_string(),
            access_token: Some("memory-access-token".to_string()),
        };
        store.save_remote(&profile).expect("save profile");
        let stored = fs::read_to_string(root.path().join("desktop.json")).expect("settings");
        assert!(!stored.contains("https://noema.example"));
        assert!(!stored.contains("client-one"));
        assert!(!stored.contains("private-refresh-token"));
        let protected = store
            .credential
            .as_ref()
            .expect("credential")
            .get_password()
            .expect("protected profile");
        assert!(protected.contains("private-refresh-token"));
        assert!(!protected.contains("memory-access-token"));

        let unavailable_path = root.path().join("unavailable.json");
        let unavailable = DesktopProfileStore {
            config_path: unavailable_path.clone(),
            credential: Err("credential store unavailable".to_string()),
        };
        assert!(unavailable.save_remote(&profile).is_err());
        assert!(!unavailable_path.exists());

        store
            .credential
            .as_ref()
            .expect("credential")
            .delete_credential()
            .expect("remove credential");
        assert!(matches!(store.load(), DesktopSelection::Recovery { .. }));
    }
}
