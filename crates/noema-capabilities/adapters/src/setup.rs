//! Model-visible, human-reviewed adapter definition proposal boundary.

use crate::{
    AdapterCapabilityService, AdapterCompiler, AdapterManifestV3, DefinitionProvenance,
    DefinitionStoreError,
};
use noema_capabilities::{
    CapabilityBinding, CapabilityError, CapabilityExecutionDecision, CapabilityOutput,
    CapabilityScope, CapabilityTarget, CapabilityToolBehavior, InvokerKey, OperationToken,
    RedactingPayloadSanitizer, ToolSpec,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
use url::Url;

pub(crate) const PROPOSE_DEFINITION_TOOL: &str = "adapter.propose_definition";
pub(crate) const PROPOSE_DEFINITION_TOKEN: &str = "adapter-setup-v1:propose-definition";
pub(crate) const DEFINITION_TEMPLATE_TOOL: &str = "adapter.definition_template";
pub(crate) const DEFINITION_TEMPLATE_TOKEN: &str = "adapter-setup-v1:definition-template";
const MAX_SOURCE_REFERENCE_BYTES: usize = 4_096;
const MAX_MANIFEST_JSON_BYTES: usize = 1_048_576;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeDefinitionInput {
    source_reference: String,
    manifest_json: String,
    #[serde(default)]
    replaces_semantic_digest: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionTemplateInput {
    #[serde(default)]
    semantic_digest: Option<String>,
}

pub(crate) fn definition_template_binding() -> Result<CapabilityBinding, crate::AdapterCatalogError>
{
    let spec = ToolSpec::new(
        DEFINITION_TEMPLATE_TOOL,
        "Inspect current API definitions or return Noema's provider-neutral AdapterManifestV3 template. Before revising a definition, use the current-definition summaries to select its exact semantic digest, then call this tool again with that digest to load the canonical manifest.",
        json!({
            "type": "object",
            "properties": {
                "semantic_digest": {
                    "type": "string",
                    "description": "Optional exact definition digest whose canonical manifest should be returned for revision."
                }
            },
            "required": [],
            "additionalProperties": false
        }),
    )
    .map_err(|_| crate::AdapterCatalogError)?;
    Ok(setup_binding(spec, DEFINITION_TEMPLATE_TOKEN))
}

pub(crate) fn proposal_binding() -> Result<CapabilityBinding, crate::AdapterCatalogError> {
    let spec = ToolSpec::new(
        PROPOSE_DEFINITION_TOOL,
        concat!(
            "Continue chat-first setup by proposing a small declarative public HTTP adapter after researching official API documentation with the available web search and fetch tools. Call the available definition-template tool before this tool. ",
            "Provide one official HTTPS source URL and a complete AdapterManifestV3 object. Noema always stores the proposal as pending human review. ",
            "When revising an existing definition, load its canonical manifest first and provide its exact digest as replaces_semantic_digest. Never submit a second unlinked proposal for the same definition family. ",
            "For OAuth, research and include a safe account_identity operation whenever the requested scopes expose a recognizable account identifier. ",
            "Never include credentials, tokens, cookies, or private user data. Prefer the smallest read-only operation set needed for the request. This path is for public HTTP APIs; do not use MCP server endpoints as adapter origins or operations."
        ),
        json!({
            "type": "object",
            "properties": {
                "source_reference": {
                    "type": "string",
                    "maxLength": MAX_SOURCE_REFERENCE_BYTES,
                    "description": "Official HTTPS API or authorization documentation URL used as primary provenance."
                },
                "manifest_json": {
                    "type": "string",
                    "maxLength": MAX_MANIFEST_JSON_BYTES,
                    "description": "Complete AdapterManifestV3 object serialized as JSON. Call the available definition-template tool first. Set reviewed to false; Noema enforces pending review."
                },
                "replaces_semantic_digest": {
                    "type": "string",
                    "description": "Exact current pending or reviewed definition digest replaced by this complete proposal. Required for an existing definition family and omitted for a new one."
                }
            },
            "required": ["source_reference", "manifest_json"],
            "additionalProperties": false
        }),
    )
    .map_err(|_| crate::AdapterCatalogError)?;
    Ok(setup_binding(spec, PROPOSE_DEFINITION_TOKEN))
}

fn setup_binding(spec: ToolSpec, token: &str) -> CapabilityBinding {
    CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new(crate::catalog::ADAPTER_INVOKER_KEY),
            OperationToken::new(token),
        ),
        CapabilityToolBehavior {
            read_only: token == DEFINITION_TEMPLATE_TOKEN,
            idempotent: token == DEFINITION_TEMPLATE_TOKEN,
            destructive: false,
            open_world: false,
        },
        CapabilityExecutionDecision::ExecuteImmediately,
        CapabilityScope::Global,
        Arc::new(RedactingPayloadSanitizer),
    )
}

