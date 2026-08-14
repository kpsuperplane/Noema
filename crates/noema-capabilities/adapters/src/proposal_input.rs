//! Concise model input expanded into one complete canonical adapter manifest.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    AdapterManifest, AdapterOperation, AdapterOperationBehavior, ArgumentDefinition,
    AuthenticationSchemeV4, HttpMethod, OperationAuthorization, OutputSchema, OutputType,
    PaginationPolicy, ResponseContract, ResponseTransform, RetryPolicy,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DefinitionProposalInput {
    pub(crate) source_reference: String,
    #[serde(default)]
    pub(crate) new_definition: Option<NewDefinitionProposal>,
    #[serde(default)]
    pub(crate) base_semantic_digest: Option<String>,
    #[serde(default)]
    pub(crate) revision: Option<DefinitionRevisionProposal>,
    #[serde(default)]
    pub(crate) upsert_operations: Vec<OperationProposal>,
    #[serde(default)]
    pub(crate) remove_operation_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewDefinitionProposal {
    definition_id: String,
    adapter_id: String,
    #[serde(default)]
    display_name: Option<String>,
    definition_revision: String,
    origin: String,
    authentication: AuthenticationSchemeV4,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DefinitionRevisionProposal {
    definition_revision: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    authentication: Option<AuthenticationSchemeV4>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperationProposal {
    operation_id: String,
    description: String,
    #[serde(default)]
    source_description: Option<String>,
    method: HttpMethod,
    path: String,
    authorization: OperationAuthorization,
    #[serde(default)]
    fixed_headers: BTreeMap<String, String>,
    #[serde(default)]
    fixed_query: BTreeMap<String, String>,
    #[serde(default)]
    arguments: Vec<ArgumentDefinition>,
    #[serde(default)]
    json_body_template: Option<serde_json::Value>,
    read_only: bool,
    idempotent: bool,
    destructive: bool,
    open_world: bool,
    #[serde(default = "default_pagination")]
    pagination: PaginationPolicy,
    response: ResponseProposal,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ResponseProposal {
    FlatObject {
        fields: Vec<ResponseFieldProposal>,
    },
    ObjectList {
        source_pointer: String,
        output_name: String,
        max_items: usize,
        fields: Vec<ResponseFieldProposal>,
    },
    ScalarList {
        source_pointer: String,
        output_name: String,
        max_items: usize,
        item: ResponseScalarProposal,
    },
    Custom {
        accepted_content_types: Vec<String>,
        #[serde(default)]
        transform: Option<ResponseTransform>,
        output_schema: OutputSchema,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseFieldProposal {
    name: String,
    source_pointer: String,
    #[serde(flatten)]
    scalar: ResponseScalarProposal,
    #[serde(default)]
    required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseScalarProposal {
    #[serde(rename = "type")]
    value_type: ResponseScalarType,
    #[serde(default)]
    max_bytes: Option<usize>,
    #[serde(default)]
    truncate: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ResponseScalarType {
    String,
    Integer,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProposalInputError {
    pub(crate) reason: &'static str,
    pub(crate) path: String,
}

pub(crate) fn build_manifest(
    input: DefinitionProposalInput,
    base: Option<AdapterManifest>,
) -> Result<AdapterManifest, ProposalInputError> {
    let DefinitionProposalInput {
        new_definition,
        base_semantic_digest,
        revision,
        upsert_operations,
        remove_operation_ids,
        ..
    } = input;
    let mut manifest = match (
        new_definition,
        base_semantic_digest.as_ref(),
        revision,
        base,
    ) {
        (Some(new), None, None, None) => AdapterManifest {
            schema_version: 9,
            definition_id: new.definition_id,
            adapter_id: new.adapter_id,
            display_name: new.display_name,
            definition_revision: new.definition_revision,
            reviewed: false,
            origin: new.origin,
            authentication: new.authentication,
            operations: Vec::new(),
        },
        (None, Some(_), Some(revision), Some(mut base)) => {
            base.definition_revision = revision.definition_revision;
            if let Some(display_name) = revision.display_name {
                base.display_name = Some(display_name);
            }
            if let Some(origin) = revision.origin {
                base.origin = origin;
            }
            if let Some(authentication) = revision.authentication {
                base.authentication = authentication;
            }
            base.reviewed = false;
            base
        }
        _ => return Err(input_error("proposal_mode", ".")),
    };
    if upsert_operations.is_empty() && remove_operation_ids.is_empty() {
        return Err(input_error("proposal_changes", "upsert_operations"));
    }
    let mut changed = BTreeSet::new();
    for (index, operation_id) in remove_operation_ids.into_iter().enumerate() {
        if operation_id.is_empty() || !changed.insert(operation_id.clone()) {
            return Err(input_error(
                "proposal_changes",
                format!("remove_operation_ids[{index}]"),
            ));
        }
        let Some(position) = manifest
            .operations
            .iter()
            .position(|operation| operation.operation_id == operation_id)
        else {
            return Err(input_error(
                "proposal_changes",
                format!("remove_operation_ids[{index}]"),
            ));
        };
        manifest.operations.remove(position);
    }
    for (index, proposal) in upsert_operations.into_iter().enumerate() {
        if proposal.operation_id.is_empty() || !changed.insert(proposal.operation_id.clone()) {
            return Err(input_error(
                "proposal_changes",
                format!("upsert_operations[{index}].operation_id"),
            ));
        }
        let operation = proposal.into_operation(index)?;
        if let Some(existing) = manifest
            .operations
            .iter_mut()
            .find(|existing| existing.operation_id == operation.operation_id)
        {
            *existing = operation;
        } else {
            manifest.operations.push(operation);
        }
    }
    Ok(manifest)
}

pub(crate) fn operation_proposals(
    manifest: &AdapterManifest,
    operation_ids: &[String],
) -> Result<Vec<OperationProposal>, ProposalInputError> {
    let mut proposals = Vec::new();
    let mut selected = BTreeSet::new();
    for (index, operation_id) in operation_ids.iter().enumerate() {
        if !selected.insert(operation_id) {
            return Err(input_error(
                "operation_unavailable",
                format!("operation_ids[{index}]"),
            ));
        }
        let operation = manifest
            .operations
            .iter()
            .find(|operation| operation.operation_id == *operation_id)
            .ok_or_else(|| {
                input_error("operation_unavailable", format!("operation_ids[{index}]"))
            })?;
        proposals.push(OperationProposal::from_operation(operation));
    }
    Ok(proposals)
}

impl OperationProposal {
    fn from_operation(operation: &AdapterOperation) -> Self {
        Self {
            operation_id: operation.operation_id.clone(),
            description: operation.description.clone(),
            source_description: operation.source_description.clone(),
            method: operation.method,
            path: operation.path.clone(),
            authorization: operation.authorization.clone(),
            fixed_headers: operation.fixed_headers.clone(),
            fixed_query: operation.fixed_query.clone(),
            arguments: operation.arguments.clone(),
            json_body_template: operation.json_body_template.clone(),
            read_only: operation.behavior.read_only.value.unwrap_or(false),
            idempotent: operation.behavior.idempotent.value.unwrap_or(false),
            destructive: operation.behavior.destructive.value.unwrap_or(true),
            open_world: operation.behavior.open_world.value.unwrap_or(true),
            pagination: operation.pagination.clone(),
            response: ResponseProposal::Custom {
                accepted_content_types: operation.response.accepted_content_types.clone(),
                transform: operation.response.transform.clone(),
                output_schema: operation.response.output_schema.clone(),
            },
        }
    }

    fn into_operation(self, index: usize) -> Result<AdapterOperation, ProposalInputError> {
        let retry = if self.method == HttpMethod::Get && self.read_only && self.idempotent {
            RetryPolicy::TransportSafeRead
        } else {
            RetryPolicy::Never
        };
        Ok(AdapterOperation {
            operation_id: self.operation_id,
            description: self.description,
            source_description: self.source_description,
            method: self.method,
            path: self.path,
            authorization: self.authorization,
            fixed_headers: self.fixed_headers,
            fixed_query: self.fixed_query,
            arguments: self.arguments,
            json_body_template: self.json_body_template,
            behavior: AdapterOperationBehavior::model(
                self.read_only,
                self.idempotent,
                self.destructive,
                self.open_world,
            ),
            retry,
            pagination: self.pagination,
            response: self.response.into_contract(index)?,
        })
    }
}

impl ResponseProposal {
    fn into_contract(self, operation_index: usize) -> Result<ResponseContract, ProposalInputError> {
        let path = format!("upsert_operations[{operation_index}].response");
        match self {
            Self::Custom {
                accepted_content_types,
                transform,
                output_schema,
            } => Ok(ResponseContract {
                accepted_content_types,
                transform,
                output_schema,
            }),
            Self::FlatObject { fields } => {
                let (schema, assignments) =
                    object_schema_and_assignments(&fields, &path, "body", "output", "  ")?;
                Ok(generated_contract(
                    schema,
                    format!("{assignments}  return output"),
                ))
            }
            Self::ObjectList {
                source_pointer,
                output_name,
                max_items,
                fields,
            } => {
                validate_output_name(&output_name, &format!("{path}.output_name"))?;
                validate_max_items(max_items, &format!("{path}.max_items"))?;
                let source = lua_path(&source_pointer, &format!("{path}.source_pointer"))?;
                let (item_schema, assignments) =
                    object_schema_and_assignments(&fields, &path, "input", "item", "      ")?;
                let output_schema = object_schema(
                    [(
                        output_name.clone(),
                        OutputSchema {
                            value_type: OutputType::Array,
                            properties: BTreeMap::new(),
                            required: Vec::new(),
                            additional_properties: None,
                            items: Some(Box::new(item_schema)),
                            max_bytes: None,
                            max_items: Some(max_items),
                        },
                    )],
                    vec![output_name.clone()],
                );
                Ok(generated_contract(
                    output_schema,
                    format!(
                        "  local source = at(body, {source})\n  local values = json.array()\n  if type(source) == \"table\" then\n    for index = 1, math.min(#source, {max_items}) do\n      local input = source[index]\n      local item = json.object()\n{assignments}      values[#values + 1] = item\n    end\n  end\n  output[{}] = values\n  return output",
                        lua_string(&output_name)
                    ),
                ))
            }
            Self::ScalarList {
                source_pointer,
                output_name,
                max_items,
                item,
            } => {
                validate_output_name(&output_name, &format!("{path}.output_name"))?;
                validate_max_items(max_items, &format!("{path}.max_items"))?;
                let source = lua_path(&source_pointer, &format!("{path}.source_pointer"))?;
                let item_schema = item.output_schema(&format!("{path}.item"))?;
                let value = item.lua_value("source[index]", &format!("{path}.item"))?;
                let output_schema = object_schema(
                    [(
                        output_name.clone(),
                        OutputSchema {
                            value_type: OutputType::Array,
                            properties: BTreeMap::new(),
                            required: Vec::new(),
                            additional_properties: None,
                            items: Some(Box::new(item_schema)),
                            max_bytes: None,
                            max_items: Some(max_items),
                        },
                    )],
                    vec![output_name.clone()],
                );
                Ok(generated_contract(
                    output_schema,
                    format!(
                        "  local source = at(body, {source})\n  local values = json.array()\n  if type(source) == \"table\" then\n    for index = 1, math.min(#source, {max_items}) do\n      local value = {value}\n      if value ~= nil then values[#values + 1] = value end\n    end\n  end\n  output[{}] = values\n  return output",
                        lua_string(&output_name)
                    ),
                ))
            }
        }
    }
}

fn generated_contract(output_schema: OutputSchema, body: String) -> ResponseContract {
    ResponseContract {
        accepted_content_types: vec!["application/json".to_string()],
        transform: Some(ResponseTransform::Luau {
            source: format!(
                "return function(response)\n  local body = json.decode(response.body)\n  local function at(value, path)\n    for _, key in ipairs(path) do\n      if type(value) ~= \"table\" then return nil end\n      local next_value = value[key]\n      if next_value == nil then\n        local index = tonumber(key)\n        if index ~= nil and tostring(index) == key then next_value = value[index + 1] end\n      end\n      value = next_value\n    end\n    return value\n  end\n  local output = json.object()\n{body}\nend"
            ),
        }),
        output_schema,
    }
}

fn object_schema_and_assignments(
    fields: &[ResponseFieldProposal],
    path: &str,
    source_name: &str,
    target_name: &str,
    indent: &str,
) -> Result<(OutputSchema, String), ProposalInputError> {
    if fields.is_empty() {
        return Err(input_error("response_recipe", format!("{path}.fields")));
    }
    let mut properties = BTreeMap::new();
    let mut required = Vec::new();
    let mut assignments = String::new();
    for (index, field) in fields.iter().enumerate() {
        let field_path = format!("{path}.fields[{index}]");
        validate_output_name(&field.name, &format!("{field_path}.name"))?;
        if properties.contains_key(&field.name) {
            return Err(input_error("response_recipe", format!("{field_path}.name")));
        }
        let source = lua_path(
            &field.source_pointer,
            &format!("{field_path}.source_pointer"),
        )?;
        let value = field
            .scalar
            .lua_value(&format!("at({source_name}, {source})"), &field_path)?;
        assignments.push_str(&format!(
            "{indent}local value_{index} = {value}\n{indent}if value_{index} ~= nil then {target_name}[{}] = value_{index} end\n",
            lua_string(&field.name)
        ));
        properties.insert(field.name.clone(), field.scalar.output_schema(&field_path)?);
        if field.required {
            required.push(field.name.clone());
        }
    }
    let schema = object_schema(properties, required);
    Ok((schema, assignments))
}

impl ResponseScalarProposal {
    fn output_schema(&self, path: &str) -> Result<OutputSchema, ProposalInputError> {
        if matches!(self.value_type, ResponseScalarType::String) != self.max_bytes.is_some()
            || !matches!(self.value_type, ResponseScalarType::String) && self.truncate
            || self.max_bytes == Some(0)
        {
            return Err(input_error("response_recipe", format!("{path}.max_bytes")));
        }
        Ok(OutputSchema {
            value_type: match self.value_type {
                ResponseScalarType::String => OutputType::String,
                ResponseScalarType::Integer => OutputType::Integer,
                ResponseScalarType::Number => OutputType::Number,
                ResponseScalarType::Boolean => OutputType::Boolean,
            },
            properties: BTreeMap::new(),
            required: Vec::new(),
            additional_properties: None,
            items: None,
            max_bytes: self.max_bytes,
            max_items: None,
        })
    }

    fn lua_value(&self, source: &str, path: &str) -> Result<String, ProposalInputError> {
        self.output_schema(path)?;
        Ok(match (self.value_type, self.truncate, self.max_bytes) {
            (ResponseScalarType::String, true, Some(max_bytes)) => format!(
                "(type({source}) == \"string\" and text.truncate_utf8({source}, {max_bytes}) or nil)"
            ),
            (ResponseScalarType::String, false, Some(_)) => {
                format!("(type({source}) == \"string\" and {source} or nil)")
            }
            (ResponseScalarType::Integer | ResponseScalarType::Number, _, _) => {
                format!("(type({source}) == \"number\" and {source} or nil)")
            }
            (ResponseScalarType::Boolean, _, _) => {
                format!("(type({source}) == \"boolean\" and {source} or nil)")
            }
            _ => return Err(input_error("response_recipe", format!("{path}.max_bytes"))),
        })
    }
}

fn object_schema(
    properties: impl IntoIterator<Item = (String, OutputSchema)>,
    required: Vec<String>,
) -> OutputSchema {
    OutputSchema {
        value_type: OutputType::Object,
        properties: properties.into_iter().collect(),
        required,
        additional_properties: Some(false),
        items: None,
        max_bytes: None,
        max_items: None,
    }
}

fn validate_output_name(value: &str, path: &str) -> Result<(), ProposalInputError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(input_error("response_recipe", path));
    }
    Ok(())
}

fn validate_max_items(value: usize, path: &str) -> Result<(), ProposalInputError> {
    if value == 0 {
        return Err(input_error("response_recipe", path));
    }
    Ok(())
}

fn lua_path(pointer: &str, path: &str) -> Result<String, ProposalInputError> {
    if pointer.is_empty() {
        return Ok("{}".to_string());
    }
    let Some(pointer) = pointer.strip_prefix('/') else {
        return Err(input_error("response_recipe", path));
    };
    let mut values = Vec::new();
    for segment in pointer.split('/') {
        let mut decoded = String::new();
        let mut characters = segment.chars();
        while let Some(character) = characters.next() {
            if character == '~' {
                match characters.next() {
                    Some('0') => decoded.push('~'),
                    Some('1') => decoded.push('/'),
                    _ => return Err(input_error("response_recipe", path)),
                }
            } else if character.is_control() {
                return Err(input_error("response_recipe", path));
            } else {
                decoded.push(character);
            }
        }
        values.push(lua_string(&decoded));
    }
    Ok(format!("{{{}}}", values.join(", ")))
}

fn lua_string(value: &str) -> String {
    let mut output = String::from("\"");
    for byte in value.as_bytes() {
        match byte {
            b'"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            0x20..=0x7e => output.push(char::from(*byte)),
            _ => output.push_str(&format!("\\{byte:03}")),
        }
    }
    output.push('"');
    output
}

fn input_error(reason: &'static str, path: impl Into<String>) -> ProposalInputError {
    ProposalInputError {
        reason,
        path: path.into(),
    }
}

fn default_pagination() -> PaginationPolicy {
    PaginationPolicy::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterCompiler, network::AdapterHttpResponse};
    use serde_json::{Value, json};

    fn operation(operation_id: &str) -> Value {
        json!({
            "operation_id": operation_id,
            "description": format!("Read {operation_id}."),
            "method": "GET",
            "path": format!("/{operation_id}"),
            "authorization": {"kind": "none"},
            "read_only": true,
            "idempotent": true,
            "destructive": false,
            "open_world": false,
            "response": {
                "kind": "flat_object",
                "fields": [{
                    "name": "id",
                    "source_pointer": "/id",
                    "type": "string",
                    "max_bytes": 32,
                    "required": true
                }]
            }
        })
    }

    fn new_input(operations: Vec<Value>) -> DefinitionProposalInput {
        serde_json::from_value(json!({
            "source_reference": "https://developers.example.test/api",
            "new_definition": {
                "definition_id": "definition:example",
                "adapter_id": "example",
                "definition_revision": "v1",
                "origin": "https://api.example.test/",
                "authentication": {"kind": "none"}
            },
            "upsert_operations": operations
        }))
        .expect("proposal input")
    }

    #[test]
    fn response_recipe_compiles_and_enforces_projection_bounds() {
        let mut input =
            serde_json::to_value(new_input(vec![operation("list_items")])).expect("proposal value");
        input["upsert_operations"][0]["response"] = json!({
            "kind": "object_list",
            "source_pointer": "/data/items",
            "output_name": "items",
            "max_items": 2,
            "fields": [
                {"name": "id", "source_pointer": "/id", "type": "string", "max_bytes": 32, "required": true},
                {"name": "name", "source_pointer": "/name", "type": "string", "max_bytes": 3, "truncate": true}
            ]
        });
        let manifest = build_manifest(
            serde_json::from_value(input).expect("recipe proposal"),
            None,
        )
        .expect("manifest");
        AdapterCompiler::compile(&manifest).expect("compiled recipe");
        let contract = &manifest.operations[0].response;
        let output = crate::luau::transform(
            contract.transform.as_ref().expect("generated transform"),
            &AdapterHttpResponse {
                status: 200,
                content_type: Some("application/json".to_string()),
                body: r#"{"data":{"items":[{"id":"one","name":"éclair"},{"id":"two","name":"second"},{"id":"three","name":"third"}]}}"#
                    .as_bytes()
                    .to_vec(),
            },
        )
        .expect("transformed response");

        assert_eq!(
            output,
            json!({"items": [{"id": "one", "name": "éc"}, {"id": "two", "name": "sec"}]})
        );
        assert!(crate::output_schema::matches(
            &contract.output_schema,
            &output
        ));
    }

    #[test]
    fn operation_changes_preserve_untouched_operations() {
        let base = build_manifest(
            new_input(vec![operation("keep"), operation("remove")]),
            None,
        )
        .expect("base manifest");
        let revision = serde_json::from_value(json!({
            "source_reference": "https://developers.example.test/api-v2",
            "base_semantic_digest": "sha256:base",
            "revision": {"definition_revision": "v2"},
            "upsert_operations": [operation("add")],
            "remove_operation_ids": ["remove"]
        }))
        .expect("revision input");

        let revised = build_manifest(revision, Some(base)).expect("revised manifest");
        assert_eq!(revised.definition_revision, "v2");
        assert_eq!(
            revised
                .operations
                .iter()
                .map(|operation| operation.operation_id.as_str())
                .collect::<Vec<_>>(),
            ["keep", "add"]
        );
    }
}
