//! SQLite row decoding and numeric conversion for local-model persistence.

use noema_providers::{
    LocalModelBackend, LocalModelEventRecord, LocalModelInstallationRecord, ProviderInstanceKey,
};

use super::StoreError;

pub(super) const INSTALLATION_SELECT: &str = r#"
SELECT installation_id, provider_instance_key, model_id, display_name, source_kind,
       source_repo, source_revision, source_file, sha256, download_gb,
       expected_bytes, downloaded_bytes, license, backend, status,
       blob_relative_path, is_active, runtime_retired_at, retirement_claimed_at, error_code,
       error_message, installed_at, created_at, updated_at
FROM local_model_installations
"#;

pub(super) const fn backend_str(backend: LocalModelBackend) -> &'static str {
    backend.as_persistence_str()
}

fn parse_backend(value: &str) -> Result<LocalModelBackend, StoreError> {
    value.parse().map_err(|_| StoreError::InvalidEnum {
        kind: "local model backend",
        value: value.to_string(),
    })
}

pub(super) type RawInstallation = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    f64,
    Option<i64>,
    i64,
    Option<String>,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
);

pub(super) fn raw_installation_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawInstallation> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
        row.get(20)?,
        row.get(21)?,
        row.get(22)?,
        row.get(23)?,
    ))
}

pub(super) fn installation_from_raw(
    raw: RawInstallation,
) -> Result<LocalModelInstallationRecord, StoreError> {
    Ok(LocalModelInstallationRecord {
        installation_id: raw.0,
        provider_instance_key: raw.1.parse::<ProviderInstanceKey>().map_err(|_| {
            StoreError::InvariantViolation {
                message: "local-model installation has an invalid provider instance key"
                    .to_string(),
            }
        })?,
        model_id: raw.2,
        display_name: raw.3,
        source_kind: raw.4.parse().map_err(|_| StoreError::InvalidEnum {
            kind: "local model source kind",
            value: raw.4.clone(),
        })?,
        source_repo: raw.5,
        source_revision: raw.6,
        source_file: raw.7,
        sha256: raw.8,
        download_gb: raw.9,
        expected_bytes: optional_i64_to_u64(raw.10, "expected bytes")?,
        downloaded_bytes: i64_to_u64(raw.11, "downloaded bytes")?,
        license: raw.12,
        backend: parse_backend(&raw.13)?,
        status: raw.14.parse().map_err(|_| StoreError::InvalidEnum {
            kind: "local model installation status",
            value: raw.14.clone(),
        })?,
        blob_relative_path: raw.15,
        is_active: raw.16,
        runtime_retired_at: raw.17,
        retirement_claimed_at: raw.18,
        error_code: raw.19,
        error_message: raw.20,
        installed_at: raw.21,
        created_at: raw.22,
        updated_at: raw.23,
    })
}

pub(super) type RawEvent = (
    i64,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<String>,
    String,
);

pub(super) fn raw_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawEvent> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

pub(super) fn event_from_raw(raw: RawEvent) -> Result<LocalModelEventRecord, StoreError> {
    Ok(LocalModelEventRecord {
        cursor: i64_to_u64(raw.0, "event cursor")?,
        installation_id: raw.1,
        kind: raw.2.parse().map_err(|_| StoreError::InvalidEnum {
            kind: "local model event kind",
            value: raw.2.clone(),
        })?,
        downloaded_bytes: optional_i64_to_u64(raw.3, "downloaded bytes")?,
        expected_bytes: optional_i64_to_u64(raw.4, "expected bytes")?,
        message: raw.5,
        created_at: raw.6,
    })
}

pub(super) fn u64_to_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidLocalModelRequest { kind: field })
}

pub(super) fn optional_u64_to_i64(
    value: Option<u64>,
    field: &'static str,
) -> Result<Option<i64>, StoreError> {
    value.map(|value| u64_to_i64(value, field)).transpose()
}

fn i64_to_u64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::InvariantViolation {
        message: format!("local-model {field} is negative"),
    })
}

fn optional_i64_to_u64(value: Option<i64>, field: &'static str) -> Result<Option<u64>, StoreError> {
    value.map(|value| i64_to_u64(value, field)).transpose()
}
