use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use serde_yaml::{Mapping, Value};
use thiserror::Error;

/// Serialized recovery-code rotation authority for one startup configuration.
#[derive(Clone)]
pub struct RecoveryCodeStore {
    path: PathBuf,
    state: Arc<Mutex<RecoveryState>>,
}

#[derive(Default)]
struct RecoveryState {
    disabled: bool,
}

struct RecoveryCode([u8; 32]);

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryCode([REDACTED])")
    }
}

impl fmt::Debug for RecoveryCodeStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryCodeStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl RecoveryCodeStore {
    /// Ensure that the startup file contains one valid private recovery code.
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryCodeError`] when the file cannot be read, validated, or secured.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, RecoveryCodeError> {
        let path = path.into();
        let (original, mut document) = read_document(&path)?;
        match recovery_value(&document)? {
            Some(value) => {
                RecoveryCode::parse(value)?;
                noema_home::atomic_write_private(&path, &original).map_err(|source| {
                    RecoveryCodeError::Write {
                        path: path.clone(),
                        source,
                    }
                })?;
            }
            None => {
                let generated = RecoveryCode::generate()?;
                set_recovery_value(&mut document, &generated.encode())?;
                write_document(&path, &document)?;
                verify_committed(&path, &generated)?;
            }
        }
        Ok(Self {
            path,
            state: Arc::new(Mutex::new(RecoveryState::default())),
        })
    }

    /// Compare one candidate and durably rotate the code before returning the result.
    ///
    /// Every candidate rotates the code. A failed filesystem operation disables later attempts.
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryCodeError`] when recovery was disabled or safe rotation fails.
    pub fn attempt(&self, candidate: &str) -> Result<bool, RecoveryCodeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RecoveryCodeError::Unavailable)?;
        if state.disabled {
            return Err(RecoveryCodeError::Unavailable);
        }
        let result = self.attempt_locked(candidate);
        if result.is_err() {
            state.disabled = true;
        }
        result
    }

    fn attempt_locked(&self, candidate: &str) -> Result<bool, RecoveryCodeError> {
        let (_, mut document) = read_document(&self.path)?;
        let current = recovery_value(&document)?
            .ok_or(RecoveryCodeError::MissingField)
            .and_then(RecoveryCode::parse)?;
        let matched = RecoveryCode::candidate(candidate)
            .is_some_and(|candidate| constant_time_equal(&current.0, &candidate.0));
        let replacement = RecoveryCode::generate()?;
        set_recovery_value(&mut document, &replacement.encode())?;
        write_document(&self.path, &document)?;
        verify_committed(&self.path, &replacement)?;
        Ok(matched)
    }
}

impl RecoveryCode {
    fn generate() -> Result<Self, RecoveryCodeError> {
        let mut bytes = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| RecoveryCodeError::Random)?;
        Ok(Self(bytes))
    }

    fn parse(value: &str) -> Result<Self, RecoveryCodeError> {
        Self::candidate(value).ok_or(RecoveryCodeError::MalformedField)
    }

    fn candidate(value: &str) -> Option<Self> {
        let decoded = URL_SAFE_NO_PAD.decode(value).ok()?;
        let bytes: [u8; 32] = decoded.try_into().ok()?;
        (URL_SAFE_NO_PAD.encode(bytes) == value).then_some(Self(bytes))
    }

    fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }
}

fn read_document(path: &Path) -> Result<(Vec<u8>, Value), RecoveryCodeError> {
    let bytes = fs::read(path).map_err(|source| RecoveryCodeError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let document = serde_yaml::from_slice(&bytes).map_err(|_| RecoveryCodeError::InvalidFile)?;
    Ok((bytes, document))
}

fn recovery_value(document: &Value) -> Result<Option<&str>, RecoveryCodeError> {
    let root = document
        .as_mapping()
        .ok_or(RecoveryCodeError::InvalidFile)?;
    let Some(web) = root.get(Value::String("web".to_string())) else {
        return Ok(None);
    };
    let web = web.as_mapping().ok_or(RecoveryCodeError::InvalidWeb)?;
    match web.get(Value::String("recovery_code".to_string())) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(RecoveryCodeError::MalformedField),
    }
}

