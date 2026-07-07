use std::collections::BTreeSet;

use super::{
    NewToolCalibration, ToolCalibrationRecord, mcp_record_fragment,
    rows::{ToolCalibrationRow, tool_calibration_from_row},
};
use crate::{
    McpCalibrationStatus,
    store::{NoemaStore, StoreError},
};

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
        self.validate_tool_calibration(&calibration).await?;
        let mcp_tool_id = calibration.mcp_tool_id.clone();
        let saved = self.write_tool_calibration_row(&calibration).await?;
        if let Some(tool) = self.get_mcp_tool(&mcp_tool_id).await? {
            self.update_mcp_server_enabled_from_calibrations(&tool.mcp_server_id)
                .await?;
        }
        Ok(saved)
    }

    async fn write_tool_calibration_row(
        &self,
        calibration: &NewToolCalibration,
    ) -> Result<ToolCalibrationRecord, StoreError> {
        let mcp_tool_id = calibration.mcp_tool_id.clone();
        let owner_extractors =
            serde_json::to_value(&calibration.owner_extractors).map_err(|error| {
                StoreError::Schema(format!("invalid owner extractor serialization: {error}"))
            })?;
        self.db()
            .query(
                r#"
                UPSERT type::record('tool_calibrations', $record_id) SET
                  calibration_id = $calibration_id,
                  mcp_tool_id = $mcp_tool_id,
                  read_classification = $read_classification,
                  write_classification = $write_classification,
                  export_classification = $export_classification,
                  owner_extractors = $owner_extractors,
                  status = $status,
                  reviewed_by = $reviewed_by,
                  reviewed_metadata_fingerprint = $reviewed_metadata_fingerprint,
                  updated_at = time::now();
                "#,
            )
            .bind((
                "record_id",
                mcp_record_fragment(&calibration.calibration_id),
            ))
            .bind(("calibration_id", calibration.calibration_id.clone()))
            .bind(("mcp_tool_id", calibration.mcp_tool_id.clone()))
            .bind((
                "read_classification",
                calibration.read_classification.as_str().to_string(),
            ))
            .bind((
                "write_classification",
                calibration.write_classification.as_str().to_string(),
            ))
            .bind((
                "export_classification",
                calibration.export_classification.as_str().to_string(),
            ))
            .bind(("owner_extractors", owner_extractors))
            .bind(("status", calibration.status.as_str().to_string()))
            .bind(("reviewed_by", calibration.reviewed_by.clone()))
            .bind((
                "reviewed_metadata_fingerprint",
                calibration.reviewed_metadata_fingerprint.clone(),
            ))
            .await?
            .check()?;
        let saved = self
            .get_tool_calibration(&mcp_tool_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing tool calibration after save: {}",
                    mcp_tool_id
                ))
            })?;
        Ok(saved)
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
        for calibration in &calibrations {
            self.validate_tool_calibration(calibration).await?;
        }

        let mut server_ids = BTreeSet::new();
        for calibration in &calibrations {
            if let Some(tool) = self.get_mcp_tool(&calibration.mcp_tool_id).await? {
                server_ids.insert(tool.mcp_server_id);
            }
        }

        let mut saved = Vec::with_capacity(calibrations.len());
        for calibration in &calibrations {
            saved.push(self.write_tool_calibration_row(calibration).await?);
        }

        for server_id in server_ids {
            self.update_mcp_server_enabled_from_calibrations(&server_id)
                .await?;
        }
        Ok(saved)
    }

    async fn update_mcp_server_enabled_from_calibrations(
        &self,
        mcp_server_id: &str,
    ) -> Result<(), StoreError> {
        let tools = self.list_mcp_tools_for_server(mcp_server_id).await?;
        let mut enabled = false;
        for tool in tools {
            if self
                .get_tool_calibration(&tool.mcp_tool_id)
                .await?
                .is_some_and(|calibration| calibration.status == McpCalibrationStatus::Ready)
            {
                enabled = true;
                break;
            }
        }
        self.db()
            .query(
                r#"
                UPDATE mcp_servers SET
                  enabled = $enabled,
                  updated_at = time::now()
                WHERE mcp_server_id = $mcp_server_id;
                "#,
            )
            .bind(("mcp_server_id", mcp_server_id.to_string()))
            .bind(("enabled", enabled))
            .await?
            .check()?;
        Ok(())
    }

    async fn validate_tool_calibration(
        &self,
        calibration: &NewToolCalibration,
    ) -> Result<(), StoreError> {
        let tool = self
            .get_mcp_tool(&calibration.mcp_tool_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "cannot calibrate missing MCP tool: {}",
                    calibration.mcp_tool_id
                ))
            })?;

        if let Some(existing) = self
            .get_tool_calibration_by_calibration_id(&calibration.calibration_id)
            .await?
            && existing.mcp_tool_id != calibration.mcp_tool_id
        {
            return Err(StoreError::Schema(format!(
                "tool calibration {} already belongs to {}",
                calibration.calibration_id, existing.mcp_tool_id
            )));
        }

        if let Some(existing) = self.get_tool_calibration(&calibration.mcp_tool_id).await?
            && existing.calibration_id != calibration.calibration_id
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
                    StoreError::Schema(
                        "reviewed MCP tool calibration requires reviewed_by".to_string(),
                    )
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
            if reviewed_fingerprint != tool.metadata_fingerprint {
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
                && calibration.owner_extractors.is_empty()
            {
                return Err(StoreError::Schema(format!(
                    "ready mixed MCP tool calibration requires an owner extractor: {}",
                    calibration.mcp_tool_id
                )));
            }
        }

        Ok(())
    }

    async fn get_tool_calibration_by_calibration_id(
        &self,
        calibration_id: &str,
    ) -> Result<Option<ToolCalibrationRecord>, StoreError> {
        let mut response = self
            .db()
            .query(
                r#"
                SELECT calibration_id, mcp_tool_id, read_classification,
                  write_classification, export_classification, owner_extractors,
                  status, reviewed_by, reviewed_metadata_fingerprint
                FROM tool_calibrations
                WHERE calibration_id = $calibration_id
                LIMIT 1;
                "#,
            )
            .bind(("calibration_id", calibration_id.to_string()))
            .await?;
        let rows: Vec<ToolCalibrationRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(tool_calibration_from_row)
            .transpose()
    }

    pub(super) async fn invalidate_tool_calibration_review(
        &self,
        mcp_tool_id: &str,
    ) -> Result<(), StoreError> {
        self.db()
            .query(
                r#"
                UPDATE tool_calibrations SET
                  status = 'needs_review',
                  reviewed_by = NONE,
                  reviewed_metadata_fingerprint = NONE,
                  updated_at = time::now()
                WHERE mcp_tool_id = $mcp_tool_id;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?
            .check()?;
        Ok(())
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
        let mut response = self
            .db()
            .query(
                r#"
                SELECT calibration_id, mcp_tool_id, read_classification,
                  write_classification, export_classification, owner_extractors,
                  status, reviewed_by, reviewed_metadata_fingerprint
                FROM tool_calibrations
                WHERE mcp_tool_id = $mcp_tool_id
                LIMIT 1;
                "#,
            )
            .bind(("mcp_tool_id", mcp_tool_id.to_string()))
            .await?;
        let rows: Vec<ToolCalibrationRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(tool_calibration_from_row)
            .transpose()
    }
}

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
