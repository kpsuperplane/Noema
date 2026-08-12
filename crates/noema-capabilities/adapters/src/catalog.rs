//! Deterministic connection-bound capability catalog compilation.

use crate::{
    AdapterConnectionAuthenticationV1, AuthorizationGrantStatus, ConnectionScan, DefinitionInstall,
    OauthAuthoritySnapshot, digest::canonical_json_bytes,
};
use noema_capabilities::{
    CapabilityAvailabilityNotice, CapabilityAvailabilityStatus, CapabilityBinding,
    CapabilityCatalogBuilder, CapabilityCatalogResult, CapabilityConnectionPolicy,
    CapabilityDestination, CapabilityExecutionDecision, CapabilityScope, CapabilityTarget,
    CapabilityToolBehavior, InvokerKey, OperationToken, RedactingPayloadSanitizer, ToolName,
    ToolSpec, resolve_capability_execution_decision, tool_enablement_name,
};
use serde::{Deserialize, Serialize};
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
    pub credential_revision: Option<u64>,
    pub grant_id: Option<String>,
    pub grant_authority_revision: Option<u64>,
    pub policy_revision: u64,
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
        oauth: &OauthAuthoritySnapshot,
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
            let authentication = resolved_authentication(descriptor, definition, oauth)?;
            for operation in &definition.operations {
                let enabled = descriptor
                    .allowed_operations
                    .contains(&operation.operation_id);
                let canonical_name = canonical_name(
                    &definition.adapter_id,
                    &descriptor.connection_slug,
                    &operation.operation_id,
                )?;
                match descriptor.status {
                    crate::AdapterConnectionStatus::Active => {
                        if !authentication.active
                            || !operation
                                .authorization
                                .is_satisfied_by(authentication.granted_scopes)
                        {
                            if enabled {
                                notices.push(CapabilityAvailabilityNotice {
                                    capability: Some(canonical_name),
                                    status: CapabilityAvailabilityStatus::AuthenticationRequired,
                                });
                            }
                            continue;
                        }
                        let Some(connection_policy) = descriptor.policy else {
                            if enabled {
                                notices.push(CapabilityAvailabilityNotice {
                                    capability: Some(canonical_name),
                                    status: CapabilityAvailabilityStatus::Disabled,
                                });
                            }
                            continue;
                        };
                        if connection_policy.revision != descriptor.policy_revision {
                            return Err(AdapterCatalogError);
                        }
                        let behavior = effective_behavior(descriptor, operation)?;
                        let operation_binding = binding(
                            canonical_name.clone(),
                            definition,
                            descriptor,
                            operation,
                            &authentication,
                            connection_policy,
                            behavior,
                        )?;
                        if enabled {
                            builder
                                .add(operation_binding)
                                .map_err(|_| AdapterCatalogError)?;
                        } else {
                            notices.push(CapabilityAvailabilityNotice {
                                capability: Some(canonical_name),
                                status: CapabilityAvailabilityStatus::Disabled,
                            });
                            builder
                                .add(enablement_binding(&operation_binding)?)
                                .map_err(|_| AdapterCatalogError)?;
                        }
                    }
                    crate::AdapterConnectionStatus::Suspended => {
                        if enabled {
                            notices.push(CapabilityAvailabilityNotice {
                                capability: Some(canonical_name),
                                status: CapabilityAvailabilityStatus::Disabled,
                            });
                        }
                    }
                    crate::AdapterConnectionStatus::AuthenticationRequired => {
                        if enabled {
                            notices.push(CapabilityAvailabilityNotice {
                                capability: Some(canonical_name),
                                status: CapabilityAvailabilityStatus::AuthenticationRequired,
                            });
                        }
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

fn enablement_binding(
    disabled: &CapabilityBinding,
) -> Result<CapabilityBinding, AdapterCatalogError> {
    let disabled_name = &disabled.spec().name;
    let spec = ToolSpec::new(
        tool_enablement_name(disabled_name)
            .map_err(|_| AdapterCatalogError)?
            .as_str(),
        format!(
            "Ask the human to enable the disabled {} tool. Use this only when that tool is required for the current request.",
            disabled_name
        ),
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        }),
    )
    .map_err(|_| AdapterCatalogError)?;
    let mut binding = CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new(ADAPTER_INVOKER_KEY),
            disabled.target().operation_token().clone(),
        ),
        CapabilityToolBehavior {
            read_only: false,
            idempotent: true,
            destructive: false,
            open_world: false,
        },
        CapabilityExecutionDecision::HumanReview,
        CapabilityScope::Global,
        Arc::new(|arguments: &serde_json::Value| {
            arguments.as_object().is_some_and(serde_json::Map::is_empty)
        }),
        Arc::new(RedactingPayloadSanitizer),
    )
    .with_destination(disabled.destination().cloned().ok_or(AdapterCatalogError)?);
    if let Some(context) = disabled.service_context() {
        binding = binding.with_service_context(context.clone());
    }
    Ok(binding)
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
            || [authority.connection_revision, authority.policy_revision].contains(&0)
            || authority.grant_id.as_deref().is_some_and(|value| {
                value.len() != 32
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            })
            || authority
                .grant_authority_revision
                .is_some_and(|revision| revision == 0)
            || authority
                .credential_revision
                .is_some_and(|revision| revision == 0)
            || authority.grant_id.is_some() != authority.grant_authority_revision.is_some()
            || authority.grant_id.is_some() && authority.credential_revision.is_some()
        {
            return Err(AdapterCatalogError);
        }
        Ok(authority)
    }

    pub(crate) fn destination_revision(&self) -> String {
        let credential_revision = self
            .credential_revision
            .map_or_else(|| "none".to_string(), |revision| revision.to_string());
        let grant_revision = self
            .grant_authority_revision
            .map_or_else(|| "none".to_string(), |revision| revision.to_string());
        format!(
            "definition:{}/connection:{}/credential:{}/grant:{}/policy:{}",
            self.semantic_digest,
            self.connection_revision,
            credential_revision,
            grant_revision,
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

struct ResolvedAuthentication<'a> {
    active: bool,
    granted_scopes: &'a [String],
    account_id: Option<&'a str>,
    credential_revision: Option<u64>,
    grant_id: Option<&'a str>,
    grant_authority_revision: Option<u64>,
}

fn resolved_authentication<'a>(
    descriptor: &'a crate::AdapterConnectionV4,
    definition: &crate::CompiledAdapterDefinition,
    oauth: &'a OauthAuthoritySnapshot,
) -> Result<ResolvedAuthentication<'a>, AdapterCatalogError> {
    match &descriptor.authentication {
        AdapterConnectionAuthenticationV1::Pending => Ok(ResolvedAuthentication {
            active: false,
            granted_scopes: &[],
            account_id: None,
            credential_revision: None,
            grant_id: None,
            grant_authority_revision: None,
        }),
        AdapterConnectionAuthenticationV1::None => Ok(ResolvedAuthentication {
            active: true,
            granted_scopes: &[],
            account_id: None,
            credential_revision: None,
            grant_id: None,
            grant_authority_revision: None,
        }),
        AdapterConnectionAuthenticationV1::Credential { revision, .. } => {
            Ok(ResolvedAuthentication {
                active: true,
                granted_scopes: &[],
                account_id: None,
                credential_revision: Some(*revision),
                grant_id: None,
                grant_authority_revision: None,
            })
        }
        AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => {
            let grant = oauth
                .grants
                .iter()
                .find(|grant| grant.grant_id == *grant_id)
                .ok_or(AdapterCatalogError)?;
            let application = oauth
                .applications
                .iter()
                .find(|application| application.application_id == grant.application_id)
                .ok_or(AdapterCatalogError)?;
            let expected_profile = definition
                .authentication
                .oauth2()
                .map(|oauth| oauth.profile_digest.as_str())
                .ok_or(AdapterCatalogError)?;
            if application.profile_digest != expected_profile {
                return Err(AdapterCatalogError);
            }
            Ok(ResolvedAuthentication {
                active: grant.status == AuthorizationGrantStatus::Active,
                granted_scopes: &grant.granted_scopes,
                account_id: grant.account_id.as_deref(),
                credential_revision: None,
                grant_id: Some(grant_id),
                grant_authority_revision: Some(grant.authority_revision),
            })
        }
    }
}

