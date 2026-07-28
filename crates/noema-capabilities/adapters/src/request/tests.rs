use super::*;
use crate::{AdapterCompiler, AdapterManifestV3};
use serde_json::json;

fn definition() -> CompiledAdapterDefinition {
    let manifest: AdapterManifestV3 = serde_json::from_value(json!({
        "schema_version": 3,
        "definition_id": "definition:request_fixture",
        "adapter_id": "request_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {"mode": "static_bearer", "scopes": []},
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "inspect_item",
            "method": "GET",
            "path": "/v1/items/{item_id}",
            "fixed_headers": {"accept": "application/json"},
            "arguments": [
                {"name": "item_id", "source": "model_input", "location": "path", "type": "string", "required": true},
                {"name": "label", "source": "model_input", "location": "query", "type": "string", "required": true},
                {"name": "tag", "source": "model_input", "location": "query", "type": "string_array"},
                {"name": "visible", "source": "model_input", "location": "json_body", "type": "boolean"}
            ],
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"}
        }]
    })).expect("manifest");
    AdapterCompiler::compile(&manifest).expect("definition")
}

#[test]
fn encodes_path_query_and_body_only_from_the_reviewed_plan() {
    let definition = definition();
    let request = encode_request(
        &definition,
        &definition.operations[0],
        &json!({
            "item_id": "../private/item",
            "label": "a&b",
            "tag": ["one two", "three"],
            "visible": true
        }),
    )
    .expect("request");

    assert_eq!(
        request.url.as_str(),
        "https://api.example.test/v1/items/..%2Fprivate%2Fitem?label=a%26b&tag=one+two&tag=three"
    );
    assert_eq!(request.body, Some(json!({"visible": true})));
    assert_eq!(
        request.headers.get("accept").map(String::as_str),
        Some("application/json")
    );
    assert!(!request.headers.contains_key("authorization"));

    let mut alternate_port = definition.clone();
    alternate_port.origin = "https://api.example.test:8443/".to_string();
    let request = encode_request(
        &alternate_port,
        &alternate_port.operations[0],
        &json!({"item_id": "one", "label": "two"}),
    )
    .expect("alternate port request");
    assert_eq!(request.url.port(), Some(8443));

    let request = encode_request(
        &definition,
        &definition.operations[0],
        &json!({"item_id": "one", "label": "two", "tag": null, "visible": null}),
    )
    .expect("nullable optional arguments");
    assert_eq!(request.url.query(), Some("label=two"));
    assert_eq!(request.body, None);
}

#[test]
fn rejects_missing_unknown_wrong_type_and_enum_arguments() {
    let mut definition = definition();
    definition.operations[0].arguments[1].enum_values = vec!["allowed".to_string()];
    let operation = &definition.operations[0];
    for arguments in [
        json!({"label": "allowed"}),
        json!({"item_id": "one", "label": "allowed", "secret": "x"}),
        json!({"item_id": 1, "label": "allowed"}),
        json!({"item_id": "one", "label": "denied"}),
        json!({"item_id": ".", "label": "allowed"}),
        json!({"item_id": "..", "label": "allowed"}),
    ] {
        assert_eq!(
            encode_request(&definition, operation, &arguments),
            Err(AdapterRequestError)
        );
    }
}
