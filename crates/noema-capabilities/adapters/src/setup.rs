//! Model-visible, human-reviewed adapter definition proposal boundary.

use crate::{
    AdapterCapabilityService, AdapterCompiler, AdapterManifestV5, DefinitionProvenance,
    DefinitionStoreError, Oauth2CallbackMode,
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
        "Inspect current API definitions or return Noema's provider-neutral AdapterManifestV5 template. Research official authentication documentation, then choose the smallest supported credential scheme. Before revising a definition, load its canonical manifest by semantic digest.",
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
            "Provide one official HTTPS source URL and a complete AdapterManifestV5 object. Noema always stores the proposal as pending human review. ",
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
                    "description": "Complete AdapterManifestV5 object serialized as JSON. Call the available definition-template tool first. Set reviewed to false; Noema enforces pending review."
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
    fn definition_help_payload(&self) -> Value {
        let mut payload = json!({
            "instructions": [
                "Replace every example.test value with facts supported by the official HTTPS source.",
                "Use the smallest operation set needed. Results may be delivered to the user's configured model provider.",
                "Keep credential values out of the manifest and Luau source. Credential fields and documents are write-only setup inputs.",
                "Prefill all four behavior hints from the researched operation semantics with source=model. Noema will apply pessimistic defaults if any field is missing.",
                "For every authenticated API, provide the exact provider credential type, official HTTPS setup URL, and short ordered instructions.",
                "Use kind=credential for API keys, tokens, Basic auth, or query credentials. The request_auth Luau transform may emit only headers and query values.",
                "Use callback-specific OAuth setups. Their document Luau must normalize only client_id and the required client_secret, and must accept only the matching provider client shape.",
                "Use a root HTTPS origin with path=/, and put every provider API prefix in operation paths.",
                "By default, each json_body argument becomes one top-level member with its declared scalar or string-array type. For a reviewed nested shape, set json_body_template to a JSON object and place each required json_body argument exactly once as {\"$argument\":\"argument_name\"}; constants remain exact reviewed values. Whole arbitrary JSON bodies remain unsupported.",
                "Every operation must include pagination. Use kind=none for a single bounded page. A response_token request_argument is runtime-only and must not also be declared in the operation arguments.",
                "If the provider requires signing, mTLS, a challenge protocol, or another unsupported authentication capability, report it as unsupported instead of approximating it with ambient Luau powers.",
                "Every operation must declare a response contract whose closed schema proves a worst-case result at or below 32 KiB. Every string needs maxBytes and every array needs maxItems. Omit transform only for already-canonical JSON or +json responses; otherwise use reviewed deterministic Luau before validation.",
                "For OAuth, research a safe profile or self operation using the requested scopes. When it exposes a recognizable account string, include that operation and authentication.account_identity; omit both only when the authorized API provides no such identifier.",
                "When compatible_oauth2_callback_mode is present, use exactly that mode when correcting a callback mismatch for this Noema app."
            ],
            "manifest_template": {
                "schema_version": 5,
                "definition_id": "definition:example_service",
                "adapter_id": "example_service",
                "display_name": "Example Service",
                "definition_revision": "v1",
                "reviewed": false,
                "origin": "https://api.example.test/",
                "authentication": {
                    "kind": "oauth2_authorization_code_pkce",
                    "scopes": ["official scope URL"],
                    "authorization_endpoint": "https://auth.example.test/authorize",
                    "token_endpoint": "https://auth.example.test/token",
                    "client_authentication": "client_secret_post",
                    "setups": [{
                        "callback_mode": "loopback",
                        "setup": {
                            "credential_type": "Desktop app",
                            "setup_url": "https://developers.example.test/oauth/clients/new",
                            "instructions": [
                                "Create a Desktop app OAuth client.",
                                "Download its JSON credential document."
                            ],
                            "input": {
                                "kind": "document",
                                "media_type": "application/json",
                                "fields": [
                                    {"id": "client_id", "label": "Client ID"},
                                    {"id": "client_secret", "label": "Client secret"}
                                ],
                                "normalize": {
                                    "language": "luau",
                                    "source": "return function(input) local document = json.decode(input.document) return { client_id = document.installed.client_id, client_secret = document.installed.client_secret } end"
                                }
                            }
                        }
                    }],
                    "extra_authorization_parameters": {},
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
                    "response": {
                        "accepted_content_types": ["application/json"],
                        "output_schema": {"type": "object", "properties": {"displayName": {"type": "string", "maxBytes": 256}}, "required": ["displayName"], "additionalProperties": false}
                    },
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
                    "response": {
                        "accepted_content_types": ["application/json"],
                        "transform": {"language": "luau", "source": "return function(response) return nil end"},
                        "output_schema": {"type": "null"}
                    },
                    "gates": []
                }]
            },
            "transformed_response_example": {
                "accepted_content_types": ["text/csv"],
                "transform": {
                    "language": "luau",
                    "source": "return function(response)\n  return { value = response.body }\nend"
                },
                "output_schema": {
                    "type": "object",
                    "properties": {"value": {"type": "string", "maxBytes": 1024}},
                    "required": ["value"],
                    "additionalProperties": false
                }
            },
            "nested_json_body_example": {
                "arguments": [{
                    "name": "response_status",
                    "source": "model_input",
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
                "oauth2.client_authentication": ["none", "client_secret_basic", "client_secret_post"],
                "oauth2.setup.callback_mode": ["loopback", "hosted"],
                "operation.method": ["GET", "POST", "PUT", "PATCH", "DELETE"],
                "argument.location": ["path", "query", "json_body"],
                "argument.type": ["string", "integer", "number", "boolean", "string_array"],
                "quota.cost_class": ["free", "metered", "unknown"],
                "operation.pagination.kind": ["none", "response_token", "provider_link", "delta_cursor"],
                "operation.behavior.fields": ["readOnly", "idempotent", "destructive", "openWorld"],
                "operation.behavior.source": ["model", "safe_default"],
                "operation.retry": ["never", "transport_safe_read"]
            },
            "response_token_pagination_example": {
                "kind": "response_token",
                "response_pointer": "/nextPageToken",
                "request_argument": "pageToken",
                "request_argument_is_runtime_only": true,
                "declare_request_argument_in_operation_arguments": false
            }
        });
        if let Some(mode) = self
            .inner
            .oauth_callback_mode
            .lock()
            .ok()
            .and_then(|configured| *configured)
        {
            payload["compatible_oauth2_callback_mode"] = json!(mode);
            payload["manifest_template"]["authentication"]["setups"][0]["callback_mode"] =
                json!(mode);
            payload["manifest_template"]["authentication"]["setups"][0]["setup"]["credential_type"] =
                json!(match mode {
                    Oauth2CallbackMode::Loopback => "Desktop app",
                    Oauth2CallbackMode::Hosted => "Web application",
                });
            payload["manifest_template"]["authentication"]["setups"][0]["setup"]["instructions"] =
                json!(match mode {
                    Oauth2CallbackMode::Loopback => vec![
                        "Create a Desktop app OAuth client.",
                        "Download its JSON credential document.",
                    ],
                    Oauth2CallbackMode::Hosted => vec![
                        "Create a Web application OAuth client.",
                        "Add the authorized redirect URI shown by Noema.",
                        "Download its JSON credential document.",
                    ],
                });
            payload["manifest_template"]["authentication"]["setups"][0]["setup"]["input"]["normalize"]
                ["source"] = json!(match mode {
                Oauth2CallbackMode::Loopback =>
                    "return function(input) local document = json.decode(input.document) return { client_id = document.installed.client_id, client_secret = document.installed.client_secret } end",
                Oauth2CallbackMode::Hosted =>
                    "return function(input) local document = json.decode(input.document) return { client_id = document.web.client_id, client_secret = document.web.client_secret } end",
            });
        }
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
        let mut payload = self.definition_help_payload();
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
            return Ok(self.proposal_rejection(reason));
        }
        if input.manifest_json.len() > MAX_MANIFEST_JSON_BYTES {
            return Ok(self.proposal_rejection("manifest_json_too_large"));
        }
        let mut deserializer = serde_json::Deserializer::from_str(&input.manifest_json);
        let mut manifest: AdapterManifestV5 =
            match serde_path_to_error::deserialize(&mut deserializer) {
                Ok(manifest) => manifest,
                Err(error) => {
                    return Ok(self.proposal_rejection_at(
                        "manifest_json_invalid",
                        &safe_manifest_path(error.path()),
                    ));
                }
            };
        manifest.reviewed = false;
        let proposed = match AdapterCompiler::compile(&manifest) {
            Ok(compiled) => compiled,
            Err(error) => return Ok(self.proposal_rejection(&error.to_string())),
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
                return Ok(self.proposal_rejection("replacement_target_invalid"));
            }
        } else {
            let Some(target_digest) = input.replaces_semantic_digest.as_deref() else {
                return Ok(
                    self.proposal_rejection("existing_definition_requires_replacement_target")
                );
            };
            let target = family
                .iter()
                .find(|definition| definition.compiled.semantic_digest.as_str() == target_digest)
                .copied();
            let Some(target) = target else {
                return Ok(self.proposal_rejection("replacement_target_invalid"));
            };
            if target.compiled.adapter_id != manifest.adapter_id {
                return Ok(self.proposal_rejection("replacement_identity_changed"));
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
                return Ok(self.proposal_rejection("replacement_target_stale"));
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
                    return Ok(self.proposal_rejection("replacement_target_stale"));
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
                return Ok(self.proposal_rejection("proposal_unchanged"));
            }
            replaces.extend(active_pending);
            replaces.insert(target_digest.to_string());
        }
        if scan
            .definitions
            .iter()
            .any(|definition| definition.compiled.semantic_digest == proposed.semantic_digest)
        {
            return Ok(self.proposal_rejection("proposal_revision_already_exists"));
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
                return Ok(self.proposal_rejection(&error.to_string()));
            }
            Err(DefinitionStoreError::Json(_)) => {
                return Ok(self.proposal_rejection("manifest_json_invalid"));
            }
            Err(DefinitionStoreError::Integrity("provenance")) => {
                return Ok(self.proposal_rejection("source_reference_invalid"));
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

    fn proposal_rejection(&self, reason: &str) -> CapabilityOutput {
        CapabilityOutput::success(json!({
            "status": "invalid_proposal",
            "reason": reason,
            "definition_help": self.definition_help_payload(),
            "next_step": "Correct the manifest from the returned template and retry the available proposal tool."
        }))
    }

    fn proposal_rejection_at(&self, reason: &str, manifest_path: &str) -> CapabilityOutput {
        let mut output = self.proposal_rejection(reason);
        output.payload["manifest_path"] = json!(manifest_path);
        output
    }
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
                    "arguments" | "extra_authorization_parameters" | "fixed_headers" | "properties"
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
            "schema_version": 5,
            "definition_id": "definition:discovered_calendar",
            "adapter_id": "discovered_calendar",
            "display_name": "Discovered Calendar",
            "definition_revision": "v1",
            "reviewed": reviewed,
            "origin": "https://api.example.test/",
            "authentication": {"kind": "none"},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_events",
                "method": "GET",
                "path": "/v1/events",
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
            json!(["source_reference", "manifest_json"])
        );
        assert!(catalog.snapshot.resolve(DEFINITION_TEMPLATE_TOOL).is_some());
        let template: AdapterManifestV5 =
            serde_json::from_value(service.definition_help_payload()["manifest_template"].clone())
                .expect("template manifest");
        assert!(template.authentication.account_identity().is_some());
        AdapterCompiler::compile(&template).expect("compilable template");
        assert_eq!(
            service.definition_help_payload()["enums"]["quota.cost_class"],
            json!(["free", "metered", "unknown"])
        );
        assert_eq!(
            service.definition_help_payload()["response_token_pagination_example"],
            json!({
                "kind": "response_token",
                "response_pointer": "/nextPageToken",
                "request_argument": "pageToken",
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
            binding.persist_output(&json!({"marker": "result", "access_token": "private"})),
            Some(json!({"marker": "result", "access_token": "[REDACTED]"}))
        );
    }

    #[test]
    fn definition_template_uses_the_serving_shell_oauth_callback_mode() {
        let home = tempfile::tempdir().expect("home");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        let service = AdapterCapabilityService::new(paths);
        service.set_oauth_callback_mode(Oauth2CallbackMode::Hosted);

        let output = service
            .definition_template(json!({}))
            .expect("definition template");

        assert_eq!(
            output.payload["compatible_oauth2_callback_mode"],
            json!("hosted")
        );
        assert_eq!(
            output.payload["manifest_template"]["authentication"]["setups"][0]["callback_mode"],
            json!("hosted")
        );
        assert_eq!(
            output.payload["manifest_template"]["authentication"]["setups"][0]["setup"]["credential_type"],
            json!("Web application")
        );
        assert_eq!(
            output.payload["manifest_template"]["authentication"]["setups"][0]["setup"]["instructions"]
                [0],
            json!("Create a Web application OAuth client.")
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
        let cases = [
            ("/quota/cost_class", "quota.cost_class", "private-standard"),
            (
                "/operations/0/pagination/kind",
                "operations[0].pagination.kind",
                "private-token",
            ),
        ];

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
}
