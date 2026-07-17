use std::collections::BTreeSet;

use noema_capabilities_mcp::{
    McpCalibrationStatus, McpRepositoryResult, NewToolCalibration, ToolCalibrationRecord,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
    conflict_error, not_found_error, recompute_server_enabled, repo_sql_error,
    rows::calibration_on_connection,
};

pub(super) fn save_calibrations_on_connection(
    connection: &mut Connection,
    calibrations: Vec<NewToolCalibration>,
) -> McpRepositoryResult<Vec<ToolCalibrationRecord>> {
    reject_duplicate_calibrations(&calibrations)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let mut server_ids = BTreeSet::new();
    for calibration in &calibrations {
        server_ids.insert(validate_calibration(&transaction, calibration)?);
    }

    let mut saved = Vec::with_capacity(calibrations.len());
    for calibration in &calibrations {
        write_calibration(&transaction, calibration)?;
        let record =
            calibration_on_connection(&transaction, "mcp_tool_id", &calibration.mcp_tool_id)?
                .ok_or_else(super::invariant_error)?;
        saved.push(record);
    }
    for server_id in server_ids {
        recompute_server_enabled(&transaction, &server_id)?;
    }
    transaction.commit().map_err(repo_sql_error)?;
    Ok(saved)
}

fn validate_calibration(
    connection: &Connection,
    calibration: &NewToolCalibration,
) -> McpRepositoryResult<String> {
    let tool = connection
        .query_row(
            r#"
            SELECT mcp_server_id, metadata_fingerprint
            FROM mcp_tools
            WHERE mcp_tool_id = ?1
            LIMIT 1
            "#,
            params![calibration.mcp_tool_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(repo_sql_error)?
        .ok_or_else(not_found_error)?;

    if let Some(existing) =
        calibration_on_connection(connection, "calibration_id", &calibration.calibration_id)?
        && existing.mcp_tool_id != calibration.mcp_tool_id
    {
        return Err(conflict_error());
    }
    if let Some(existing) =
        calibration_on_connection(connection, "mcp_tool_id", &calibration.mcp_tool_id)?
        && existing.calibration_id != calibration.calibration_id
    {
        return Err(conflict_error());
    }

    if calibration.status.requires_reviewed_metadata() {
        if calibration
            .reviewed_by
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
        {
            return Err(conflict_error());
        }
        if calibration.reviewed_metadata_fingerprint.as_deref() != Some(tool.1.as_str()) {
            return Err(conflict_error());
        }
        if calibration.status == McpCalibrationStatus::Ready
            && (!calibration.has_enabled_classification() || calibration.has_mixed_classification())
        {
            return Err(conflict_error());
        }
    }
    Ok(tool.0)
}

fn write_calibration(
    connection: &Connection,
    calibration: &NewToolCalibration,
) -> McpRepositoryResult<()> {
    connection
        .execute(
            r#"
            INSERT INTO tool_calibrations (
              calibration_id, mcp_tool_id, read_classification,
              write_classification, export_classification, status, reviewed_by,
              reviewed_metadata_fingerprint, updated_at
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
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
        )
        .map_err(repo_sql_error)?;
    Ok(())
}

fn reject_duplicate_calibrations(calibrations: &[NewToolCalibration]) -> McpRepositoryResult<()> {
    let mut calibration_ids = BTreeSet::new();
    let mut tool_ids = BTreeSet::new();
    for calibration in calibrations {
        if !calibration_ids.insert(calibration.calibration_id.as_str())
            || !tool_ids.insert(calibration.mcp_tool_id.as_str())
        {
            return Err(conflict_error());
        }
    }
    Ok(())
}
