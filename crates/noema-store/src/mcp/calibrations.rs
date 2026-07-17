use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{NewToolCalibration, ToolCalibrationRecord, rows::tool_calibration_from_row};
use noema_capabilities_mcp::McpCalibrationStatus;

use crate::{NoemaStore, StoreError, sqlite::now_timestamp_sql};

impl NoemaStore {
    /// Save reviewed calibration for one MCP tool.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails, or
    /// when a stored enum is invalid.
    pub async fn save_tool_calibration(
        &self,
        calibration: NewToolCalibration,
    ) -> Result<ToolCalibrationRecord, StoreError> {
        self.save_tool_calibrations(vec![calibration])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| StoreError::Schema("missing tool calibration after save".to_string()))
    }

    /// Save reviewed calibrations for multiple MCP tools after validating the full batch.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when any calibration is invalid, when the embedded
    /// store write or read fails, or when a stored enum is invalid. No
    /// calibration is written when batch validation fails.
    pub async fn save_tool_calibrations(
        &self,
        calibrations: Vec<NewToolCalibration>,
    ) -> Result<Vec<ToolCalibrationRecord>, StoreError> {
        reject_duplicate_calibrations_in_batch(&calibrations)?;
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let mut server_ids = BTreeSet::new();
            for calibration in &calibrations {
                let mcp_server_id =
                    validate_tool_calibration_on_connection(&transaction, calibration)?;
                server_ids.insert(mcp_server_id);
            }

            let mut saved = Vec::with_capacity(calibrations.len());
            for calibration in &calibrations {
                write_tool_calibration_row(&transaction, calibration)?;
                let saved_calibration = get_tool_calibration_on_connection(
                    &transaction,
                    TOOL_CALIBRATION_SELECT_BY_TOOL_ID,
                    &calibration.mcp_tool_id,
                )?
                .ok_or_else(|| {
                    StoreError::Schema(format!(
                        "missing tool calibration after save: {}",
                        calibration.mcp_tool_id
                    ))
                })?;
                saved.push(saved_calibration);
            }

            for server_id in server_ids {
                update_mcp_server_enabled_from_calibrations_on_connection(
                    &transaction,
                    &server_id,
                )?;
            }
            transaction.commit()?;
            Ok(saved)
        })
        .await
    }

    /// Return reviewed calibration for one MCP tool id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_tool_calibration(
        &self,
        mcp_tool_id: &str,
    ) -> Result<Option<ToolCalibrationRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                TOOL_CALIBRATION_SELECT_BY_TOOL_ID,
                params![mcp_tool_id],
                tool_calibration_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }
}

pub(super) fn invalidate_tool_calibration_review_on_connection(
    conn: &Connection,
    mcp_tool_id: &str,
) -> Result<(), StoreError> {
    conn.execute(
        format!(
            r#"
            UPDATE tool_calibrations SET
              status = 'needs_review',
              reviewed_by = NULL,
              reviewed_metadata_fingerprint = NULL,
              updated_at = {}
            WHERE mcp_tool_id = ?1
            "#,
            now_timestamp_sql()
        )
        .as_str(),
        params![mcp_tool_id],
    )?;
    Ok(())
}

pub(super) fn update_mcp_server_enabled_from_calibrations_on_connection(
    conn: &Connection,
    mcp_server_id: &str,
) -> Result<(), StoreError> {
    let enabled = conn.query_row(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM mcp_tools t
          JOIN tool_calibrations c ON c.mcp_tool_id = t.mcp_tool_id
          WHERE t.mcp_server_id = ?1
            AND c.status = 'ready'
            AND c.read_classification IN ('trusted', 'untrusted')
            AND c.write_classification = 'none'
            AND c.export_classification = 'none'
            AND c.reviewed_metadata_fingerprint = t.metadata_fingerprint
          LIMIT 1
        )
        "#,
        params![mcp_server_id],
        |row| row.get::<_, i64>(0),
    )? != 0;
    conn.execute(
        format!(
            r#"
            UPDATE mcp_servers SET
              enabled = ?2,
              updated_at = {}
            WHERE mcp_server_id = ?1
            "#,
            now_timestamp_sql()
        )
        .as_str(),
        params![mcp_server_id, super::rows::bool_to_i64(enabled)],
    )?;
    Ok(())
}

fn write_tool_calibration_row(
    conn: &Connection,
    calibration: &NewToolCalibration,
) -> Result<(), StoreError> {
    conn.execute(
        format!(
            r#"
            INSERT INTO tool_calibrations (
              calibration_id, mcp_tool_id, read_classification,
              write_classification, export_classification, status, reviewed_by,
              reviewed_metadata_fingerprint, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, {})
            ON CONFLICT(calibration_id) DO UPDATE SET
              mcp_tool_id = excluded.mcp_tool_id,
              read_classification = excluded.read_classification,
              write_classification = excluded.write_classification,
              export_classification = excluded.export_classification,
              status = excluded.status,
              reviewed_by = excluded.reviewed_by,
              reviewed_metadata_fingerprint = excluded.reviewed_metadata_fingerprint,
              updated_at = excluded.updated_at
            "#,
            now_timestamp_sql()
        )
        .as_str(),
        params![
            calibration.calibration_id,
            calibration.mcp_tool_id,
            calibration.read_classification.as_str(),
            calibration.write_classification.as_str(),
            calibration.export_classification.as_str(),
            calibration.status.as_str(),
            calibration.reviewed_by,
            calibration.reviewed_metadata_fingerprint,
        ],
    )?;
    Ok(())
}