pub(crate) fn is_proposal_invocation(operation: &str, token: &OperationToken) -> bool {
    operation == PROPOSE_DEFINITION_TOOL && token.as_str() == PROPOSE_DEFINITION_TOKEN
}

pub(crate) fn is_definition_template_invocation(operation: &str, token: &OperationToken) -> bool {
    operation == DEFINITION_TEMPLATE_TOOL && token.as_str() == DEFINITION_TEMPLATE_TOKEN
}

impl AdapterCapabilityService {
    fn definition_help_payload() -> Value {
        json!({
            "instructions": [
                "Replace every example.test value with facts supported by the official HTTPS source.",
                "Use the smallest operation set needed. Results may be delivered to the user's configured model provider.",
                "Keep credential values out of the manifest. credential_import contains JSON pointers only.",
                "Prefill all four behavior hints from the researched operation semantics with source=model. Noema will apply pessimistic defaults if any field is missing.",
                "For OAuth client JSON setup, include the official HTTPS client_setup_url for the provider's developer console. Omit query strings and fragments.",
                "Omit an operation response block for ordinary JSON or +json responses. Use the reviewed Luau response contract only when non-JSON data must be parsed or the raw JSON shape must be normalized.",
                "For OAuth, research a safe profile or self operation using the requested scopes. When it exposes a recognizable account string, include that operation and authentication.account_identity; omit both only when the authorized API provides no such identifier."
            ],
            "manifest_template": {
                "schema_version": 3,
                "definition_id": "definition:example_service",
                "adapter_id": "example_service",
                "display_name": "Example Service",
                "definition_revision": "v1",
                "reviewed": false,
                "origin": "https://api.example.test/",
                "authentication": {
                    "mode": "oauth2_authorization_code_pkce",
                    "scopes": ["official scope URL"],
                    "client_setup_url": "https://developers.example.test/oauth/clients/new",
                    "credential_import": {
                        "kind": "oauth_client_json",
                        "alternatives": [{
                            "client_id_pointer": "/installed/client_id",
                            "client_secret_pointer": "/installed/client_secret"
                        }]
                    },
                    "oauth2": {
                        "authorization_endpoint": "https://auth.example.test/authorize",
                        "token_endpoint": "https://auth.example.test/token",
                        "client_authentication": "client_secret_post",
                        "callback_modes": ["loopback"],
                        "extra_authorization_parameters": {}
                    },
                    "account_identity": {
                        "operation_id": "get_profile",
                        "arguments": {},
                        "output_pointer": "/displayName"
                    }
                },
                "gates": [],
                "quota": {"cost_class": "free"},
                "operations": [{
                    "operation_id": "get_profile",
                    "method": "GET",
                    "path": "/v1/profile",
                    "fixed_headers": {},
                    "arguments": [],
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": false, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {"kind": "none"},
                    "gates": []
                }, {
                    "operation_id": "list_items",
                    "method": "GET",
                    "path": "/v1/items",
                    "fixed_headers": {},
                    "arguments": [{
                        "name": "limit",
                        "source": "model_input",
                        "location": "query",
                        "type": "integer",
                        "required": false,
                        "enum_values": []
                    }],
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {"kind": "none"},
                    "gates": []
                }]
            },
            "optional_response_example": {
                "accepted_content_types": ["text/csv"],
                "transform": {
                    "language": "luau",
                    "source": "return function(response)\n  return { value = response.body }\nend"
                },
                "output_schema": {
                    "type": "object",
                    "properties": {"value": {"type": "string"}},
                    "required": ["value"],
                    "additionalProperties": false
                }
            },
            "enums": {
                "authentication.mode": ["none", "static_bearer", "oauth2_authorization_code_pkce"],
                "oauth2.client_authentication": ["none", "client_secret_basic", "client_secret_post"],
                "oauth2.callback_modes": ["loopback", "hosted"],
                "operation.method": ["GET", "POST", "PUT", "PATCH", "DELETE"],
                "argument.location": ["path", "query", "json_body"],
                "argument.type": ["string", "integer", "number", "boolean", "string_array"],
                "operation.behavior.fields": ["readOnly", "idempotent", "destructive", "openWorld"],
                "operation.behavior.source": ["model", "safe_default"],
                "operation.retry": ["never", "transport_safe_read"]
            }
        })
    }