fn binding(
    canonical_name: ToolName,
    definition: &crate::CompiledAdapterDefinition,
    descriptor: &crate::AdapterConnectionV4,
    operation: &crate::CompiledOperation,
    authentication: &ResolvedAuthentication<'_>,
    connection_policy: CapabilityConnectionPolicy,
    behavior: CapabilityToolBehavior,
) -> Result<CapabilityBinding, AdapterCatalogError> {
    let authority = AdapterOperationAuthorityV1 {
        version: TOKEN_VERSION,
        canonical_name: canonical_name.as_str().to_string(),
        connection_id: descriptor.connection_id.clone(),
        connection_slug: descriptor.connection_slug.clone(),
        account_id: authentication.account_id.map(str::to_string),
        semantic_digest: descriptor.semantic_digest.clone(),
        operation_id: operation.operation_id.clone(),
        operation_digest: operation.operation_digest.to_string(),
        definition_token: operation.token.as_str().to_string(),
        connection_revision: descriptor.connection_revision,
        credential_revision: authentication.credential_revision,
        grant_id: authentication.grant_id.map(str::to_string),
        grant_authority_revision: authentication.grant_authority_revision,
        policy_revision: descriptor.policy_revision,
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
        authentication.account_id.map(str::to_string),
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
    descriptor: &crate::AdapterConnectionV4,
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
