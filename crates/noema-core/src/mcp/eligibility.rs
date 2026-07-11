//! Shared MCP tool eligibility and prompt-safety helpers.

use crate::{
    McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, McpTrustClassification, ToolCalibrationRecord,
};

/// Reason a discovered MCP tool must not be exposed to or executed for the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpToolIneligibility {
    /// The owning server is disabled.
    ServerDisabled,
    /// The owning server is not currently healthy.
    ServerUnhealthy,
    /// The owning server requires authentication before use.
    ServerAuthRequired,
    /// The tool is missing a current ready calibration.
    ToolNotCalibrated,
    /// The tool can write or export and has no one-shot approval for this dispatch.
    ToolApprovalRequired,
}

impl McpToolIneligibility {
    /// Stable gateway error code released to the provider-visible tool result.
    #[must_use]
    pub const fn gateway_error(self) -> &'static str {
        match self {
            Self::ServerDisabled => "mcp_server_disabled",
            Self::ServerUnhealthy => "mcp_server_unhealthy",
            Self::ServerAuthRequired => "mcp_server_auth_required",
            Self::ToolNotCalibrated => "mcp_tool_not_calibrated",
            Self::ToolApprovalRequired => "mcp_tool_approval_required",
        }
    }
}

/// Return why a tool is not eligible for model exposure or gateway execution.
#[must_use]
pub fn mcp_tool_ineligibility(
    server: &McpServerRecord,
    tool: &McpToolRecord,
    calibration: Option<&ToolCalibrationRecord>,
) -> Option<McpToolIneligibility> {
    if !server.enabled {
        return Some(McpToolIneligibility::ServerDisabled);
    }
    if server.health_status != McpServerHealthStatus::Healthy {
        return Some(McpToolIneligibility::ServerUnhealthy);
    }
    if !matches!(
        server.auth_status,
        McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
    ) {
        return Some(McpToolIneligibility::ServerAuthRequired);
    }

    let Some(calibration) = calibration else {
        return Some(McpToolIneligibility::ToolNotCalibrated);
    };
    if calibration.status != McpCalibrationStatus::Ready
        || [
            calibration.read_classification,
            calibration.write_classification,
            calibration.export_classification,
        ]
        .contains(&McpTrustClassification::Mixed)
        || calibration.reviewed_metadata_fingerprint.as_deref()
            != Some(tool.metadata_fingerprint.as_str())
    {
        return Some(McpToolIneligibility::ToolNotCalibrated);
    }
    if calibration.write_classification != McpTrustClassification::None
        || calibration.export_classification != McpTrustClassification::None
    {
        return Some(McpToolIneligibility::ToolApprovalRequired);
    }

    None
}

/// Return a bounded, prompt-safe one-line MCP tool description.
#[must_use]
pub fn prompt_safe_mcp_tool_description(
    description: Option<&str>,
    max_chars: usize,
) -> Option<String> {
    let description = description?;
    let without_examples = description
        .split_once("<example")
        .map_or(description, |(before_examples, _)| before_examples);
    let candidate = without_examples
        .lines()
        .map(sanitize_prompt_line)
        .find(|line| !line.is_empty() && !looks_like_prompt_directive(line))?;
    let first_sentence = candidate
        .split_once(". ")
        .map_or(candidate.as_str(), |(sentence, _)| sentence);
    let mut hint = first_sentence.trim().to_string();
    if candidate.len() > hint.len() && !hint.ends_with('.') {
        hint.push('.');
    }

    Some(truncate_chars(&hint, max_chars))
}

pub(crate) fn sanitize_prompt_line(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_prompt_directive(value: &str) -> bool {
    let lower = value.trim_start().to_ascii_lowercase();
    [
        "system:",
        "developer:",
        "assistant:",
        "user:",
        "instruction:",
        "instructions:",
        "ignore ",
        "ignore:",
        "disregard ",
        "disregard:",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

pub(crate) fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_write_or_export_tool_requires_one_shot_approval() {
        let server = McpServerRecord {
            mcp_server_id: "mcp:docs".to_string(),
            display_name: "Docs".to_string(),
            transport_kind: crate::McpTransportKind::Stdio,
            safe_config: serde_json::json!({}),
            enabled: true,
            health_status: McpServerHealthStatus::Healthy,
            auth_status: McpServerAuthStatus::Authenticated,
            tool_count: 1,
        };
        let tool = McpToolRecord {
            mcp_tool_id: "mcp_tool:docs:read".to_string(),
            mcp_server_id: "mcp:docs".to_string(),
            name: "read".to_string(),
            description: None,
            input_schema: serde_json::json!({}),
            output_schema: None,
            annotations: serde_json::json!({}),
            metadata_fingerprint: "fp1".to_string(),
            discovered_at: "now".to_string(),
        };
        for (write_classification, export_classification) in [
            (
                McpTrustClassification::Trusted,
                McpTrustClassification::None,
            ),
            (
                McpTrustClassification::None,
                McpTrustClassification::Untrusted,
            ),
        ] {
            let calibration = ToolCalibrationRecord {
                calibration_id: "cal1".to_string(),
                mcp_tool_id: tool.mcp_tool_id.clone(),
                read_classification: McpTrustClassification::Trusted,
                write_classification,
                export_classification,
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:local".to_string()),
                reviewed_metadata_fingerprint: Some("fp1".to_string()),
            };
            let reason = mcp_tool_ineligibility(&server, &tool, Some(&calibration))
                .expect("write/export tool must require approval");
            assert_eq!(reason, McpToolIneligibility::ToolApprovalRequired);
            assert_eq!(reason.gateway_error(), "mcp_tool_approval_required");
        }

        let calibration = ToolCalibrationRecord {
            calibration_id: "cal1".to_string(),
            mcp_tool_id: tool.mcp_tool_id.clone(),
            read_classification: McpTrustClassification::Trusted,
            write_classification: McpTrustClassification::Trusted,
            export_classification: McpTrustClassification::None,
            status: McpCalibrationStatus::Ready,
            reviewed_by: Some("human:local".to_string()),
            reviewed_metadata_fingerprint: Some("stale".to_string()),
        };
        assert_eq!(
            mcp_tool_ineligibility(&server, &tool, Some(&calibration)),
            Some(McpToolIneligibility::ToolNotCalibrated)
        );
        let mut disabled_server = server;
        disabled_server.enabled = false;
        assert_eq!(
            mcp_tool_ineligibility(&disabled_server, &tool, Some(&calibration)),
            Some(McpToolIneligibility::ServerDisabled)
        );
    }
}