    pub(crate) fn definition_template(
        &self,
        arguments: Value,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let input: DefinitionTemplateInput =
            serde_json::from_value(arguments).map_err(|_| CapabilityError::InvalidArguments)?;
        let scan = self
            .inner
            .definitions
            .scan()
            .map_err(|_| CapabilityError::Unavailable)?;
        let superseded = self
            .inner
            .definitions
            .superseded_pending_digests(&scan)
            .map_err(|_| CapabilityError::Unavailable)?;
        let replaced_by_reviewed = scan
            .definitions
            .iter()
            .filter(|definition| definition.compiled.reviewed)
            .try_fold(BTreeSet::new(), |mut replaced, definition| {
                let stored = self
                    .inner
                    .definitions
                    .load(definition.compiled.semantic_digest.as_str())?;
                replaced.extend(stored.provenance.replaces_semantic_digests);
                Ok::<_, DefinitionStoreError>(replaced)
            })
            .map_err(|_| CapabilityError::Unavailable)?;
        let mut definitions = scan
            .definitions
            .iter()
            .filter(|definition| {
                if definition.compiled.reviewed {
                    !replaced_by_reviewed.contains(definition.compiled.semantic_digest.as_str())
                } else {
                    !superseded.contains(definition.compiled.semantic_digest.as_str())
                }
            })
            .map(|definition| {
                json!({
                    "semantic_digest": definition.compiled.semantic_digest.as_str(),
                    "definition_id": definition.compiled.definition_id,
                    "adapter_id": definition.compiled.adapter_id,
                    "display_name": definition.compiled.display_name,
                    "definition_revision": definition.compiled.definition_revision,
                    "reviewed": definition.compiled.reviewed
                })
            })
            .collect::<Vec<_>>();
        definitions.sort_by_key(Value::to_string);
        let truncated = definitions.len() > 100;
        definitions.truncate(100);
        let mut payload = Self::definition_help_payload();
        let object = payload
            .as_object_mut()
            .ok_or(CapabilityError::Unavailable)?;
        object.insert("current_definitions".to_string(), Value::Array(definitions));
        object.insert(
            "current_definitions_truncated".to_string(),
            Value::Bool(truncated),
        );
        if let Some(digest) = input.semantic_digest {
            let stored = self
                .inner
                .definitions
                .load(&digest)
                .map_err(|_| CapabilityError::InvalidArguments)?;
            object.insert(
                "selected_definition".to_string(),
                json!({
                    "semantic_digest": digest,
                    "source_reference": stored.provenance.source_reference,
                    "manifest_json": serde_json::to_string_pretty(&stored.manifest)
                        .map_err(|_| CapabilityError::Unavailable)?
                }),
            );
        }
        Ok(CapabilityOutput::success(payload))
    }

