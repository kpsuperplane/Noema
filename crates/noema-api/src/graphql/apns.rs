//! APNs credential authority, GraphQL types, and protected-file helpers.

use std::fs;

use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_capability_adapters::{
    create_private_dir, random_hex, read_bounded_regular_file, require_regular_directory,
    sync_directory, write_new_file,
};
use noema_home::NoemaPaths;
use noema_store::{ApnsEnvironment, ClientNotificationRecord};
use reqwest::StatusCode;
use ring::digest;
use serde::{Deserialize, Serialize};
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair};

pub(crate) const APNS_TOPIC: &str = "dev.noema.app.ios";
pub(crate) const LIVE_ACTIVITY_TOPIC: &str = "dev.noema.app.ios.push-type.liveactivity";
pub(crate) const LIVE_ACTIVITY_ATTRIBUTES_TYPE: &str = "NoemaTasksActivityAttributes";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ApnsEnvironment")]
pub(crate) enum GraphqlApnsEnvironment {
    #[graphql(name = "DEVELOPMENT")]
    Development,
    #[graphql(name = "PRODUCTION")]
    Production,
}

impl From<GraphqlApnsEnvironment> for ApnsEnvironment {
    fn from(value: GraphqlApnsEnvironment) -> Self {
        match value {
            GraphqlApnsEnvironment::Development => Self::Development,
            GraphqlApnsEnvironment::Production => Self::Production,
        }
    }
}

