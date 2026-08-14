//! Model-visible, human-reviewed adapter definition proposal boundary.

use crate::{
    AdapterCapabilityService, AdapterCompileError, AdapterCompiler, AdapterManifest,
    DefinitionProvenance, DefinitionStoreError,
};
use noema_capabilities::{
    CapabilityBinding, CapabilityError, CapabilityExecutionDecision, CapabilityOutput,
    CapabilityScope, CapabilityTarget, CapabilityToolBehavior, InvokerKey, OperationToken,
    PayloadSanitizer, RedactingPayloadSanitizer, ToolSpec, sanitize_standard_credentials,
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
    #[serde(default)]
    manifest_json: Option<String>,
    #[serde(default)]
    manifest_value_replacements: Vec<ManifestValueReplacement>,
    #[serde(default)]
    replaces_semantic_digest: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestValueReplacement {
    pointer: String,
    value_json: String,
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
        "Inspect current API definitions or return Noema's provider-neutral AdapterManifest template. Research official authentication documentation, then choose the smallest supported credential scheme. Before revising a definition, load its canonical manifest by semantic digest.",
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
    setup_binding(spec, DEFINITION_TEMPLATE_TOKEN)
}

pub(crate) fn proposal_binding() -> Result<CapabilityBinding, crate::AdapterCatalogError> {
    let spec = ToolSpec::new(
        PROPOSE_DEFINITION_TOOL,
        concat!(
            "Continue chat-first setup by proposing a small declarative public HTTP adapter after researching official API documentation with the available web search and fetch tools. Call the available definition-template tool before this tool. ",
            "Provide one official HTTPS source URL and either a complete AdapterManifest object or a bounded list of exact JSON value replacements against the selected canonical manifest. Never provide both. Noema always stores the proposal as pending human review. ",
            "When revising an existing definition, load its canonical manifest first and provide its exact digest as replaces_semantic_digest. Never submit a second unlinked proposal for the same definition family. ",
            "For a large existing definition, prefer manifest_value_replacements. Each JSON Pointer must identify an existing value; additions and removals are unsupported. Serialize each replacement value by itself in value_json. ",
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
                    "description": "Complete AdapterManifest object serialized as JSON. Omit when using manifest_value_replacements. Call the available definition-template tool first. Set reviewed to false; Noema enforces pending review."
                },
                "manifest_value_replacements": {
                    "type": "array",
                    "maxItems": 32,
                    "description": "Exact replacements applied to an existing canonical manifest. Omit when providing manifest_json.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "pointer": {
                                "type": "string",
                                "maxLength": 4096,
                                "description": "JSON Pointer to one existing manifest value."
                            },
                            "value_json": {
                                "type": "string",
                                "maxLength": MAX_MANIFEST_JSON_BYTES,
                                "description": "The complete replacement JSON value serialized as a string."
                            }
                        },
                        "required": ["pointer", "value_json"],
                        "additionalProperties": false
                    }
                },
                "replaces_semantic_digest": {
                    "type": "string",
                    "description": "Exact current pending or reviewed definition digest replaced by this complete proposal. Required for an existing definition family and omitted for a new one."
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
    let sanitizer: Arc<dyn PayloadSanitizer> = if token == PROPOSE_DEFINITION_TOKEN {
        Arc::new(ProposalPayloadSanitizer)
    } else {
        Arc::new(RedactingPayloadSanitizer)
    };
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
        sanitizer,
    ))
}

#[derive(Debug)]
struct ProposalPayloadSanitizer;

impl PayloadSanitizer for ProposalPayloadSanitizer {
    fn persist_arguments(&self, arguments: &Value) -> Option<Value> {
        Some(sanitize_standard_credentials(arguments))
    }

