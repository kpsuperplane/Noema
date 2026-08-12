//! Deterministic connection-bound capability catalog compilation.

use crate::{ConnectionScan, DefinitionInstall, digest::canonical_json_bytes};
use noema_capabilities::{
    CapabilityAvailabilityNotice, CapabilityAvailabilityStatus, CapabilityBinding,
    CapabilityCatalogBuilder, CapabilityCatalogResult, CapabilityConnectionPolicy,
    CapabilityDestination, CapabilityScope, CapabilityTarget, CapabilityToolBehavior, InvokerKey,
    OperationToken, RedactingPayloadSanitizer, ToolName, ToolSpec,
    resolve_capability_execution_decision,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

const TOKEN_VERSION: u16 = 1;
const MAX_TOKEN_BYTES: usize = 1024;
pub(crate) const ADAPTER_INVOKER_KEY: &str = "adapter_json_v1";

/// Deterministic compiler from filesystem-validated connections to bindings.
#[derive(Debug, Default)]
pub struct AdapterCatalogCompiler;

/// Safe catalog compilation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("adapter connection catalog is invalid")]
pub struct AdapterCatalogError;

/// Exact non-secret authority retained inside one opaque operation token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterOperationAuthorityV1 {
    pub version: u16,
    pub canonical_name: String,
    pub connection_id: String,
    pub connection_slug: String,
    pub account_id: Option<String>,
    pub semantic_digest: String,
    pub operation_id: String,
    pub operation_digest: String,
    pub definition_token: String,
    pub connection_revision: u64,
    pub credential_revision: u64,
    pub grant_revision: u64,
    pub policy_revision: u64,
    pub credential_generation: Option<String>,
    pub account_kind: String,
}