fn validate_tool_calibration_on_connection(
    conn: &Connection,
    calibration: &NewToolCalibration,
) -> Result<String, StoreError> {
    let tool = conn
        .query_row(
            "SELECT mcp_server_id, metadata_fingerprint FROM mcp_tools WHERE mcp_tool_id = ?1 LIMIT 1",
            params![calibration.mcp_tool_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| {
            StoreError::Schema(format!(
                "cannot calibrate missing MCP tool: {}",
                calibration.mcp_tool_id
            ))
        })?;

    if let Some(existing) = get_tool_calibration_on_connection(
        conn,
        TOOL_CALIBRATION_SELECT_BY_CALIBRATION_ID,
        &calibration.calibration_id,
    )? && existing.mcp_tool_id != calibration.mcp_tool_id
    {
        return Err(StoreError::Schema(format!(
            "tool calibration {} already belongs to {}",
            calibration.calibration_id, existing.mcp_tool_id
        )));
    }

    if let Some(existing) = get_tool_calibration_on_connection(
        conn,
        TOOL_CALIBRATION_SELECT_BY_TOOL_ID,
        &calibration.mcp_tool_id,
    )? && existing.calibration_id != calibration.calibration_id
    {
        return Err(StoreError::Schema(format!(
            "MCP tool {} already has calibration {}",
            calibration.mcp_tool_id, existing.calibration_id
        )));
    }

    if calibration.status.requires_reviewed_metadata() {
        calibration
            .reviewed_by
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                StoreError::Schema("reviewed MCP tool calibration requires reviewed_by".to_string())
            })?;
        let reviewed_fingerprint = calibration
            .reviewed_metadata_fingerprint
            .as_deref()
            .ok_or_else(|| {
                StoreError::Schema(
                    "reviewed MCP tool calibration requires reviewed_metadata_fingerprint"
                        .to_string(),
                )
            })?;
        if reviewed_fingerprint != tool.1 {
            return Err(StoreError::Schema(format!(
                "reviewed metadata fingerprint does not match current MCP tool metadata for {}",
                calibration.mcp_tool_id
            )));
        }
        if calibration.status == McpCalibrationStatus::Ready
            && !calibration.has_enabled_classification()
        {
            return Err(StoreError::Schema(format!(
                "ready MCP tool calibration requires at least one non-none classification: {}",
                calibration.mcp_tool_id
            )));
        }
        if calibration.status == McpCalibrationStatus::Ready
            && calibration.has_mixed_classification()
        {
            return Err(StoreError::Schema(format!(
                "ready mixed MCP tool calibration is unsupported without ownership enforcement: {}",
                calibration.mcp_tool_id
            )));
        }
    }

    Ok(tool.0)
}

fn get_tool_calibration_on_connection(
    conn: &Connection,
    sql: &str,
    id: &str,
) -> Result<Option<ToolCalibrationRecord>, StoreError> {
    conn.query_row(sql, params![id], tool_calibration_from_row)
        .optional()
        .map_err(StoreError::Sqlite)
}

const TOOL_CALIBRATION_SELECT_BY_CALIBRATION_ID: &str = r#"
SELECT calibration_id, mcp_tool_id, read_classification, write_classification,
  export_classification, status, reviewed_by, reviewed_metadata_fingerprint
FROM tool_calibrations
WHERE calibration_id = ?1
LIMIT 1
"#;

const TOOL_CALIBRATION_SELECT_BY_TOOL_ID: &str = r#"
SELECT calibration_id, mcp_tool_id, read_classification, write_classification,
  export_classification, status, reviewed_by, reviewed_metadata_fingerprint
FROM tool_calibrations
WHERE mcp_tool_id = ?1
LIMIT 1
"#;

fn reject_duplicate_calibrations_in_batch(
    calibrations: &[NewToolCalibration],
) -> Result<(), StoreError> {
    let mut calibration_ids = BTreeSet::new();
    let mut tool_ids = BTreeSet::new();
    for calibration in calibrations {
        if !calibration_ids.insert(calibration.calibration_id.as_str()) {
            return Err(StoreError::Schema(format!(
                "duplicate calibration id in MCP calibration batch: {}",
                calibration.calibration_id
            )));
        }
        if !tool_ids.insert(calibration.mcp_tool_id.as_str()) {
            return Err(StoreError::Schema(format!(
                "duplicate MCP tool id in calibration batch: {}",
                calibration.mcp_tool_id
            )));
        }
    }
    Ok(())
}