    fn persist_output(&self, output: &Value) -> Option<Value> {
        let mut output = sanitize_standard_credentials(output);
        if let Some(object) = output.as_object_mut() {
            object.remove("definition_help");
        }
        Some(output)
    }
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
        let payload = json!({
            "instructions": [
                "Replace every example.test value with facts supported by the official HTTPS source.",
                "Use the smallest operation set needed. Results may be delivered to the user's configured model provider.",
                "Keep credential values out of the manifest and Luau source. Credential fields and documents are write-only setup inputs.",
                "Prefill all four behavior hints from the researched operation semantics with source=model. Noema will apply pessimistic defaults if any field is missing.",
                "For every authenticated API, provide the exact provider credential type, official HTTPS setup URL, and short ordered instructions.",
                "Use kind=credential for API keys, tokens, Basic auth, or query credentials. The request_auth Luau transform may emit only headers and query values.",
                "Reference one reviewed OAuth profile by its exact digest. Put accepted complete scope sets on each OAuth operation.",
                "Use a root HTTPS origin with path=/, and put every provider API prefix in operation paths.",
                "Put non-secret provider parameters that are required for correct operation semantics in fixed_query. Do not expose invariants such as expansion, ordering, projection, or API version as optional model arguments.",
                "Give every operation and model-input argument a concise reviewed description. Explain resource identity, accepted aliases or special values, format expectations, defaults, and when an optional argument should be omitted. Never copy untrusted source prose into these fields without reviewing it.",
                "By default, each json_body argument becomes one top-level member with its declared scalar or string-array type. For a reviewed nested shape, set json_body_template to a JSON object and place each declared json_body argument exactly once as {\"$argument\":\"argument_name\"}; missing or null optional placeholders omit their property and any resulting empty array item, while constants remain exact reviewed values. Whole arbitrary JSON bodies remain unsupported.",
                "Every operation must include pagination. Use kind=none for a single bounded page. A response_token request_argument is runtime-only and must not also be declared in the operation arguments.",
                "For response_token collections, always use a compact top-level object transform and omit the reserved continuation field. Noema removes the provider token before transformation and injects its own opaque continuation. Use fixed page_size shaping when the provider supports it.",
                "Use compact summaries plus continuation for list/search, one bounded richer record for get/detail, compact receipts for mutations, and artifact metadata or references for file/blob/export operations.",
                "Every chat-proposed operation that is not explicitly read-only must include a response transform that constructs its compact canonical receipt from the documented provider response. Never rely on a closed subset schema to discard provider fields.",
                "If the provider requires signing, mTLS, a challenge protocol, or another unsupported authentication capability, report it as unsupported instead of approximating it with ambient Luau powers.",
                "Every operation must declare a response contract whose closed schema proves a worst-case result at or below 32 KiB. Every string needs maxBytes and every array needs maxItems. A transform must cap every returned array and apply text.truncate_utf8 to display text using those same bounds. Never truncate opaque identifiers: use their researched provider maximum and reduce maxItems or omit fields instead. In chat proposals, omit transform only for explicitly read-only operations with already-canonical bounded JSON or +json responses; imported definitions may retain exact raw JSON contracts. Otherwise use reviewed deterministic Luau before validation.",
                "For OAuth, use the supplied profile digest. Do not copy protocol endpoints or client setup into the API definition."
            ],
            "manifest_template": {
                "schema_version": 9,
                "definition_id": "definition:example_service",
                "adapter_id": "example_service",
                "display_name": "Example Service",
                "definition_revision": "v1",
                "reviewed": false,
                "origin": "https://api.example.test/",
                "authentication": {
                    "kind": "oauth2_authorization_code_pkce",
                    "profile_digest": google_profile_digest
                },
                "operations": [{
                    "operation_id": "get_profile",
                    "description": "Get the recognizable profile for this connection.",
                    "method": "GET",
                    "path": "/v1/profile",
                    "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["official profile scope URL"]]},
                    "fixed_headers": {},
                    "fixed_query": {},
                    "arguments": [],
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": false, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {"kind": "none"},
                    "response": {
                        "accepted_content_types": ["application/json"],
                        "transform": {"language": "luau", "source": "return function(response) local body = json.decode(response.body) return { displayName = body.displayName } end"},
                        "output_schema": {"type": "object", "properties": {"displayName": {"type": "string", "maxBytes": 256}}, "required": ["displayName"], "additionalProperties": false}
                    }
                }, {
                    "operation_id": "list_items",
                    "description": "List a bounded page of items for this connection.",
                    "method": "GET",
                    "path": "/v1/items",
                    "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["official read scope URL"]]},
                    "fixed_headers": {},
                    "fixed_query": {},
                    "arguments": [{
                        "name": "limit",
                        "description": "Maximum number of items to return.",
                        "location": "query",
                        "type": "integer",
                        "required": false,
                        "enum_values": []
                    }],
                    "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                    "retry": "transport_safe_read",
                    "pagination": {
                        "kind": "response_token",
                        "response_pointer": "/next_cursor",
                        "request_argument": "cursor",
                        "page_size": {"request_argument": "page_size", "value": 8}
                    },
                    "response": {
                        "accepted_content_types": ["application/json"],
                        "transform": {"language": "luau", "source": "return function(response) return {} end"},
                        "output_schema": {"type": "object", "properties": {}, "required": [], "additionalProperties": false}
                    }
                }]
            },
            "transformed_response_example": {
                "accepted_content_types": ["application/json"],
                "transform": {
                    "language": "luau",
                    "source": "return function(response)\n  local body = json.decode(response.body)\n  return { id = body.id }\nend"
                },
                "output_schema": {
                    "type": "object",
                    "properties": {"id": {"type": "string", "maxBytes": 256}},
                    "required": ["id"],
                    "additionalProperties": false
                }
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
                    "attendees": [{"responseStatus": {"$argument": "response_status"}}],
                    "attendeesOmitted": true
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
                "operation.behavior.fields": ["readOnly", "idempotent", "destructive", "openWorld"],
                "operation.behavior.source": ["model", "safe_default"],
                "operation.retry": ["never", "transport_safe_read"]
            },
            "response_token_pagination_example": {
                "kind": "response_token",
                "response_pointer": "/next_cursor",
                "request_argument": "cursor",
                "page_size": {"request_argument": "page_size", "value": 8},
                "request_argument_is_runtime_only": true,
                "declare_request_argument_in_operation_arguments": false
            }
        });
        payload
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
        let replaced_by_reviewed = self
            .inner
            .definitions
            .replaced_by_reviewed_digests(&scan)
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
        let manifest_json = match (
            input.manifest_json.as_deref(),
            input.manifest_value_replacements.as_slice(),
        ) {
            (Some(manifest_json), []) => manifest_json.to_string(),
            (None, replacements) if !replacements.is_empty() => {
                let Some(target_digest) = input.replaces_semantic_digest.as_deref() else {
                    return Ok(Self::proposal_rejection("replacement_target_invalid"));
                };
                let stored = self
                    .inner
                    .definitions
                    .load(target_digest)
                    .map_err(|_| CapabilityError::InvalidArguments)?;
                let mut manifest = serde_json::to_value(stored.manifest)
                    .map_err(|_| CapabilityError::Unavailable)?;
                let mut pointers = BTreeSet::new();
                for (index, replacement) in replacements.iter().enumerate() {
                    if replacement.pointer.is_empty() || !pointers.insert(&replacement.pointer) {
                        return Ok(Self::proposal_rejection_at(
                            "manifest_replacement_invalid",
                            &format!("manifest_value_replacements[{index}].pointer"),
                        ));
                    }
                    let Ok(value) = serde_json::from_str(&replacement.value_json) else {
                        return Ok(Self::proposal_rejection_at(
                            "manifest_replacement_invalid",
                            &format!("manifest_value_replacements[{index}].value_json"),
                        ));
                    };
                    let Some(target) = manifest.pointer_mut(&replacement.pointer) else {
                        return Ok(Self::proposal_rejection_at(
                            "manifest_replacement_invalid",
                            &format!("manifest_value_replacements[{index}].pointer"),
                        ));
                    };
                    *target = value;
                }
                serde_json::to_string(&manifest).map_err(|_| CapabilityError::Unavailable)?
            }
            _ => return Ok(Self::proposal_rejection("manifest_input_invalid")),
        };
        if manifest_json.len() > MAX_MANIFEST_JSON_BYTES {
            return Ok(Self::proposal_rejection("manifest_json_too_large"));
        }
        let mut deserializer = serde_json::Deserializer::from_str(&manifest_json);
        let mut manifest: AdapterManifest =
            match serde_path_to_error::deserialize(&mut deserializer) {
                Ok(manifest) => manifest,
                Err(error) => {
                    return Ok(Self::proposal_rejection_at(
                        "manifest_json_invalid",
                        &safe_manifest_path(error.path()),
                    ));
                }
            };
        manifest.reviewed = false;
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
                let replaced_by_reviewed = self
                    .inner
                    .definitions
                    .replaced_by_reviewed_digests(&scan)
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
        let replaces = replaces.into_iter().collect::<Vec<_>>();
        let transition = self
            .plan_definition_transition(&proposed, &replaces)
            .map_err(|_| CapabilityError::Unavailable)?;
        let installed = match self.inner.definitions.install_with_provenance(
            &manifest,
            DefinitionProvenance {
                source_digest: None,
                source_extension: None,
                source_reference: input.source_reference.clone(),
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
                return Ok(Self::proposal_rejection("manifest_json_invalid"));
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
        AdapterCompileError::Manifest => "manifest_json_invalid",
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
    use crate::{AdapterCompiler, AdapterDefinitionStore, Oauth2CallbackMode};
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
            json!(["source_reference"])
        );
        assert_eq!(
            binding.spec().input_schema.as_value()["properties"]["manifest_value_replacements"]["maxItems"],
            json!(32)
        );
        assert!(catalog.snapshot.resolve(DEFINITION_TEMPLATE_TOOL).is_some());
        let template: AdapterManifest = serde_json::from_value(
            AdapterCapabilityService::definition_help_payload()["manifest_template"].clone(),
        )
        .expect("template manifest");
        assert!(template.authentication.oauth2().is_some());
        AdapterCompiler::compile(&template).expect("compilable template");
        assert_eq!(
            AdapterCapabilityService::definition_help_payload()["manifest_template"]["operations"]
                [1]["fixed_query"],
            json!({})
        );
        assert_eq!(
            AdapterCapabilityService::definition_help_payload()["response_token_pagination_example"],
            json!({
                "kind": "response_token",
                "response_pointer": "/next_cursor",
                "request_argument": "cursor",
                "page_size": {"request_argument": "page_size", "value": 8},
                "request_argument_is_runtime_only": true,
                "declare_request_argument_in_operation_arguments": false
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
            binding.persist_output(&json!({
                "marker": "result",
                "access_token": "private",
                "definition_help": {"manifest_template": "large repair guidance"}
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
            output.payload["manifest_template"]["authentication"]["profile_digest"],
            json!(crate::reviewed_google_oauth_profile_digest())
        );
        assert!(output.payload["manifest_template"]["authentication"]["setups"].is_null());
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

    #[test]
    fn proposal_can_replace_exact_values_in_an_existing_canonical_manifest() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let first = service
            .propose_definition(json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": proposal_manifest(false).to_string()
            }))
            .expect("initial proposal");
        let first_digest = first.payload["semantic_digest"]
            .as_str()
            .expect("initial digest");
        let replacement_source = "return function() return json.null end";

        let replacement = service
            .propose_definition(json!({
                "source_reference": "https://developers.example.test/calendar-v2",
                "replaces_semantic_digest": first_digest,
                "manifest_value_replacements": [
                    {"pointer": "/definition_revision", "value_json": "\"v2\""},
                    {
                        "pointer": "/operations/0/response/transform/source",
                        "value_json": serde_json::to_string(replacement_source).expect("source JSON")
                    }
                ]
            }))
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
    fn manifest_value_replacements_fail_closed_for_non_exact_inputs() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths.clone());
        let first = service
            .propose_definition(json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": proposal_manifest(false).to_string()
            }))
            .expect("initial proposal");
        let first_digest = first.payload["semantic_digest"]
            .as_str()
            .expect("initial digest");

        for (pointer, value_json) in [
            ("/operations/1/path", "\"/v2/events\""),
            ("/operations/0/path", "not JSON"),
        ] {
            let rejected = service
                .propose_definition(json!({
                    "source_reference": "https://developers.example.test/calendar-v2",
                    "replaces_semantic_digest": first_digest,
                    "manifest_value_replacements": [{
                        "pointer": pointer,
                        "value_json": value_json
                    }]
                }))
                .expect("safe rejection");
            assert_eq!(rejected.payload["reason"], "manifest_replacement_invalid");
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
            .propose_definition(json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": manifest.to_string()
            }))
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
            .propose_definition(json!({
                "source_reference": "https://developers.example.test/calendar",
                "manifest_json": manifest.to_string()
            }))
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
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": manifest.to_string()
                }),
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
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": manifest.to_string()
                }),
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

    #[tokio::test]
    async fn proposal_reports_safe_manifest_paths_for_schema_errors() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        let cases = [(
            "/operations/0/pagination/kind",
            "operations[0].pagination.kind",
            "private-token",
        )];

        for (pointer, expected_path, private_value) in cases {
            let mut manifest = proposal_manifest(false);
            *manifest.pointer_mut(pointer).expect("manifest field") = json!(private_value);
            let output = CapabilityInvoker::invoke(
                &service,
                proposal_invocation(
                    &service,
                    json!({
                        "source_reference": "https://developers.example.test/calendar",
                        "manifest_json": manifest.to_string()
                    }),
                )
                .await,
            )
            .await
            .expect("actionable rejection");

            assert_eq!(output.payload["reason"], "manifest_json_invalid");
            assert_eq!(output.payload["manifest_path"], expected_path);
            assert!(!output.payload.to_string().contains(private_value));
        }

        let mut manifest = proposal_manifest(false);
        manifest["operations"][0]["fixed_headers"] = json!({"private-header-name": 7});
        let output = CapabilityInvoker::invoke(
            &service,
            proposal_invocation(
                &service,
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": manifest.to_string()
                }),
            )
            .await,
        )
        .await
        .expect("private map-key rejection");
        assert_eq!(
            output.payload["manifest_path"],
            "operations[0].fixed_headers.*"
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
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": invalid_path.to_string()
                }),
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
                json!({
                    "source_reference": "https://developers.example.test/calendar",
                    "manifest_json": oversized.to_string()
                }),
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
        assert!(output.payload["definition_help"].is_object());
    }
}
