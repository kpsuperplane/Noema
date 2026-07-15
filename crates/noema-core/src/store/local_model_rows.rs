//! SQLite row decoding and numeric conversion for local-model persistence.

use crate::local_models::{
    LocalModelBackend, LocalModelEventKind, LocalModelEventRecord, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelSourceKind,
};

use super::StoreError;

pub(super) const INSTALLATION_SELECT: &str = r#"
SELECT installation_id, model_id, display_name, source_kind, source_repo,
       source_revision, source_file, sha256, download_gb, expected_bytes,
       downloaded_bytes, license, backend, status, blob_relative_path, is_active,
       error_code, error_message, installed_at, created_at, updated_at
FROM local_model_installations
"#;

pub(super) const fn backend_str(backend: LocalModelBackend) -> &'static str {
    match backend {
        LocalModelBackend::Metal => "metal",
        LocalModelBackend::Cuda => "cuda",
        LocalModelBackend::Vulkan => "vulkan",
        LocalModelBackend::Cpu => "cpu",
    }
}

fn parse_backend(value: &str) -> Result<LocalModelBackend, StoreError> {
    match value {
        "metal" => Ok(LocalModelBackend::Metal),
        "cuda" => Ok(LocalModelBackend::Cuda),
        "vulkan" => Ok(LocalModelBackend::Vulkan),
        "cpu" => Ok(LocalModelBackend::Cpu),
        _ => Err(StoreError::InvalidEnum {
            kind: "local model backend",
            value: value.to_string(),
        }),
    }
}

pub(super) type RawInstallation = (
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
    ))
}

pub(super) fn installation_from_raw(
    raw: RawInstallation,
) -> Result<LocalModelInstallationRecord, StoreError> {
    Ok(LocalModelInstallationRecord {
        installation_id: raw.0,
        model_id: raw.1,
        display_name: raw.2,
        source_kind: LocalModelSourceKind::from_str(&raw.3).ok_or_else(|| {
            StoreError::InvalidEnum {
                kind: "local model source kind",
                value: raw.3,
            }
        })?,
        source_repo: raw.4,
        source_revision: raw.5,
        source_file: raw.6,
        sha256: raw.7,
        download_gb: raw.8,
        expected_bytes: optional_i64_to_u64(raw.9, "expected bytes")?,
        downloaded_bytes: i64_to_u64(raw.10, "downloaded bytes")?,
        license: raw.11,
        backend: parse_backend(&raw.12)?,
        status: LocalModelInstallationStatus::from_str(&raw.13).ok_or_else(|| {
            StoreError::InvalidEnum {
                kind: "local model installation status",
                value: raw.13,
            }
        })?,
        blob_relative_path: raw.14,
        is_active: raw.15,
        error_code: raw.16,
        error_message: raw.17,
        installed_at: raw.18,
        created_at: raw.19,
        updated_at: raw.20,
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
        kind: LocalModelEventKind::from_str(&raw.2).ok_or_else(|| StoreError::InvalidEnum {
            kind: "local model event kind",
            value: raw.2,
        })?,
        downloaded_bytes: optional_i64_to_u64(raw.3, "downloaded bytes")?,
        expected_bytes: optional_i64_to_u64(raw.4, "expected bytes")?,
        message: raw.5,
        created_at: raw.6,
    })
}

pub(super) fn u64_to_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvariantViolation {
        message: format!("local-model {field} exceeds SQLite integer range"),
    })
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
