//! Migration-only readers for filesystem-canonical v1/v2 definitions.

use crate::{
    AccountGate, AdapterManifestV3, AdapterOperation, AdapterOperationBehavior, ArgumentDefinition,
    AuthenticationRequirement, EventMetadata, HttpMethod, PaginationPolicy, QuotaPolicy,
    RetryPolicy,
};
use noema_capabilities::{CapabilityToolHint, CapabilityToolHintSource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyAdapterManifestV1 {
    pub schema_version: u16,
    pub definition_id: String,
    pub adapter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub definition_revision: String,
    pub reviewed: bool,
    pub origin: String,
    pub authentication: AuthenticationRequirement,
    #[serde(default)]
    pub gates: Vec<AccountGate>,
    pub provider_data_policy: LegacyProviderDataPolicy,
    pub quota: QuotaPolicy,
    pub operations: Vec<LegacyAdapterOperation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyProviderDataPolicy {
    pub retention_allowed: bool,
    pub deletion_supported: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyAdapterOperation {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
    pub method: HttpMethod,
    pub path: String,
    #[serde(default)]
    pub fixed_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub arguments: Vec<ArgumentDefinition>,
    pub effect: LegacyOperationEffect,
    pub admission: LegacyAdmissionMode,
    pub result: LegacyResultDefinition,
    pub retry: RetryPolicy,
    pub pagination: PaginationPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventMetadata>,
    #[serde(default)]
    pub gates: Vec<AccountGate>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyResultDefinition {
    pub classification: LegacyResultClassification,
    pub model_route: LegacyModelRoute,
    pub model_payload: LegacyModelPayload,
    pub provider_retention: LegacyProviderRetention,
    pub persistence: LegacyPersistenceMode,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyResultClassification {
    Public,
    Private,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyModelRoute {
    AnyKnownRoute,
    LocalOnly,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyModelPayload {
    Full,
    MetadataOnly,
    Omit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyProviderRetention {
    Allow,
    Deny,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyPersistenceMode {
    Redacted,
    MetadataOnly,
    Omit,
}

impl LegacyAdapterManifestV1 {
    pub(crate) fn into_v3(self) -> AdapterManifestV3 {
        AdapterManifestV3 {
            schema_version: 3,
            definition_id: self.definition_id,
            adapter_id: self.adapter_id,
            display_name: self.display_name,
            definition_revision: self.definition_revision,
            reviewed: self.reviewed,
            origin: self.origin,
            authentication: self.authentication,
            gates: self.gates,
            quota: self.quota,
            operations: self
                .operations
                .into_iter()
                .map(LegacyAdapterOperation::into_v3)
                .collect(),
        }
    }
}

impl LegacyAdapterOperation {
    fn into_v3(self) -> AdapterOperation {
        AdapterOperation {
            operation_id: self.operation_id,
            source_description: self.source_description,
            method: self.method,
            path: self.path,
            fixed_headers: self.fixed_headers,
            arguments: self.arguments,
            behavior: behavior_from_effect(self.effect),
            retry: self.retry,
            pagination: self.pagination,
            event: self.event,
            gates: self.gates,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyOperationEffect {
    ReadOnly,
    ExternalWrite,
    ExternalExport,
    ExternalWriteAndExport,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyAdmissionMode {
    Direct,
    ReviewerMayApprove,
    AlwaysAsk,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyAdapterManifestV2 {
    pub schema_version: u16,
    pub definition_id: String,
    pub adapter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub definition_revision: String,
    pub reviewed: bool,
    pub origin: String,
    pub authentication: AuthenticationRequirement,
    #[serde(default)]
    pub gates: Vec<AccountGate>,
    pub quota: QuotaPolicy,
    pub operations: Vec<LegacyAdapterOperationV2>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyAdapterOperationV2 {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
    pub method: HttpMethod,
    pub path: String,
    #[serde(default)]
    pub fixed_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub arguments: Vec<ArgumentDefinition>,
    pub effect: LegacyOperationEffect,
    pub admission: LegacyAdmissionMode,
    pub retry: RetryPolicy,
    pub pagination: PaginationPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<EventMetadata>,
    #[serde(default)]
    pub gates: Vec<AccountGate>,
}

impl LegacyAdapterManifestV2 {
    pub(crate) fn into_v3(self) -> AdapterManifestV3 {
        AdapterManifestV3 {
            schema_version: 3,
            definition_id: self.definition_id,
            adapter_id: self.adapter_id,
            display_name: self.display_name,
            definition_revision: self.definition_revision,
            reviewed: self.reviewed,
            origin: self.origin,
            authentication: self.authentication,
            gates: self.gates,
            quota: self.quota,
            operations: self
                .operations
                .into_iter()
                .map(|operation| AdapterOperation {
                    operation_id: operation.operation_id,
                    source_description: operation.source_description,
                    method: operation.method,
                    path: operation.path,
                    fixed_headers: operation.fixed_headers,
                    arguments: operation.arguments,
                    behavior: behavior_from_effect(operation.effect),
                    retry: operation.retry,
                    pagination: operation.pagination,
                    event: operation.event,
                    gates: operation.gates,
                })
                .collect(),
        }
    }
}

fn behavior_from_effect(effect: LegacyOperationEffect) -> AdapterOperationBehavior {
    let values = match effect {
        LegacyOperationEffect::ReadOnly | LegacyOperationEffect::ExternalExport => {
            [true, true, false, true]
        }
        LegacyOperationEffect::ExternalWrite | LegacyOperationEffect::ExternalWriteAndExport => {
            [false, false, true, true]
        }
    };
    let hint = |value| CapabilityToolHint {
        value: Some(value),
        source: Some(CapabilityToolHintSource::SafeDefault),
    };
    AdapterOperationBehavior {
        read_only: hint(values[0]),
        idempotent: hint(values[1]),
        destructive: hint(values[2]),
        open_world: hint(values[3]),
    }
}