    pub(crate) fn propose_definition(
        &self,
        arguments: Value,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let input: ProposeDefinitionInput =
            serde_json::from_value(arguments).map_err(|_| CapabilityError::InvalidArguments)?;
        if let Err(reason) = validate_source_reference(&input.source_reference) {
            return Ok(Self::proposal_rejection(reason));
        }
        if input.manifest_json.len() > MAX_MANIFEST_JSON_BYTES {
            return Ok(Self::proposal_rejection("manifest_json_too_large"));
        }
        let mut manifest: AdapterManifestV3 = match serde_json::from_str(&input.manifest_json) {
            Ok(manifest) => manifest,
            Err(_) => return Ok(Self::proposal_rejection("manifest_json_invalid")),
        };
        manifest.reviewed = false;
        let proposed = match AdapterCompiler::compile(&manifest) {
            Ok(compiled) => compiled,
            Err(error) => return Ok(Self::proposal_rejection(&error.to_string())),
        };
        let _guard = self
            .inner
            .definition_lock
            .lock()
            .map_err(|_| CapabilityError::Unavailable)?;
        let scan = self
            .inner
            .definitions
            .scan()
            .map_err(|_| CapabilityError::Unavailable)?;
        let superseded = self
            .inner
            .definitions
            .superseded_pending_digests(&scan)
            .map_err(|_| CapabilityError::Unavailable)?;
        let family = scan
            .definitions
            .iter()
            .filter(|definition| definition.compiled.definition_id == manifest.definition_id)
            .collect::<Vec<_>>();
        let mut replaces = BTreeSet::new();
        if family.is_empty() {
            if input.replaces_semantic_digest.is_some() {
                return Ok(Self::proposal_rejection("replacement_target_invalid"));
            }
        } else {
            let Some(target_digest) = input.replaces_semantic_digest.as_deref() else {
                return Ok(Self::proposal_rejection(
                    "existing_definition_requires_replacement_target",
                ));
            };
            let target = family
                .iter()
                .find(|definition| definition.compiled.semantic_digest.as_str() == target_digest)
                .copied();
            let Some(target) = target else {
                return Ok(Self::proposal_rejection("replacement_target_invalid"));
            };
            if target.compiled.adapter_id != manifest.adapter_id {
                return Ok(Self::proposal_rejection("replacement_identity_changed"));
            }
            let active_pending = family
                .iter()
                .filter(|definition| {
                    !definition.compiled.reviewed
                        && !superseded.contains(definition.compiled.semantic_digest.as_str())
                })
                .map(|definition| definition.compiled.semantic_digest.to_string())
                .collect::<BTreeSet<_>>();
            if !active_pending.is_empty() && !active_pending.contains(target_digest) {
                return Ok(Self::proposal_rejection("replacement_target_stale"));
            }
            if active_pending.is_empty() {
                let replaced_by_reviewed = family
                    .iter()
                    .filter(|definition| definition.compiled.reviewed)
                    .try_fold(BTreeSet::new(), |mut replaced, definition| {
                        let stored = self
                            .inner
                            .definitions
                            .load(definition.compiled.semantic_digest.as_str())?;
                        replaced.extend(stored.provenance.replaces_semantic_digests);
                        Ok::<_, DefinitionStoreError>(replaced)
                    })
                    .map_err(|_| CapabilityError::Unavailable)?;
                let current_reviewed = family
                    .iter()
                    .filter(|definition| {
                        definition.compiled.reviewed
                            && !replaced_by_reviewed
                                .contains(definition.compiled.semantic_digest.as_str())
                    })
                    .max_by(|left, right| {
                        left.compiled
                            .definition_revision
                            .cmp(&right.compiled.definition_revision)
                            .then_with(|| {
                                left.compiled
                                    .semantic_digest
                                    .cmp(&right.compiled.semantic_digest)
                            })
                    });
                if current_reviewed.is_none_or(|definition| {
                    definition.compiled.semantic_digest.as_str() != target_digest
                }) {
                    return Ok(Self::proposal_rejection("replacement_target_stale"));
                }
            }
            let stored_target = self
                .inner
                .definitions
                .load(target_digest)
                .map_err(|_| CapabilityError::Unavailable)?;
            let mut comparable = stored_target.manifest;
            comparable.reviewed = false;
            if AdapterCompiler::compile(&comparable)
                .is_ok_and(|compiled| compiled.semantic_digest == proposed.semantic_digest)
            {
                return Ok(Self::proposal_rejection("proposal_unchanged"));
            }
            replaces.extend(active_pending);
            replaces.insert(target_digest.to_string());
        }
        if scan
            .definitions
            .iter()
            .any(|definition| definition.compiled.semantic_digest == proposed.semantic_digest)
        {
            return Ok(Self::proposal_rejection("proposal_revision_already_exists"));
        }
        let display_name = manifest
            .display_name
            .clone()
            .unwrap_or_else(|| manifest.adapter_id.clone());
        let operation_ids = manifest
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        let installed = match self.inner.definitions.install_with_provenance(
            &manifest,
            DefinitionProvenance {
                source_digest: None,
                source_extension: None,
                source_reference: input.source_reference.clone(),
                imported_at: None,
                replaces_semantic_digests: replaces.into_iter().collect(),
            },
            None,
        ) {
            Ok(installed) => installed,
            Err(DefinitionStoreError::Compile(error)) => {
                return Ok(Self::proposal_rejection(&error.to_string()));
            }
            Err(DefinitionStoreError::Json(_)) => {
                return Ok(Self::proposal_rejection("manifest_json_invalid"));
            }
            Err(DefinitionStoreError::Integrity("provenance")) => {
                return Ok(Self::proposal_rejection("source_reference_invalid"));
            }
            Err(_) => return Err(CapabilityError::Unavailable),
        };
        Ok(CapabilityOutput::success(json!({
            "status": "review_required",
            "semantic_digest": installed.compiled.semantic_digest.as_str(),
            "display_name": display_name,
            "source_reference": input.source_reference,
            "operation_ids": operation_ids,
            "replaces_semantic_digest": input.replaces_semantic_digest,
            "next_step": "Tell the human that the discovered definition is waiting for review directly above the chat composer, then continue setup through that chat intervention."
        })))
    }

