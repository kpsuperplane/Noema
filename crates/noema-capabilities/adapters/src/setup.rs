//! Model-visible, human-reviewed adapter definition proposal boundary.

use crate::{
    AdapterCapabilityService, AdapterCompileError, AdapterCompiler, AdapterManifest,
    DefinitionProvenance, DefinitionStoreError, OperationAuthorization, OutputSchema,
    proposal_input::{DefinitionProposalInput, build_manifest, operation_proposals},
};
use noema_capabilities::{
    CapabilityBinding, CapabilityError, CapabilityExecutionDecision, CapabilityOutput,
    CapabilityScope, CapabilityTarget, CapabilityToolBehavior, InvokerKey, OperationToken,
    PayloadSanitizer, ToolSpec, sanitize_standard_credentials,
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
const MAX_MANIFEST_BYTES: usize = 1_048_576;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionTemplateInput {
    #[serde(default)]
    semantic_digest: Option<String>,
    #[serde(default)]
    operation_ids: Vec<String>,
}

#[derive(Debug)]
struct AdapterSetupPayloadSanitizer;

impl PayloadSanitizer for AdapterSetupPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        let mut persisted = sanitize_standard_credentials(arguments);
        restore_proposal_metadata(arguments, &mut persisted);
        Some(persisted)
    }
}

fn restore_proposal_metadata(source: &Value, persisted: &mut Value) {
    match (source, persisted) {
        (Value::Object(source), Value::Object(persisted)) => {
            for (key, value) in source {
                let Some(persisted_value) = persisted.get_mut(key) else {
                    continue;
                };
                if (key == "authorization"
                    && serde_json::from_value::<OperationAuthorization>(value.clone()).is_ok())
                    || (key == "output_schema"
                        && serde_json::from_value::<OutputSchema>(value.clone()).is_ok())
                {
                    *persisted_value = value.clone();
                } else {
                    restore_proposal_metadata(value, persisted_value);
                }
            }
        }
        (Value::Array(source), Value::Array(persisted)) => {
            for (source, persisted) in source.iter().zip(persisted) {
                restore_proposal_metadata(source, persisted);
            }
        }
        _ => {}
    }
}

pub(crate) fn definition_template_binding() -> Result<CapabilityBinding, crate::AdapterCatalogError>
{
    let spec = ToolSpec::new(
        DEFINITION_TEMPLATE_TOOL,
        "List current API definitions or return a concise revision base. Before a revision, provide its exact digest and only the operation IDs that need inspection. Reuse the result during corrections.",
        json!({
            "type": "object",
            "properties": {
                "semantic_digest": {
                    "type": "string",
                    "description": "Optional exact definition digest selected for revision."
                },
                "operation_ids": {
                    "type": "array",
                    "maxItems": 32,
                    "items": {"type": "string"},
                    "description": "Exact operation IDs whose direct proposal form should be returned. Requires semantic_digest."
                }
            },
            "required": [],
            "additionalProperties": false
        }),
    )
    .map_err(|_| crate::AdapterCatalogError)?;
    setup_binding(spec, DEFINITION_TEMPLATE_TOKEN)
}

pub(crate) fn proposal_binding() -> Result<CapabilityBinding, crate::AdapterCatalogError> {
    let spec = ToolSpec::new(
        PROPOSE_DEFINITION_TOOL,
        concat!(
            "Propose one public HTTP API definition after researching official documentation. Call the definition-template tool first. ",
            "For a new service, provide new_definition and the required upsert_operations. For a revision, provide the exact base_semantic_digest, revision, and operation changes. ",
            "An upsert adds or replaces one complete operation by operation_id. remove_operation_ids removes exact operations. Noema compiles one complete immutable pending revision. ",
            "For OAuth, research and include a safe account_identity operation whenever the requested scopes expose a recognizable account identifier. ",
            "For each OAuth operation, include every documented scope alternative that supports its complete argument contract. Prefer the least privileged alternative. ",
            "Never include credentials, tokens, cookies, or private user data. Prefer the smallest required operation set. Do not use MCP endpoints as adapter origins."
        ),
        json!({
            "type": "object",
            "properties": {
                "source_reference": {
                    "type": "string",
                    "maxLength": MAX_SOURCE_REFERENCE_BYTES,
                    "description": "Official HTTPS API or authorization documentation URL used as primary provenance."
                },
                "new_definition": {
                    "type": "object",
                    "additionalProperties": {},
                    "description": "Direct JSON service header for a new definition. Include definition_id, adapter_id, optional display_name, definition_revision, origin, and authentication. Omit for a revision."
                },
                "base_semantic_digest": {
                    "type": "string",
                    "description": "Exact semantic digest loaded for a revision. Omit for a new definition."
                },
                "revision": {
                    "type": "object",
                    "additionalProperties": {},
                    "description": "Direct JSON revision header. Include definition_revision. Optionally replace display_name, origin, or authentication."
                },
                "upsert_operations": {
                    "type": "array",
                    "maxItems": 128,
                    "description": "Direct JSON operations added or replaced by stable operation_id.",
                    "items": {
                        "type": "object",
                        "additionalProperties": {},
                        "description": "One operation proposal from the definition-template contract."
                    }
                },
                "remove_operation_ids": {
                    "type": "array",
                    "maxItems": 128,
                    "items": {"type": "string"},
                    "description": "Stable operation IDs removed from the exact base revision."
                }
            },
            "required": ["source_reference"],
            "additionalProperties": false
        }),
    )
    .map_err(|_| crate::AdapterCatalogError)?;
    setup_binding(spec, PROPOSE_DEFINITION_TOKEN)
}
fn setup_binding(
    spec: ToolSpec,
    token: &str,
) -> Result<CapabilityBinding, crate::AdapterCatalogError> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .build(spec.input_schema.as_value())
        .map_err(|_| crate::AdapterCatalogError)?;
    let input_check: Arc<dyn noema_capabilities::ToolInputCheck> =
        Arc::new(move |arguments: &Value| validator.is_valid(arguments));
    Ok(CapabilityBinding::new(
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
        input_check,
        Arc::new(AdapterSetupPayloadSanitizer),
    ))
}