impl From<ApnsEnvironment> for GraphqlApnsEnvironment {
    fn from(value: ApnsEnvironment) -> Self {
        match value {
            ApnsEnvironment::Development => Self::Development,
            ApnsEnvironment::Production => Self::Production,
        }
    }
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ApnsProviderStatus")]
pub(crate) struct GraphqlApnsProviderStatus {
    pub configured: bool,
    pub team_id: Option<String>,
    pub key_id: Option<String>,
    pub topic: String,
    pub key_fingerprint: Option<String>,
    pub revision: i64,
    pub updated_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_at: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ClientNotificationStatus")]
pub(crate) struct GraphqlClientNotificationStatus {
    pub available: bool,
    pub blocker: Option<String>,
    pub enabled: bool,
    pub environment: Option<GraphqlApnsEnvironment>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ClientLiveActivityStatus")]
pub(crate) struct GraphqlClientLiveActivityStatus {
    pub available: bool,
    pub blocker: Option<String>,
    pub enabled: bool,
    pub registered: bool,
    pub environment: Option<GraphqlApnsEnvironment>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ClientNotificationPresenceEvent")]
pub(crate) struct GraphqlClientNotificationPresenceEvent {
    pub client_id: String,
    pub ready: bool,
}

#[derive(Clone, InputObject)]
#[graphql(name = "ConfigureApnsProviderInput")]
pub(crate) struct GraphqlConfigureApnsProviderInput {
    pub team_id: String,
    pub key_id: String,
    pub private_key_pem: String,
    pub expected_revision: i64,
}

#[derive(Clone, InputObject)]
#[graphql(name = "RegisterClientNotificationsInput")]
pub(crate) struct GraphqlRegisterClientNotificationsInput {
    pub device_token: String,
    pub environment: GraphqlApnsEnvironment,
}

#[derive(Clone, InputObject)]
#[graphql(name = "RegisterClientLiveActivitiesInput")]
pub(crate) struct GraphqlRegisterClientLiveActivitiesInput {
    pub push_to_start_token: String,
    pub environment: GraphqlApnsEnvironment,
    pub active_activity_ids: Vec<String>,
}

#[derive(Clone, InputObject)]
#[graphql(name = "RegisterClientLiveActivityUpdateInput")]
pub(crate) struct GraphqlRegisterClientLiveActivityUpdateInput {
    pub activity_id: String,
    pub update_token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ApnsCredential {
    pub configured: bool,
    pub team_id: Option<String>,
    pub key_id: Option<String>,
    pub topic: String,
    pub key_fingerprint: Option<String>,
    pub private_key_pem: Option<String>,
    pub revision: u64,
    pub updated_at: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_at: Option<String>,
}

impl From<ApnsCredential> for GraphqlApnsProviderStatus {
    fn from(value: ApnsCredential) -> Self {
        Self {
            configured: value.configured,
            team_id: value.team_id,
            key_id: value.key_id,
            topic: value.topic,
            key_fingerprint: value.key_fingerprint,
            revision: i64::try_from(value.revision).unwrap_or(i64::MAX),
            updated_at: value.updated_at,
            last_error_code: value.last_error_code,
            last_error_at: value.last_error_at,
        }
    }
}

pub(crate) fn client_status(
    credential: &ApnsCredential,
    registration: Option<&ClientNotificationRecord>,
) -> GraphqlClientNotificationStatus {
    GraphqlClientNotificationStatus {
        available: credential.configured,
        blocker: (!credential.configured).then(|| "APNs provider is not configured".to_string()),
        enabled: registration.is_some(),
        environment: registration.map(|record| record.environment.into()),
    }
}

pub(crate) fn live_activity_status(
    credential: &ApnsCredential,
    registration: Option<&noema_store::ClientLiveActivityRegistration>,
) -> GraphqlClientLiveActivityStatus {
    GraphqlClientLiveActivityStatus {
        available: credential.configured,
        blocker: (!credential.configured).then(|| "APNs provider is not configured".to_string()),
        enabled: registration.is_none_or(|record| record.enabled),
        registered: registration.is_some_and(|record| record.push_to_start_token.is_some()),
        environment: registration.and_then(|record| record.environment.map(Into::into)),
    }
}

#[derive(Debug)]
pub(crate) enum ApnsSendError {
    Provider(&'static str),
    Transport(&'static str),
    InvalidToken,
}

pub(crate) fn delivery_disposition_apns(
    result: std::result::Result<StatusCode, ApnsSendError>,
) -> (&'static str, Option<&'static str>) {
    match result {
        Ok(status) if status.is_success() => ("delivered", None),
        Ok(status) if matches!(status.as_u16(), 400 | 410) => ("failed", Some("remote_rejected")),
        Ok(status) if status.as_u16() == 403 => ("failed", Some("provider_auth_rejected")),
        Ok(status) if status.as_u16() == 429 || status.is_server_error() => {
            ("retry", Some("remote_retry"))
        }
        Ok(_) => ("failed", Some("remote_rejected")),
        Err(ApnsSendError::InvalidToken) => ("invalid_token", Some("invalid_device_token")),
        Err(ApnsSendError::Provider(code)) => ("failed", Some(code)),
        Err(ApnsSendError::Transport(code)) => ("retry", Some(code)),
    }
}

pub(crate) fn positive_revision(value: i64) -> Result<u64> {
    u64::try_from(value)
        .map_err(|_| async_graphql::Error::new("APNs provider revision must be non-negative"))
}

pub(crate) fn validate_apns_identifier(value: &str, label: &str) -> Result<()> {
    if value.len() != 10
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(async_graphql::Error::new(format!(
            "APNs {label} must be 10 uppercase letters or digits"
        )));
    }
    Ok(())
}

pub(crate) fn is_pkcs8_pem(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("-----BEGIN PRIVATE KEY-----") && value.ends_with("-----END PRIVATE KEY-----")
}

pub(crate) fn now_timestamp() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn read_apns_credential(
    paths: &NoemaPaths,
) -> std::result::Result<ApnsCredential, String> {
    let path = paths.apns_provider_path();
    let bytes = match fs::symlink_metadata(&path) {
        Ok(_) => {
            require_regular_directory(&paths.notifications_dir())
                .map_err(|_| "APNs credential directory is unsafe".to_string())?;
            read_bounded_regular_file(&path, 64 * 1024)
                .map_err(|_| "APNs provider credential is unavailable".to_string())?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ApnsCredential {
                configured: false,
                team_id: None,
                key_id: None,
                topic: APNS_TOPIC.to_string(),
                key_fingerprint: None,
                private_key_pem: None,
                revision: 0,
                updated_at: None,
                last_error_code: None,
                last_error_at: None,
            });
        }
        Err(_) => return Err("APNs provider credential is unavailable".to_string()),
    };
    let credential: ApnsCredential = serde_json::from_slice(&bytes)
        .map_err(|_| "APNs provider credential is invalid".to_string())?;
    validate_apns_credential(&credential)?;
    Ok(credential)
}

pub(crate) fn validate_apns_credential(
    credential: &ApnsCredential,
) -> std::result::Result<(), String> {
    if credential.topic != APNS_TOPIC
        || credential
            .last_error_code
            .as_deref()
            .is_some_and(|code| code.is_empty() || code.len() > 128 || code.trim() != code)
    {
        return Err("APNs provider credential is invalid".to_string());
    }
    if !credential.configured {
        if credential.team_id.is_some()
            || credential.key_id.is_some()
            || credential.key_fingerprint.is_some()
            || credential.private_key_pem.is_some()
        {
            return Err("APNs provider tombstone is invalid".to_string());
        }
        return Ok(());
    }
    let team_id = credential
        .team_id
        .as_deref()
        .ok_or_else(|| "APNs provider metadata is incomplete".to_string())?;
    let key_id = credential
        .key_id
        .as_deref()
        .ok_or_else(|| "APNs provider metadata is incomplete".to_string())?;
    let fingerprint = credential
        .key_fingerprint
        .as_deref()
        .ok_or_else(|| "APNs provider metadata is incomplete".to_string())?;
    let private_key = credential
        .private_key_pem
        .as_deref()
        .ok_or_else(|| "APNs provider key is unavailable".to_string())?;
    if team_id.len() != 10
        || !team_id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        || key_id.len() != 10
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        || private_key.len() > 16 * 1024
        || !is_pkcs8_pem(private_key)
        || fingerprint.len() != 64
        || !fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("APNs provider metadata is invalid".to_string());
    }
    let key_pair = ES256KeyPair::from_pem(private_key)
        .map_err(|_| "APNs private key is invalid".to_string())?;
    let actual = hex_digest(
        digest::digest(
            &digest::SHA256,
            key_pair
                .public_key()
                .public_key()
                .to_bytes_uncompressed()
                .as_ref(),
        )
        .as_ref(),
    );
    if actual != fingerprint {
        return Err("APNs key fingerprint does not match private key".to_string());
    }
    Ok(())
}

pub(crate) fn write_apns_credential(
    paths: &NoemaPaths,
    credential: &ApnsCredential,
) -> std::result::Result<(), String> {
    validate_apns_credential(credential)?;
    create_private_dir(&paths.notifications_dir())
        .map_err(|_| "APNs credential directory is unavailable".to_string())?;
    let path = paths.apns_provider_path();
    match fs::symlink_metadata(&path) {
        Ok(_) => read_bounded_regular_file(&path, 64 * 1024)
            .map(|_| ())
            .map_err(|_| "APNs provider credential path is unsafe".to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("APNs provider credential path is unavailable".to_string()),
    }
    let nonce = random_hex(16).map_err(|_| "APNs credential staging failed".to_string())?;
    let temp = path.with_extension(format!("json.{nonce}.tmp"));
    let bytes = serde_json::to_vec_pretty(credential)
        .map_err(|_| "APNs provider credential could not be encoded".to_string())?;
    write_new_file(&temp, &bytes)
        .map_err(|_| "APNs provider credential could not be staged".to_string())?;
    if fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
        return Err("APNs provider credential could not be committed".to_string());
    }
    sync_directory(&paths.notifications_dir())
        .map_err(|_| "APNs credential directory could not be synchronized".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn provider_status_preserves_ids_without_exposing_private_key() {
        let status: GraphqlApnsProviderStatus = ApnsCredential {
            configured: true,
            team_id: Some("TEAM123456".to_string()),
            key_id: Some("KEYID12345".to_string()),
            topic: APNS_TOPIC.to_string(),
            key_fingerprint: Some("a".repeat(64)),
            private_key_pem: Some("PRIVATE KEY MATERIAL".to_string()),
            revision: 3,
            updated_at: Some("2026-08-08T00:00:00.000Z".to_string()),
            last_error_code: None,
            last_error_at: None,
        }
        .into();
        assert_eq!(status.team_id.as_deref(), Some("TEAM123456"));
        assert_eq!(status.key_id.as_deref(), Some("KEYID12345"));
        assert!(!format!("{status:?}").contains("PRIVATE KEY MATERIAL"));
    }

    #[test]
    fn protected_credential_file_round_trips_tombstone_and_permissions() {
        let root = TempDir::new().expect("credential root");
        let paths = NoemaPaths::from_noema_home(root.path()).expect("paths");
        let credential = ApnsCredential {
            configured: false,
            team_id: None,
            key_id: None,
            topic: APNS_TOPIC.to_string(),
            key_fingerprint: None,
            private_key_pem: None,
            revision: 4,
            updated_at: Some("2026-08-08T00:00:00.000Z".to_string()),
            last_error_code: None,
            last_error_at: None,
        };
        write_apns_credential(&paths, &credential).expect("write tombstone");
        assert_eq!(
            read_apns_credential(&paths)
                .expect("read tombstone")
                .revision,
            4
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(paths.notifications_dir())
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(paths.apns_provider_path())
                    .expect("file metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            std::fs::remove_file(paths.apns_provider_path()).expect("remove credential");
            let outside = root.path().join("outside.json");
            std::fs::write(&outside, b"untouched").expect("write outside file");
            std::os::unix::fs::symlink(&outside, paths.apns_provider_path())
                .expect("link credential path");
            assert!(read_apns_credential(&paths).is_err());
            assert!(write_apns_credential(&paths, &credential).is_err());
            assert_eq!(
                std::fs::read(&outside).expect("read outside file"),
                b"untouched"
            );
        }
    }
}