    fn proposal_rejection(reason: &str) -> CapabilityOutput {
        CapabilityOutput::success(json!({
            "status": "invalid_proposal",
            "reason": reason,
            "definition_help": Self::definition_help_payload(),
            "next_step": "Correct the manifest from the returned template and retry the available proposal tool."
        }))
    }
}

fn validate_source_reference(reference: &str) -> Result<(), &'static str> {
    if reference.len() > MAX_SOURCE_REFERENCE_BYTES || reference.trim() != reference {
        return Err("source_reference_invalid");
    }
    let url = Url::parse(reference).map_err(|_| "source_reference_invalid")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("source_reference_invalid");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterCompiler, AdapterDefinitionStore};
    use noema_capabilities::{
        CapabilityBindingSource, CapabilityInvocation, CapabilityInvoker, ToolName,
    };
    use noema_home::NoemaPaths;

    fn proposal_manifest(reviewed: bool) -> Value {
        json!({
            "schema_version": 3,
            "definition_id": "definition:discovered_calendar",
            "adapter_id": "discovered_calendar",
            "display_name": "Discovered Calendar",
            "definition_revision": "v1",
            "reviewed": reviewed,
            "origin": "https://api.example.test/",
            "authentication": {"mode": "none", "scopes": []},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_events",
                "method": "GET",
                "path": "/v1/events",
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            }]
        })
    }

    async fn proposal_invocation(
        service: &AdapterCapabilityService,
        arguments: Value,
    ) -> CapabilityInvocation {
        let catalog = CapabilityBindingSource::catalog(service)
            .await
            .expect("catalog");
        let binding = catalog
            .snapshot
            .resolve(PROPOSE_DEFINITION_TOOL)
            .expect("proposal binding");
        CapabilityInvocation {
            operation: ToolName::new(PROPOSE_DEFINITION_TOOL).expect("tool name"),
            operation_token: binding.target().operation_token().clone(),
            arguments,
            reviewed_authorization: None,
        }
    }

    #[tokio::test]
    async fn proposal_binding_is_internal_and_persists_redacted_payloads() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        let catalog = CapabilityBindingSource::catalog(&service)
            .await
            .expect("catalog");
        let binding = catalog
            .snapshot
            .resolve(PROPOSE_DEFINITION_TOOL)
            .expect("binding");
        assert_eq!(
            binding.spec().input_schema.as_value()["properties"]["manifest_json"]["type"],
            "string"
        );
        assert_eq!(
            binding.spec().input_schema.as_value()["required"],
            json!(["source_reference", "manifest_json"])
        );
        assert!(catalog.snapshot.resolve(DEFINITION_TEMPLATE_TOOL).is_some());
        let template: AdapterManifestV3 = serde_json::from_value(
            AdapterCapabilityService::definition_help_payload()["manifest_template"].clone(),
        )
        .expect("template manifest");
        assert!(template.authentication.account_identity.is_some());
        AdapterCompiler::compile(&template).expect("compilable template");
        assert_eq!(
            binding.execution_decision(),
            CapabilityExecutionDecision::ExecuteImmediately
        );
        assert_eq!(
            binding.persist_arguments(&json!({"marker": "draft", "api_key": "private"})),
            Some(json!({"marker": "draft", "api_key": "[REDACTED]"}))
        );
        assert_eq!(
            binding.persist_output(&json!({"marker": "result", "access_token": "private"})),
            Some(json!({"marker": "result", "access_token": "[REDACTED]"}))
        );
    }

    #[tokio::test]
    async fn model_proposal_is_forced_pending_and_survives_service_recreation() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let invocation = proposal_invocation(
            &service,
            json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": proposal_manifest(true).to_string()
            }),
        )
        .await;
        let output = CapabilityInvoker::invoke(&service, invocation)
            .await
            .expect("proposal");
        assert_eq!(output.payload["status"], "review_required");

        drop(service);
        let scan = AdapterDefinitionStore::new(paths)
            .scan()
            .expect("rediscover");
        assert_eq!(scan.definitions.len(), 1);
        assert!(!scan.definitions[0].compiled.reviewed);
        assert_eq!(scan.definitions[0].projection.review_status, "pending");
    }

    #[tokio::test]
    async fn revisions_supersede_pending_drafts_and_fence_stale_review() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let first = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": proposal_manifest(false).to_string()
                }),
            )
            .await,
        )
        .await
        .expect("first proposal");
        let first_digest = first.payload["semantic_digest"]
            .as_str()
            .expect("first digest")
            .to_string();

        let missing_target = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": proposal_manifest(false).to_string()
                }),
            )
            .await,
        )
        .await
        .expect("safe rejection");
        assert_eq!(
            missing_target.payload["reason"],
            "existing_definition_requires_replacement_target"
        );

        let mut replacement = proposal_manifest(false);
        replacement["operations"][0]["path"] = json!("/v2/events");
        let second = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                json!({
                    "source_reference": "https://developers.example.test/calendar-v2",
                    "manifest_json": replacement.to_string(),
                    "replaces_semantic_digest": first_digest
                }),
            )
            .await,
        )
        .await
        .expect("replacement proposal");
        let second_digest = second.payload["semantic_digest"]
            .as_str()
            .expect("second digest")
            .to_string();
        let scan = AdapterDefinitionStore::new(paths.clone())
            .scan()
            .expect("scan");
        let superseded = AdapterDefinitionStore::new(paths.clone())
            .superseded_pending_digests(&scan)
            .expect("superseded drafts");
        assert!(superseded.contains(&first_digest));
        assert!(!superseded.contains(&second_digest));
        assert!(matches!(
            service.review_definition(&first_digest),
            Err(crate::AdapterManagementError::Conflict)
        ));

        let selected = service
            .definition_template(json!({"semantic_digest": second_digest}))
            .expect("template lookup");
        assert_eq!(
            selected.payload["selected_definition"]["manifest_json"]
                .as_str()
                .and_then(|manifest| serde_json::from_str::<Value>(manifest).ok())
                .and_then(|manifest| manifest["operations"][0]["path"]
                    .as_str()
                    .map(str::to_string)),
            Some("/v2/events".to_string())
        );

        let reviewed = service
            .review_definition(&second_digest)
            .expect("review current proposal");
        let reviewed_digest = reviewed.compiled.semantic_digest.to_string();
        let mut approved_replacement = proposal_manifest(false);
        approved_replacement["operations"][0]["path"] = json!("/v3/events");
        let third = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                json!({
                    "source_reference": "https://developers.example.test/calendar-v3",
                    "manifest_json": approved_replacement.to_string(),
                    "replaces_semantic_digest": reviewed_digest
                }),
            )
            .await,
        )
        .await
        .expect("approved-definition replacement");
        assert_eq!(third.payload["status"], "review_required");
        assert!(
            AdapterDefinitionStore::new(paths)
                .scan()
                .expect("final scan")
                .definitions
                .iter()
                .any(|definition| {
                    definition.compiled.semantic_digest == reviewed.compiled.semantic_digest
                        && definition.compiled.reviewed
                })
        );
    }

    #[tokio::test]
    async fn proposal_rejects_non_https_provenance_and_invalid_manifests() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        for arguments in [
            json!({
                "source_reference": "http://developers.example.test/calendar",
                "manifest_json": proposal_manifest(false).to_string()
            }),
            json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": serde_json::json!({"schema_version": 1}).to_string()
            }),
            json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": "x".repeat(MAX_MANIFEST_JSON_BYTES + 1)
            }),
        ] {
            let invocation = proposal_invocation(&service, arguments).await;
            let output = CapabilityInvoker::invoke(&service, invocation)
                .await
                .expect("actionable rejection");
            assert_eq!(output.payload["status"], "invalid_proposal");
            assert!(output.payload["definition_help"].is_object());
        }
        assert!(
            AdapterDefinitionStore::new(paths)
                .scan()
                .expect("scan")
                .definitions
                .is_empty()
        );
    }
}