fn set_recovery_value(document: &mut Value, value: &str) -> Result<(), RecoveryCodeError> {
    let root = document
        .as_mapping_mut()
        .ok_or(RecoveryCodeError::InvalidFile)?;
    let web = root
        .entry(Value::String("web".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let web = web.as_mapping_mut().ok_or(RecoveryCodeError::InvalidWeb)?;
    web.insert(
        Value::String("recovery_code".to_string()),
        Value::String(value.to_string()),
    );
    Ok(())
}

fn write_document(path: &Path, document: &Value) -> Result<(), RecoveryCodeError> {
    let bytes = serde_yaml::to_string(document)
        .map_err(|_| RecoveryCodeError::InvalidFile)?
        .into_bytes();
    noema_home::atomic_write_private(path, &bytes).map_err(|source| RecoveryCodeError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn verify_committed(path: &Path, expected: &RecoveryCode) -> Result<(), RecoveryCodeError> {
    let (_, document) = read_document(path)?;
    let actual = recovery_value(&document)?
        .ok_or(RecoveryCodeError::MissingField)
        .and_then(RecoveryCode::parse)?;
    constant_time_equal(&actual.0, &expected.0)
        .then_some(())
        .ok_or(RecoveryCodeError::Verification)
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..left.len() {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

/// Recovery-code startup or rotation failure.
#[derive(Debug, Error)]
pub enum RecoveryCodeError {
    /// The startup file could not be read.
    #[error("failed to read startup configuration at {}: {source}", path.display())]
    Read {
        /// Startup configuration path.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// The startup file could not be replaced privately.
    #[error("failed to write startup configuration at {}: {source}", path.display())]
    Write {
        /// Startup configuration path.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// The startup file is not valid YAML mapping data.
    #[error("startup configuration is not valid YAML mapping data")]
    InvalidFile,
    /// The web configuration is not a mapping.
    #[error("web configuration must be a YAML mapping")]
    InvalidWeb,
    /// The recovery-code field is missing during rotation.
    #[error("web.recovery_code is missing")]
    MissingField,
    /// The recovery-code field is not canonical 32-byte base64url data.
    #[error("web.recovery_code must be canonical unpadded base64url for exactly 32 bytes")]
    MalformedField,
    /// Secure random generation failed.
    #[error("failed to generate a recovery code")]
    Random,
    /// The committed replacement did not match the intended code.
    #[error("failed to verify the committed recovery code")]
    Verification,
    /// Recovery is disabled after an unsafe state transition.
    #[error("recovery is unavailable until Noema restarts")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_code(path: &Path) -> String {
        let (_, document) = read_document(path).expect("document");
        recovery_value(&document)
            .expect("recovery field")
            .expect("configured code")
            .to_string()
    }

    #[test]
    fn missing_code_is_generated_privately_without_losing_other_values() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("noema/config.yaml");
        fs::create_dir_all(path.parent().expect("parent")).expect("parent");
        fs::write(&path, "provider: codex\nweb:\n  host: 127.0.0.2\n").expect("config");

        let store = RecoveryCodeStore::open(&path).expect("recovery store");
        let code = configured_code(&path);
        let (_, document) = read_document(&path).expect("document");

        assert_eq!(code.len(), 43);
        assert_eq!(document["provider"], "codex");
        assert_eq!(document["web"]["host"], "127.0.0.2");
        super::super::Config::load(Some(path.clone())).expect("load rotated startup config");
        assert!(!format!("{store:?}").contains(&code));
        assert!(!format!("{:?}", RecoveryCode::parse(&code).expect("code")).contains(&code));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).expect("metadata").permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn every_candidate_rotates_and_only_the_current_code_matches() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("config.yaml");
        fs::write(&path, "web: {}\n").expect("config");
        let store = RecoveryCodeStore::open(&path).expect("recovery store");
        let first = configured_code(&path);

        assert!(!store.attempt("malformed").expect("malformed attempt"));
        let second = configured_code(&path);
        assert_ne!(second, first);
        assert!(!store.attempt(&first).expect("stale attempt"));
        let third = configured_code(&path);
        assert_ne!(third, second);
        assert!(store.attempt(&third).expect("current attempt"));
        assert_ne!(configured_code(&path), third);
    }

    #[test]
    fn malformed_configured_codes_fail_with_field_specific_errors() {
        for value in ["", "short", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="] {
            let root = tempfile::tempdir().expect("root");
            let path = root.path().join("config.yaml");
            fs::write(&path, format!("web:\n  recovery_code: '{value}'\n")).expect("config");
            assert!(matches!(
                RecoveryCodeStore::open(path),
                Err(RecoveryCodeError::MalformedField)
            ));
        }
    }

    #[test]
    fn failed_live_read_disables_later_attempts() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("config.yaml");
        fs::write(&path, "web: {}\n").expect("config");
        let store = RecoveryCodeStore::open(&path).expect("recovery store");
        fs::remove_file(&path).expect("remove config");

        assert!(matches!(
            store.attempt("candidate"),
            Err(RecoveryCodeError::Read { .. })
        ));
        assert!(matches!(
            store.attempt("candidate"),
            Err(RecoveryCodeError::Unavailable)
        ));
    }
}
