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
    Recovery {
        metadata: Option<RemoteMetadata>,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMetadata {
    pub(crate) origin: String,
    pub(crate) client_id: String,
}

pub(crate) struct RemoteProfile {
    pub(crate) metadata: RemoteMetadata,
    pub(crate) token: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum StoredSelection {
    Local,
    Remote { origin: String, client_id: String },
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
                        metadata: None,
                        message: "Noema could not read its desktop connection settings."
                            .to_string(),
                    };
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => StoredSelection::Local,
            Err(_) => {
                return DesktopSelection::Recovery {
                    metadata: None,
                    message: "Noema could not read its desktop connection settings.".to_string(),
                };
            }
        };
        match selection {
            StoredSelection::Local => DesktopSelection::Local,
            StoredSelection::Remote { origin, client_id } => {
                let metadata = RemoteMetadata { origin, client_id };
                let token = self
                    .credential
                    .as_ref()
                    .map_err(|message| message.clone())
                    .and_then(|credential| {
                        credential.get_password().map_err(|_| {
                            "Noema could not read the remote client credential from secure storage."
                                .to_string()
                        })
                    });
                match token {
                    Ok(token) => DesktopSelection::Remote(RemoteProfile { metadata, token }),
                    Err(message) => DesktopSelection::Recovery {
                        metadata: Some(metadata),
                        message,
                    },
                }
            }
        }
    }

    pub(crate) fn save_remote(&self, profile: &RemoteProfile) -> Result<(), String> {
        let credential = self
            .credential
            .as_ref()
            .map_err(|message| message.clone())?;
        credential.set_password(&profile.token).map_err(|_| {
            "Noema could not store the remote client credential securely.".to_string()
        })?;
        let selection = StoredSelection::Remote {
            origin: profile.metadata.origin.clone(),
            client_id: profile.metadata.client_id.clone(),
        };
        if let Err(error) = self.write_selection(&selection) {
            let _ = credential.delete_credential();
            return Err(error);
        }
        Ok(())
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
            token: "client-one.private-bearer".to_string(),
        };
        store.save_remote(&profile).expect("save profile");
        let stored = fs::read_to_string(root.path().join("desktop.json")).expect("settings");
        assert!(stored.contains("https://noema.example"));
        assert!(!stored.contains("private-bearer"));

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