pub(crate) fn is_proposal_invocation(operation: &str, token: &OperationToken) -> bool {
    operation == PROPOSE_DEFINITION_TOOL && token.as_str() == PROPOSE_DEFINITION_TOKEN
}

pub(crate) fn is_definition_template_invocation(operation: &str, token: &OperationToken) -> bool {
    operation == DEFINITION_TEMPLATE_TOOL && token.as_str() == DEFINITION_TEMPLATE_TOKEN
}

impl AdapterCapabilityService {
    fn definition_help_payload() -> Value {
        let google_profile_digest = crate::reviewed_google_oauth_profile_digest();
        json!({
            "instructions": [
                "Replace example values with facts from official HTTPS documentation.",
                "Use the smallest operation set needed. Never include credentials or private user data.",
                "Use direct JSON. Do not serialize a manifest or response value into a string.",
                "For a revision, load the exact digest once and submit operation-keyed changes.",
                "Noema adds canonical schema fields, review state, behavior provenance, retry policy, and common response transforms.",
                "Declare all four behavior booleans from researched semantics.",
                "For each OAuth operation, research its official authorization documentation.",
                "Include every documented scope alternative that supports the complete operation and all accepted arguments.",
                "Prefer the least privileged scope alternative. Exclude alternatives that reject an accepted argument.",
                "Use response kind flat_object, object_list, or scalar_list when possible.",
                "Use kind custom only when pointer-based field projection cannot express the documented response.",
                "A string field requires max_bytes. Set truncate only for display text, never opaque identifiers.",
                "If authentication needs unsupported signing, mTLS, or challenges, report it as unsupported."
            ],
            "proposal_template": {
                "source_reference": "https://developers.example.test/api",
                "new_definition": {
                    "definition_id": "definition:example_service",
                    "adapter_id": "example_service",
                    "display_name": "Example Service",
                    "definition_revision": "v1",
                    "origin": "https://api.example.test/",
                    "authentication": {
                        "kind": "oauth2_authorization_code_pkce",
                        "profile_digest": google_profile_digest
                    }
                },
                "upsert_operations": [{
                    "operation_id": "list_items",
                    "description": "List a bounded page of items.",
                    "method": "GET",
                    "path": "/v1/items",
                    "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["official read scope URL"]]},
                    "arguments": [],
                    "read_only": true,
                    "idempotent": true,
                    "destructive": false,
                    "open_world": true,
                    "pagination": {"kind": "none"},
                    "response": {
                        "kind": "object_list",
                        "source_pointer": "/items",
                        "output_name": "items",
                        "max_items": 8,
                        "fields": [
                            {"name": "id", "source_pointer": "/id", "type": "string", "max_bytes": 256, "required": true},
                            {"name": "name", "source_pointer": "/name", "type": "string", "max_bytes": 512, "truncate": true}
                        ]
                    }
                }],
                "remove_operation_ids": []
            },
            "revision_template": {
                "source_reference": "https://developers.example.test/api",
                "base_semantic_digest": "exact digest from revision_base",
                "revision": {"definition_revision": "v2"},
                "upsert_operations": ["complete changed or added operation objects"],
                "remove_operation_ids": ["removed_operation_id"]
            },
            "flat_object_response_example": {
                "kind": "flat_object",
                "fields": [
                    {"name": "id", "source_pointer": "/id", "type": "string", "max_bytes": 256, "required": true},
                    {"name": "count", "source_pointer": "/count", "type": "integer"}
                ]
            },
            "scalar_list_response_example": {
                "kind": "scalar_list",
                "source_pointer": "/labels",
                "output_name": "labels",
                "max_items": 16,
                "item": {"type": "string", "max_bytes": 128, "truncate": true}
            },
            "custom_response_example": {
                "kind": "custom",
                "accepted_content_types": ["application/json"],
                "transform": {
                    "language": "luau",
                    "source": "return function(response) local body = json.decode(response.body) return { id = body.id } end"
                },
                "output_schema": {
                    "type": "object",
                    "properties": {"id": {"type": "string", "maxBytes": 256}},
                    "required": ["id"],
                    "additionalProperties": false
                }
            },
            "credential_authentication_example": {
                "kind": "credential",
                "setup": {
                    "credential_type": "API key",
                    "setup_url": "https://developers.example.test/api-keys",
                    "instructions": ["Create an API key and paste it below."],
                    "input": {"kind": "fields", "fields": [{"id": "api_key", "label": "API key"}]}
                },
                "request_auth": {
                    "language": "luau",
                    "source": "return function(input) return { headers = { ['X-API-Key'] = input.credentials.api_key } } end"
                }
            },
            "enums": {
                "authentication.kind": ["none", "credential", "oauth2_authorization_code_pkce"],
                "operation.authorization.kind": ["none", "oauth_scopes"],
                "operation.method": ["GET", "POST", "PUT", "PATCH", "DELETE"],
                "argument.location": ["path", "query", "json_body"],
                "argument.type": ["string", "integer", "number", "boolean", "string_array"],
                "operation.pagination.kind": ["none", "response_token"],
                "response.kind": ["flat_object", "object_list", "scalar_list", "custom"]
            },
            "nested_json_body_example": {
                "arguments": [{
                    "name": "response_status",
                    "description": "Attendance response to apply.",
                    "location": "json_body",
                    "type": "string",
                    "required": true,
                    "enum_values": ["accepted", "tentative", "declined"]
                }],
                "json_body_template": {
                    "attendees": [{"responseStatus": {"$argument": "response_status"}}]
                }
            },
            "response_token_pagination_example": {
                "kind": "response_token",
                "response_pointer": "/next_cursor",
                "request_argument": "cursor",
                "page_size": {"request_argument": "page_size", "value": 8},
                "request_argument_is_runtime_only": true
            }
        })
    }