impl AdapterCatalogCompiler {
    /// Compile exact active bindings and typed availability notices.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterCatalogError`] if validated filesystem authorities no
    /// longer compose uniquely or cannot produce bounded root contracts.
    pub fn compile(
        definitions: &[DefinitionInstall],
        connections: &ConnectionScan,
    ) -> Result<CapabilityCatalogResult, AdapterCatalogError> {
        let by_digest = definitions
            .iter()
            .map(|definition| {
                (
                    definition.compiled.semantic_digest.as_str(),
                    &definition.compiled,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut builder = CapabilityCatalogBuilder::new();
        let mut notices = Vec::new();
        for connection in &connections.connections {
            let descriptor = &connection.descriptor;
            let definition = by_digest
                .get(descriptor.semantic_digest.as_str())
                .ok_or(AdapterCatalogError)?;
            for operation_id in &descriptor.allowed_operations {
                let operation = definition
                    .operations
                    .iter()
                    .find(|operation| operation.operation_id == *operation_id)
                    .ok_or(AdapterCatalogError)?;
                let canonical_name = canonical_name(
                    &definition.adapter_id,
                    &descriptor.connection_slug,
                    operation_id,
                )?;
                match descriptor.status {
                    crate::AdapterConnectionStatus::Active => {
                        if !operation
                            .authorization
                            .is_satisfied_by(&descriptor.granted_scopes)
                        {
                            notices.push(CapabilityAvailabilityNotice {
                                capability: Some(canonical_name),
                                status: CapabilityAvailabilityStatus::AuthenticationRequired,
                            });
                            continue;
                        }
                        let Some(connection_policy) = descriptor.policy else {
                            notices.push(CapabilityAvailabilityNotice {
                                capability: Some(canonical_name),
                                status: CapabilityAvailabilityStatus::Disabled,
                            });
                            continue;
                        };
                        if connection_policy.revision != descriptor.revisions.policy {
                            return Err(AdapterCatalogError);
                        }
                        let behavior = effective_behavior(descriptor, operation)?;
                        builder
                            .add(binding(
                                canonical_name,
                                definition,
                                descriptor,
                                operation,
                                connection_policy,
                                behavior,
                            )?)
                            .map_err(|_| AdapterCatalogError)?;
                    }
                    crate::AdapterConnectionStatus::Suspended => {
                        notices.push(CapabilityAvailabilityNotice {
                            capability: Some(canonical_name),
                            status: CapabilityAvailabilityStatus::Disabled,
                        })
                    }
                    crate::AdapterConnectionStatus::AuthenticationRequired => {
                        notices.push(CapabilityAvailabilityNotice {
                            capability: Some(canonical_name),
                            status: CapabilityAvailabilityStatus::AuthenticationRequired,
                        })
                    }
                }
            }
        }
        Ok(CapabilityCatalogResult {
            snapshot: builder.build(),
            availability_notices: notices,
        })
    }
}

impl AdapterOperationAuthorityV1 {
    pub(crate) fn from_operation_token(
        token: &OperationToken,
    ) -> Result<Self, AdapterCatalogError> {
        let bytes = token.as_str().as_bytes();
        if bytes.len() > MAX_TOKEN_BYTES {
            return Err(AdapterCatalogError);
        }
        let authority: Self = serde_json::from_slice(bytes).map_err(|_| AdapterCatalogError)?;
        if authority.version != TOKEN_VERSION
            || canonical_json_bytes(
                &serde_json::to_value(&authority).map_err(|_| AdapterCatalogError)?,
            )
            .map_err(|_| AdapterCatalogError)?
                != bytes
            || authority.canonical_name.is_empty()
            || authority.connection_id.len() != 32
            || !authority
                .connection_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || authority.semantic_digest.len() != 64
            || authority.operation_digest.len() != 64
            || [
                authority.connection_revision,
                authority.grant_revision,
                authority.policy_revision,
            ]
            .contains(&0)
            || authority
                .credential_generation
                .as_deref()
                .is_some_and(|value| {
                    value.len() != 32
                        || !value
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                })
        {
            return Err(AdapterCatalogError);
        }
        Ok(authority)
    }

    pub(crate) fn destination_revision(&self) -> String {
        format!(
            "definition:{}/connection:{}/credential:{}/grant:{}/policy:{}",
            self.semantic_digest,
            self.connection_revision,
            self.credential_revision,
            self.grant_revision,
            self.policy_revision,
        )
    }

    fn authentication_revision(&self) -> String {
        format!(
            "definition:{}/operation:{}/policy:{}",
            self.semantic_digest, self.operation_digest, self.policy_revision,
        )
    }
}

fn binding(
    canonical_name: ToolName,
    definition: &crate::CompiledAdapterDefinition,
    descriptor: &crate::AdapterConnectionV3,
    operation: &crate::CompiledOperation,
    connection_policy: CapabilityConnectionPolicy,
    behavior: CapabilityToolBehavior,
) -> Result<CapabilityBinding, AdapterCatalogError> {
    let authority = AdapterOperationAuthorityV1 {
        version: TOKEN_VERSION,
        canonical_name: canonical_name.as_str().to_string(),
        connection_id: descriptor.connection_id.clone(),
        connection_slug: descriptor.connection_slug.clone(),
        account_id: descriptor.account_id.clone(),
        semantic_digest: descriptor.semantic_digest.clone(),
        operation_id: operation.operation_id.clone(),
        operation_digest: operation.operation_digest.to_string(),
        definition_token: operation.token.as_str().to_string(),
        connection_revision: descriptor.revisions.connection,
        credential_revision: descriptor.revisions.credential,
        grant_revision: descriptor.revisions.grant,
        policy_revision: descriptor.revisions.policy,
        credential_generation: descriptor.credential_generation.clone(),
        account_kind: descriptor.account_kind.clone(),
    };
    let token =
        canonical_json_bytes(&serde_json::to_value(&authority).map_err(|_| AdapterCatalogError)?)
            .map_err(|_| AdapterCatalogError)?;
    if token.len() > MAX_TOKEN_BYTES {
        return Err(AdapterCatalogError);
    }
    let token = String::from_utf8(token).map_err(|_| AdapterCatalogError)?;
    let destination_revision = authority.destination_revision();
    let destination = CapabilityDestination::new(
        "adapter",
        descriptor.connection_id.clone(),
        descriptor.account_id.clone(),
        destination_revision,
    )
    .and_then(|destination| {
        destination.with_authentication_revision(authority.authentication_revision())
    })
    .map_err(|_| AdapterCatalogError)?;
    let spec = ToolSpec::new(
        canonical_name.as_str(),
        operation.description.clone(),
        operation.input_schema.clone(),
    )
    .map_err(|_| AdapterCatalogError)?;
    let input_operation = operation.clone();
    let input_check: Arc<dyn noema_capabilities::ToolInputCheck> =
        Arc::new(move |arguments: &serde_json::Value| {
            crate::request::model_arguments_are_valid(&input_operation, arguments)
        });
    let mut service_context = noema_capabilities::CapabilityServiceContext::new(
        definition
            .display_name
            .clone()
            .unwrap_or_else(|| definition.adapter_id.clone()),
        None::<String>,
    )
    .map_err(|_| AdapterCatalogError)?;
    if let Some(connection_label) = &descriptor.connection_label {
        service_context = service_context
            .with_connection_label(connection_label.clone())
            .map_err(|_| AdapterCatalogError)?;
    }
    Ok(CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new(ADAPTER_INVOKER_KEY),
            OperationToken::new(token),
        ),
        behavior,
        resolve_capability_execution_decision(connection_policy, behavior),
        CapabilityScope::Global,
        input_check,
        Arc::new(RedactingPayloadSanitizer),
    )
    .with_destination(destination)
    .with_service_context(service_context))
}

pub(crate) fn effective_behavior(
    descriptor: &crate::AdapterConnectionV3,
    operation: &crate::CompiledOperation,
) -> Result<CapabilityToolBehavior, AdapterCatalogError> {
    let Some(override_policy) = descriptor
        .tool_overrides
        .iter()
        .find(|policy| policy.tool_id == operation.operation_id)
    else {
        return Ok(operation.behavior);
    };
    if override_policy.source_revision != operation.operation_digest.as_str() {
        return Err(AdapterCatalogError);
    }
    Ok(CapabilityToolBehavior {
        read_only: override_policy.read_only,
        idempotent: override_policy.idempotent,
        destructive: override_policy.destructive,
        open_world: override_policy.open_world,
    })
}

pub(crate) fn canonical_name(
    adapter_id: &str,
    connection_slug: &str,
    operation_id: &str,
) -> Result<ToolName, AdapterCatalogError> {
    ToolName::new(format!("{adapter_id}_{connection_slug}.{operation_id}"))
        .map_err(|_| AdapterCatalogError)
}

#[cfg(test)]
mod tests;