    pub(crate) fn definition_template(
        &self,
        arguments: Value,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let input: DefinitionTemplateInput =
            serde_json::from_value(arguments).map_err(|_| CapabilityError::InvalidArguments)?;
        if let Some(digest) = input.semantic_digest {
            let stored = self
                .inner
                .definitions
                .load(&digest)
                .map_err(|_| CapabilityError::InvalidArguments)?;
            let selected_operations = operation_proposals(&stored.manifest, &input.operation_ids)
                .map_err(|_| CapabilityError::InvalidArguments)?;
            return Ok(CapabilityOutput::success(json!({
                "instructions": [
                    "Use revision_base.semantic_digest as base_semantic_digest.",
                    "Upsert only changed or added operations. Remove operations by stable operation_id.",
                    "Reuse this response. Do not reload it for each correction."
                ],
                "revision_base": {
                    "semantic_digest": digest,
                    "source_reference": stored.provenance.source_reference,
                    "definition_id": stored.manifest.definition_id,
                    "adapter_id": stored.manifest.adapter_id,
                    "display_name": stored.manifest.display_name,
                    "definition_revision": stored.manifest.definition_revision,
                    "origin": stored.manifest.origin,
                    "authentication": stored.manifest.authentication,
                    "operations": stored.manifest.operations.iter().map(|operation| json!({
                        "operation_id": operation.operation_id,
                        "description": operation.description
                    })).collect::<Vec<_>>()
                },
                "selected_operations": selected_operations
            })));
        } else if !input.operation_ids.is_empty() {
            return Err(CapabilityError::InvalidArguments);
        }
        let registry = self
            .definition_registry()
            .map_err(|_| CapabilityError::Unavailable)?;
        let mut definitions = registry
            .definitions
            .iter()
            .filter(|definition| {
                if definition.compiled.reviewed {
                    !registry
                        .replaced_definition_digests
                        .contains(definition.compiled.semantic_digest.as_str())
                } else {
                    !registry
                        .superseded_pending_digests
                        .contains(definition.compiled.semantic_digest.as_str())
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
        Ok(CapabilityOutput::success(payload))
    }

    pub(crate) fn propose_definition(
        &self,
        arguments: Value,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let arguments_json =
            serde_json::to_string(&arguments).map_err(|_| CapabilityError::InvalidArguments)?;
        let mut deserializer = serde_json::Deserializer::from_str(&arguments_json);
        let input: DefinitionProposalInput =
            match serde_path_to_error::deserialize(&mut deserializer) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(Self::proposal_rejection_at(
                        "proposal_input_invalid",
                        &safe_manifest_path(error.path()),
                    ));
                }
            };
        if let Err(reason) = validate_source_reference(&input.source_reference) {
            return Ok(Self::proposal_rejection(reason));
        }
        let source_reference = input.source_reference.clone();
        let base_semantic_digest = input.base_semantic_digest.clone();
        let base_manifest = if let Some(digest) = base_semantic_digest.as_deref() {
            match self.inner.definitions.load(digest) {
                Ok(stored) => Some(stored.manifest),
                Err(_) => return Ok(Self::proposal_rejection("replacement_target_invalid")),
            }
        } else {
            None
        };
        let manifest = match build_manifest(input, base_manifest) {
            Ok(manifest) => manifest,
            Err(error) => return Ok(Self::proposal_rejection_at(error.reason, &error.path)),
        };
        let manifest_bytes =
            serde_json::to_string(&manifest).map_err(|_| CapabilityError::Unavailable)?;
        if manifest_bytes.len() > MAX_MANIFEST_BYTES {
            return Ok(Self::proposal_rejection("manifest_too_large"));
        }
        if let Some((path, operation_id)) = missing_proposal_description(&manifest) {
            let mut output = Self::proposal_rejection_at("description", &path);
            output.payload["operation_id"] = json!(operation_id);
            output.payload["message"] = json!(format!(
                "The reviewed model-facing description at {path} cannot be empty."
            ));
            return Ok(output);
        }
        let proposed = match AdapterCompiler::compile(&manifest) {
            Ok(compiled) => compiled,
            Err(error) => return Ok(Self::proposal_compile_rejection(&manifest, &error)),
        };
        if let Some((index, operation)) =
            manifest
                .operations
                .iter()
                .enumerate()
                .find(|(_, operation)| {
                    operation.behavior.read_only.value != Some(true)
                        && operation.response.transform.is_none()
                })
        {
            let mut output = Self::proposal_rejection("mutation_response_transform");
            output.payload["manifest_path"] =
                json!(format!("operations[{index}].response.transform"));
            output.payload["operation_id"] = json!(operation.operation_id);
            output.payload["message"] = json!(
                "Chat-proposed operations that are not explicitly read-only must transform the documented provider response into a compact canonical receipt."
            );
            return Ok(output);
        }
        let _guard = self
            .inner
            .definition_lock
            .lock()
            .map_err(|_| CapabilityError::Unavailable)?;
        let registry = self
            .definition_registry()
            .map_err(|_| CapabilityError::Unavailable)?;
        let family = registry
            .definitions
            .iter()
            .filter(|definition| definition.compiled.definition_id == manifest.definition_id)
            .collect::<Vec<_>>();
        let mut replaces = BTreeSet::new();
        if family.is_empty() {
            if base_semantic_digest.is_some() {
                return Ok(Self::proposal_rejection("replacement_target_invalid"));
            }
        } else {
            let Some(target_digest) = base_semantic_digest.as_deref() else {
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
                        && !registry
                            .superseded_pending_digests
                            .contains(definition.compiled.semantic_digest.as_str())
                })
                .map(|definition| definition.compiled.semantic_digest.to_string())
                .collect::<BTreeSet<_>>();
            if !active_pending.is_empty() && !active_pending.contains(target_digest) {
                return Ok(Self::proposal_rejection("replacement_target_stale"));
            }
            if active_pending.is_empty() {
                let current_reviewed = family
                    .iter()
                    .filter(|definition| {
                        definition.compiled.reviewed
                            && !registry
                                .replaced_definition_digests
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
        if registry
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
        let replaces = replaces.into_iter().collect::<Vec<_>>();
        let transition = self
            .plan_definition_transition(&proposed, &replaces)
            .map_err(|_| CapabilityError::Unavailable)?;
        let next_step = if base_semantic_digest.is_none() {
            "Tell the human that the discovered definition is waiting for review above the chat composer, then continue setup through that chat intervention."
        } else if transition.authentication_changed {
            "Tell the human that the definition is waiting for review above the chat composer. After approval, continue authentication through Settings."
        } else {
            "Tell the human that the definition is waiting for review above the chat composer. Do not infer reauthorization from this proposal. After approval, continue from the current connection status."
        };
        let installed = match self.inner.definitions.install_with_provenance(
            &manifest,
            DefinitionProvenance {
                source_digest: None,
                source_extension: None,
                source_reference: source_reference.clone(),
                imported_at: None,
                replaces_semantic_digests: replaces,
                transition: Some(transition),
            },
            None,
        ) {
            Ok(installed) => installed,
            Err(DefinitionStoreError::Compile(error)) => {
                return Ok(Self::proposal_rejection(&error.to_string()));
            }
            Err(DefinitionStoreError::Json(_)) => {
                return Ok(Self::proposal_rejection("manifest_invalid"));
            }
            Err(DefinitionStoreError::Integrity("provenance")) => {
                return Ok(Self::proposal_rejection("source_reference_invalid"));
            }
            Err(_) => return Err(CapabilityError::Unavailable),
        };
        self.refresh_definition_registry()
            .map_err(|_| CapabilityError::Unavailable)?;
        Ok(CapabilityOutput::success(json!({
            "status": "review_required",
            "semantic_digest": installed.compiled.semantic_digest.as_str(),
            "display_name": display_name,
            "source_reference": source_reference,
            "operation_ids": operation_ids,
            "base_semantic_digest": base_semantic_digest,
            "next_step": next_step
        })))
    }

    fn proposal_rejection(reason: &str) -> CapabilityOutput {
        CapabilityOutput::success(json!({
            "status": "invalid_proposal",
            "reason": reason,
            "next_step": "Correct the reported value and retry. Reuse the definition template already loaded for this task."
        }))
    }

    fn proposal_compile_rejection(
        manifest: &AdapterManifest,
        error: &AdapterCompileError,
    ) -> CapabilityOutput {
        let reason = compile_error_reason(error);
        let mut output = Self::proposal_rejection(reason);
        output.payload["message"] = json!(error.to_string());
        let operation = manifest
            .operations
            .iter()
            .enumerate()
            .find(|(_, operation)| {
                crate::compiler::validate_operation(operation)
                    .as_ref()
                    .err()
                    == Some(error)
            });
        let Some((index, operation)) = operation else {
            return output;
        };
        let manifest_path = operation_manifest_path(index, reason);
        output.payload["manifest_path"] = json!(manifest_path);
        if reason != "operation_id" {
            output.payload["operation_id"] = json!(operation.operation_id);
        }
        output.payload["message"] = json!(format!(
            "Operation validation failed at {manifest_path} (reason: {reason}). Correct that reviewed manifest value and retry."
        ));
        if reason == "response_size" {
            let maximum =
                crate::output_schema::maximum_serialized_bytes(&operation.response.output_schema);
            output.payload["details"] = json!({
                "maximum_serialized_bytes": maximum,
                "limit_bytes": crate::output_schema::MAX_MODEL_RESULT_BYTES,
            });
            output.payload["message"] = json!(match maximum {
                Some(maximum) => format!(
                    "The response schema at {manifest_path} permits at most {maximum} serialized bytes, above Noema's {}-byte limit. response_size is a computed constraint, not a manifest field. Reduce maxItems or maxBytes, cap every returned array, and apply text.truncate_utf8 to display text using the same declared bounds. Never truncate opaque identifiers; reduce the item count or omit fields instead.",
                    crate::output_schema::MAX_MODEL_RESULT_BYTES
                ),
                None => format!(
                    "The response schema at {manifest_path} has no finite serialized-size bound within Noema's {}-byte limit. response_size is a computed constraint, not a manifest field. Reduce maxItems or maxBytes, and make the reviewed transform enforce those same bounds.",
                    crate::output_schema::MAX_MODEL_RESULT_BYTES
                ),
            });
        }
        output
    }

    fn proposal_rejection_at(reason: &str, manifest_path: &str) -> CapabilityOutput {
        let mut output = Self::proposal_rejection(reason);
        output.payload["manifest_path"] = json!(manifest_path);
        output
    }
}

fn compile_error_reason(error: &AdapterCompileError) -> &'static str {
    match error {
        AdapterCompileError::Manifest => "manifest_invalid",
        AdapterCompileError::Invalid(reason) | AdapterCompileError::Unsupported(reason) => reason,
    }
}

fn missing_proposal_description(manifest: &AdapterManifest) -> Option<(String, String)> {
    for (operation_index, operation) in manifest.operations.iter().enumerate() {
        if operation.description.is_empty() {
            return Some((
                format!("operations[{operation_index}].description"),
                operation.operation_id.clone(),
            ));
        }
        for (argument_index, argument) in operation.arguments.iter().enumerate() {
            if argument.description.is_empty() {
                return Some((
                    format!(
                        "operations[{operation_index}].arguments[{argument_index}].description"
                    ),
                    operation.operation_id.clone(),
                ));
            }
        }
    }
    None
}

fn operation_manifest_path(index: usize, reason: &str) -> String {
    let field = match reason {
        "operation_id" => "operation_id",
        "description" => "description",
        "source_description" => "source_description",
        "operation_path" | "path_arguments" => "path",
        "arguments"
        | "argument_name"
        | "optional_path_argument"
        | "argument_enum"
        | "duplicate_argument" => "arguments",
        "json_body_template"
        | "json_body_template_arguments"
        | "json_body_template_placeholder"
        | "json_body_template_argument" => "json_body_template",
        "fixed_headers" | "fixed_header" | "authority_header" => "fixed_headers",
        "fixed_query" | "fixed_query_name" => "fixed_query",
        "operation_behavior" => "behavior",
        "unsafe_retry" => "retry",
        "pagination" | "pagination_response" => "pagination",
        "response_size" | "response_schema" | "reserved_response_field" => "response.output_schema",
        "response_content_types" | "response_content_type" => "response.accepted_content_types",
        "response_transform" => "response.transform",
        _ => return format!("operations[{index}]"),
    };
    format!("operations[{index}].{field}")
}

fn safe_manifest_path(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;

    let mut rendered = String::new();
    let mut dynamic_map = false;
    for segment in path {
        match segment {
            Segment::Seq { index } => {
                rendered.push_str(&format!("[{index}]"));
                dynamic_map = false;
            }
            Segment::Map { key } => {
                if dynamic_map {
                    rendered.push_str(".*");
                    break;
                }
                if !rendered.is_empty() {
                    rendered.push('.');
                }
                rendered.push_str(key);
                dynamic_map = matches!(
                    key.as_str(),
                    "arguments"
                        | "extra_authorization_parameters"
                        | "fixed_headers"
                        | "fixed_query"
                        | "properties"
                );
            }
            Segment::Enum { .. } | Segment::Unknown => {
                rendered.push_str(".*");
                break;
            }
        }
    }
    if rendered.is_empty() {
        ".".to_string()
    } else {
        rendered
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
    use crate::{AdapterDefinitionStore, Oauth2CallbackMode};
    use noema_capabilities::{
        CapabilityBindingSource, CapabilityInvocation, CapabilityInvoker, ToolName,
    };
    use noema_home::NoemaPaths;

    fn proposal_manifest(reviewed: bool) -> Value {
        json!({
            "schema_version": 9,
            "definition_id": "definition:discovered_calendar",
            "adapter_id": "discovered_calendar",
            "display_name": "Discovered Calendar",
            "definition_revision": "v1",
            "reviewed": reviewed,
            "origin": "https://api.example.test/",
            "authentication": {"kind": "none"},
            "operations": [{
                "operation_id": "list_events",
                "description": "List calendar events.",
                "method": "GET",
                "path": "/v1/events",
                "authorization": {"kind": "none"},
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
            }]
        })
    }

    fn new_proposal_arguments(manifest: Value, source_reference: &str) -> Value {
        let manifest: AdapterManifest = serde_json::from_value(manifest).expect("manifest");
        let operation_ids = manifest
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        let operations = crate::proposal_input::operation_proposals(&manifest, &operation_ids)
            .expect("operation proposals");
        json!({
            "source_reference": source_reference,
            "new_definition": {
                "definition_id": manifest.definition_id,
                "adapter_id": manifest.adapter_id,
                "display_name": manifest.display_name,
                "definition_revision": manifest.definition_revision,
                "origin": manifest.origin,
                "authentication": manifest.authentication
            },
            "upsert_operations": operations
        })
    }

    fn revision_arguments(
        base_semantic_digest: &str,
        manifest: Value,
        source_reference: &str,
    ) -> Value {
        let manifest: AdapterManifest = serde_json::from_value(manifest).expect("manifest");
        let operation_ids = manifest
            .operations
            .iter()
            .map(|operation| operation.operation_id.clone())
            .collect::<Vec<_>>();
        let operations = crate::proposal_input::operation_proposals(&manifest, &operation_ids)
            .expect("operation proposals");
        json!({
            "source_reference": source_reference,
            "base_semantic_digest": base_semantic_digest,
            "revision": {
                "definition_revision": manifest.definition_revision,
                "display_name": manifest.display_name,
                "origin": manifest.origin,
                "authentication": manifest.authentication
            },
            "upsert_operations": operations
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
        assert!(
            binding.spec().input_schema.as_value()["properties"]
                .get("manifest_json")
                .is_none()
        );
        assert_eq!(
            binding.spec().input_schema.as_value()["required"],
            json!(["source_reference"])
        );
        assert!(
            binding
                .spec()
                .description
                .contains("complete argument contract")
        );
        for pointer in [
            "/properties/new_definition",
            "/properties/revision",
            "/properties/upsert_operations/items",
        ] {
            let schema = binding
                .spec()
                .input_schema
                .as_value()
                .pointer(pointer)
                .expect("direct JSON schema");
            assert_eq!(schema["type"], "object");
            assert!(schema["additionalProperties"].is_object());
        }
        assert!(catalog.snapshot.resolve(DEFINITION_TEMPLATE_TOOL).is_some());
        assert_eq!(
            AdapterCapabilityService::definition_help_payload()["proposal_template"]["new_definition"]
                ["authentication"]["profile_digest"],
            json!(crate::reviewed_google_oauth_profile_digest())
        );
        assert!(
            AdapterCapabilityService::definition_help_payload()["instructions"]
                .as_array()
                .expect("instructions")
                .iter()
                .any(|instruction| instruction
                    .as_str()
                    .is_some_and(|text| text.contains("all accepted arguments")))
        );
        assert_eq!(
            AdapterCapabilityService::definition_help_payload()["response_token_pagination_example"],
            json!({
                "kind": "response_token",
                "response_pointer": "/next_cursor",
                "request_argument": "cursor",
                "page_size": {"request_argument": "page_size", "value": 8},
                "request_argument_is_runtime_only": true
            })
        );
        assert_eq!(
            binding.execution_decision(),
            CapabilityExecutionDecision::ExecuteImmediately
        );
        assert_eq!(
            binding.persist_arguments(&json!({"marker": "draft", "api_key": "private"})),
            Some(json!({"marker": "draft", "api_key": "[REDACTED]"}))
        );
        assert_eq!(
            binding.persist_arguments(&json!({
                "upsert_operations": [{
                    "authorization": {"kind": "none"},
                    "response": {"output_schema": {"type": "object", "properties": {"api_key": {"type": "string", "maxBytes": 32}}, "required": [], "additionalProperties": false}},
                    "api_key": "private"
                }]
            })),
            Some(json!({
                "upsert_operations": [{
                    "authorization": {"kind": "none"},
                    "response": {"output_schema": {"type": "object", "properties": {"api_key": {"type": "string", "maxBytes": 32}}, "required": [], "additionalProperties": false}},
                    "api_key": "[REDACTED]"
                }]
            }))
        );
        assert_eq!(
            binding.persist_output(&json!({
                "marker": "result",
                "access_token": "private"
            })),
            Some(json!({"marker": "result", "access_token": "[REDACTED]"}))
        );
    }

    #[test]
    fn definition_template_references_a_profile_without_client_setup() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        service.set_oauth_callback_mode(Oauth2CallbackMode::Hosted);

        let output = service
            .definition_template(json!({}))
            .expect("definition template");

        assert!(output.payload["compatible_oauth2_callback_mode"].is_null());
        assert_eq!(
            output.payload["proposal_template"]["new_definition"]["authentication"]["profile_digest"],
            json!(crate::reviewed_google_oauth_profile_digest())
        );
        assert!(
            output.payload["proposal_template"]["new_definition"]["authentication"]["setups"]
                .is_null()
        );
    }

    #[tokio::test]
    async fn model_proposal_is_forced_pending_and_survives_service_recreation() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let invocation = proposal_invocation(
            &service,
            new_proposal_arguments(
                proposal_manifest(true),
                "https://developers.example.test/calendar",
            ),
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

    #[test]
    fn proposal_can_update_one_operation_in_an_existing_definition() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let first = service
            .propose_definition(new_proposal_arguments(
                proposal_manifest(false),
                "https://developers.example.test/calendar",
            ))
            .expect("initial proposal");
        let first_digest = first.payload["semantic_digest"]
            .as_str()
            .expect("initial digest");
        let replacement_source = "return function() return json.null end";
        let mut revised = proposal_manifest(false);
        revised["definition_revision"] = json!("v2");
        revised["operations"][0]["response"]["transform"]["source"] = json!(replacement_source);

        let replacement = service
            .propose_definition(revision_arguments(
                first_digest,
                revised,
                "https://developers.example.test/calendar-v2",
            ))
            .expect("replacement proposal");

        assert_eq!(replacement.payload["status"], "review_required");
        let stored = AdapterDefinitionStore::new(paths)
            .load(
                replacement.payload["semantic_digest"]
                    .as_str()
                    .expect("replacement digest"),
            )
            .expect("stored replacement");
        assert_eq!(stored.manifest.definition_revision, "v2");
        assert_eq!(
            stored.manifest.operations[0]
                .response
                .transform
                .as_ref()
                .map(|transform| match transform {
                    crate::ResponseTransform::Luau { source } => source.as_str(),
                }),
            Some(replacement_source)
        );
        assert_eq!(stored.manifest.operations[0].path, "/v1/events");
    }

    #[test]
    fn operation_changes_fail_closed_for_unknown_or_conflicting_ids() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let first = service
            .propose_definition(new_proposal_arguments(
                proposal_manifest(false),
                "https://developers.example.test/calendar",
            ))
            .expect("initial proposal");
        let first_digest = first.payload["semantic_digest"]
            .as_str()
            .expect("initial digest");

        let operation = new_proposal_arguments(
            proposal_manifest(false),
            "https://developers.example.test/calendar",
        )["upsert_operations"][0]
            .clone();
        for changes in [
            json!({"remove_operation_ids": ["missing_operation"]}),
            json!({
                "remove_operation_ids": ["list_events"],
                "upsert_operations": [operation]
            }),
        ] {
            let mut arguments = json!({
                "source_reference": "https://developers.example.test/calendar-v2",
                "base_semantic_digest": first_digest,
                "revision": {"definition_revision": "v2"}
            });
            arguments
                .as_object_mut()
                .expect("arguments")
                .extend(changes.as_object().expect("changes").clone());
            let rejected = service
                .propose_definition(arguments)
                .expect("safe rejection");
            assert_eq!(rejected.payload["reason"], "proposal_changes");
        }
        assert_eq!(
            AdapterDefinitionStore::new(paths)
                .scan()
                .expect("scan")
                .definitions
                .len(),
            1
        );
    }

    #[test]
    fn proposals_require_non_empty_reviewed_descriptions_at_exact_paths() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        let mut manifest = proposal_manifest(false);
        manifest["operations"][0]["description"] = json!("");
        let rejected = service
            .propose_definition(new_proposal_arguments(
                manifest.clone(),
                "https://developers.example.test/calendar",
            ))
            .expect("operation description rejection");
        assert_eq!(rejected.payload["reason"], "description");
        assert_eq!(
            rejected.payload["manifest_path"],
            "operations[0].description"
        );

        manifest["operations"][0]["description"] = json!("List calendar events.");
        manifest["operations"][0]["arguments"] = json!([{
            "name": "calendar_id",
            "description": "",
            "location": "query",
            "type": "string"
        }]);
        let rejected = service
            .propose_definition(new_proposal_arguments(
                manifest,
                "https://developers.example.test/calendar",
            ))
            .expect("argument description rejection");
        assert_eq!(rejected.payload["reason"], "description");
        assert_eq!(
            rejected.payload["manifest_path"],
            "operations[0].arguments[0].description"
        );
    }

    #[tokio::test]
    async fn proposal_requires_mutation_response_transform_before_persistence() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let mut manifest = proposal_manifest(false);
        manifest["operations"][0]["operation_id"] = json!("create_event");
        manifest["operations"][0]["method"] = json!("POST");
        manifest["operations"][0]["behavior"]["readOnly"]["value"] = json!(false);
        manifest["operations"][0]["behavior"]["idempotent"]["value"] = json!(false);
        manifest["operations"][0]["retry"] = json!("never");
        manifest["operations"][0]["response"]["output_schema"] = json!({"type": "object", "properties": {"id": {"type": "string", "maxBytes": 256}}, "required": ["id"], "additionalProperties": false});
        manifest["operations"][0]["response"]
            .as_object_mut()
            .expect("response")
            .remove("transform");

        let rejected = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                new_proposal_arguments(
                    manifest.clone(),
                    "https://developers.example.test/calendar",
                ),
            )
            .await,
        )
        .await
        .expect("actionable rejection");
        assert_eq!(rejected.payload["reason"], "mutation_response_transform");
        assert_eq!(
            rejected.payload["manifest_path"],
            "operations[0].response.transform"
        );
        assert_eq!(rejected.payload["operation_id"], "create_event");
        assert!(
            AdapterDefinitionStore::new(paths.clone())
                .scan()
                .expect("scan rejected proposal")
                .definitions
                .is_empty()
        );

        manifest["operations"][0]["response"]["transform"] = json!({
            "language": "luau",
            "source": "return function(response) local body = json.decode(response.body) return { id = body.id } end"
        });
        let accepted = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                new_proposal_arguments(manifest, "https://developers.example.test/calendar"),
            )
            .await,
        )
        .await
        .expect("transformed proposal");
        assert_eq!(accepted.payload["status"], "review_required");
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
                new_proposal_arguments(
                    proposal_manifest(false),
                    "https://developers.example.test/calendar",
                ),
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
                new_proposal_arguments(
                    proposal_manifest(false),
                    "https://developers.example.test/calendar",
                ),
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
                revision_arguments(
                    &first_digest,
                    replacement,
                    "https://developers.example.test/calendar-v2",
                ),
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
        assert!(scan.superseded_pending_digests.contains(&first_digest));
        assert!(!scan.superseded_pending_digests.contains(&second_digest));
        assert!(matches!(
            service.review_definition(&first_digest),
            Err(crate::AdapterManagementError::Conflict)
        ));

        let selected = service
            .definition_template(json!({
                "semantic_digest": second_digest,
                "operation_ids": ["list_events"]
            }))
            .expect("template lookup");
        assert_eq!(
            selected.payload["selected_operations"][0]["path"],
            "/v2/events"
        );
        assert!(selected.payload.get("proposal_template").is_none());
        assert!(selected.payload.get("current_definitions").is_none());

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
                revision_arguments(
                    &reviewed_digest,
                    approved_replacement,
                    "https://developers.example.test/calendar-v3",
                ),
            )
            .await,
        )
        .await
        .expect("approved-definition replacement");
        assert_eq!(third.payload["status"], "review_required");
        let third_digest = third.payload["semantic_digest"]
            .as_str()
            .expect("third digest");
        assert!(
            service
                .cancel_definition_proposal(third_digest)
                .expect("cancel current proposal")
        );
        let final_scan = AdapterDefinitionStore::new(paths)
            .scan()
            .expect("final scan");
        assert!(final_scan.definitions.iter().any(|definition| {
            definition.compiled.semantic_digest == reviewed.compiled.semantic_digest
                && definition.compiled.reviewed
        }));
        assert!(
            final_scan
                .definitions
                .iter()
                .all(|definition| definition.compiled.reviewed)
        );
    }

    #[tokio::test]
    async fn proposal_rejects_non_https_provenance_and_invalid_direct_inputs() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let mut invalid_source = new_proposal_arguments(
            proposal_manifest(false),
            "http://developers.example.test/calendar",
        );
        let mut invalid_recipe = new_proposal_arguments(
            proposal_manifest(false),
            "https://developers.example.test/calendar",
        );
        invalid_recipe["upsert_operations"][0]["response"] = json!({
            "kind": "flat_object",
            "fields": [{"name": "id", "source_pointer": "/id", "type": "string"}]
        });
        for arguments in [
            std::mem::take(&mut invalid_source),
            json!({
                "source_reference": "https://developers.example.test/calendar",
                "new_definition": {"definition_id": "definition:invalid"},
                "upsert_operations": []
            }),
            invalid_recipe,
        ] {
            let invocation = proposal_invocation(&service, arguments).await;
            let output = CapabilityInvoker::invoke(&service, invocation)
                .await
                .expect("actionable rejection");
            assert_eq!(output.payload["status"], "invalid_proposal");
            assert!(output.payload.get("definition_help").is_none());
        }
        assert!(
            AdapterDefinitionStore::new(paths)
                .scan()
                .expect("scan")
                .definitions
                .is_empty()
        );
    }

    #[tokio::test]
    async fn proposal_reports_safe_manifest_paths_for_schema_errors() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        let cases = [(
            "/upsert_operations/0/pagination/kind",
            "upsert_operations[0].pagination.kind",
            "private-token",
        )];

        for (pointer, expected_path, private_value) in cases {
            let mut arguments = new_proposal_arguments(
                proposal_manifest(false),
                "https://developers.example.test/calendar",
            );
            *arguments.pointer_mut(pointer).expect("proposal field") = json!(private_value);
            let output =
                CapabilityInvoker::invoke(&service, proposal_invocation(&service, arguments).await)
                    .await
                    .expect("actionable rejection");

            assert_eq!(output.payload["reason"], "proposal_input_invalid");
            assert_eq!(output.payload["manifest_path"], expected_path);
            assert!(!output.payload.to_string().contains(private_value));
        }

        let mut arguments = new_proposal_arguments(
            proposal_manifest(false),
            "https://developers.example.test/calendar",
        );
        arguments["upsert_operations"][0]["fixed_headers"] = json!({"private-header-name": 7});
        let output =
            CapabilityInvoker::invoke(&service, proposal_invocation(&service, arguments).await)
                .await
                .expect("private map-key rejection");
        assert_eq!(
            output.payload["manifest_path"],
            "upsert_operations[0].fixed_headers.*"
        );
        assert!(!output.payload.to_string().contains("private-header-name"));
    }

    #[tokio::test]
    async fn proposal_compile_errors_identify_operations_and_response_budget() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);

        let mut invalid_path = proposal_manifest(false);
        invalid_path["operations"][0]["path"] = json!("/v1/../private");
        let output = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                new_proposal_arguments(invalid_path, "https://developers.example.test/calendar"),
            )
            .await,
        )
        .await
        .expect("path rejection");
        assert_eq!(output.payload["reason"], "operation_path");
        assert_eq!(output.payload["manifest_path"], "operations[0].path");
        assert_eq!(output.payload["operation_id"], "list_events");

        let mut oversized = proposal_manifest(false);
        oversized["operations"][0]["response"]["output_schema"] =
            json!({"type": "string", "maxBytes": 5_500});
        let output = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                new_proposal_arguments(oversized, "https://developers.example.test/calendar"),
            )
            .await,
        )
        .await
        .expect("response-size rejection");
        assert_eq!(output.payload["reason"], "response_size");
        assert_eq!(
            output.payload["manifest_path"],
            "operations[0].response.output_schema"
        );
        assert_eq!(output.payload["operation_id"], "list_events");
        assert_eq!(
            output.payload["details"],
            json!({
                "maximum_serialized_bytes": 33_002,
                "limit_bytes": 32_768
            })
        );
        assert!(
            output.payload["message"].as_str().is_some_and(
                |message| message.contains("computed constraint, not a manifest field")
            )
        );
        assert!(output.payload.get("definition_help").is_none());
    }
}
