use super::*;
use crate::{AdapterCompiler, AdapterManifestV5};
use serde_json::json;
use std::collections::BTreeMap;

fn definition() -> CompiledAdapterDefinition {
    let manifest: AdapterManifestV5 = serde_json::from_value(json!({
        "schema_version": 5,
        "definition_id": "definition:request_fixture",
        "adapter_id": "request_fixture",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {"kind": "none"},
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "inspect_item",
            "method": "GET",
            "path": "/v1/items/{item_id}",
            "fixed_headers": {"accept": "application/json"},
            "fixed_query": {"orderBy": "startTime", "singleEvents": "true"},
            "arguments": [
                {"name": "item_id", "source": "model_input", "location": "path", "type": "string", "required": true},
                {"name": "label", "source": "model_input", "location": "query", "type": "string", "required": true},
                {"name": "tag", "source": "model_input", "location": "query", "type": "string_array"},
                {"name": "visible", "source": "model_input", "location": "json_body", "type": "boolean"}
            ],
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "transport_safe_read",
            "pagination": {"kind": "none"},
            "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
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
        "https://api.example.test/v1/items/..%2Fprivate%2Fitem?orderBy=startTime&singleEvents=true&label=a%26b&tag=one+two&tag=three"
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
    assert_eq!(
        request.url.query(),
        Some("orderBy=startTime&singleEvents=true&label=two")
    );
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

#[test]
fn renders_reviewed_nested_json_body_without_arbitrary_body_input() {
    let manifest: AdapterManifestV5 = serde_json::from_value(json!({
        "schema_version": 5,
        "definition_id": "definition:calendar_rsvp",
        "adapter_id": "calendar_rsvp",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://www.googleapis.com/",
        "authentication": {"kind": "none"},
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "respond_to_invitation",
            "method": "PATCH",
            "path": "/calendar/v3/calendars/{calendar_id}/events/{event_id}",
            "arguments": [
                {"name": "calendar_id", "source": "model_input", "location": "path", "type": "string", "required": true},
                {"name": "event_id", "source": "model_input", "location": "path", "type": "string", "required": true},
                {"name": "response_status", "source": "model_input", "location": "json_body", "type": "string", "required": true, "enum_values": ["accepted", "tentative", "declined"]}
            ],
            "json_body_template": {
                "attendees": [{"responseStatus": {"$argument": "response_status"}}],
                "attendeesOmitted": true
            },
            "behavior": {"readOnly": {"value": false, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "never",
            "pagination": {"kind": "none"},
            "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
        }]
    }))
    .expect("manifest");
    let definition = AdapterCompiler::compile(&manifest).expect("definition");
    let operation = &definition.operations[0];
    let request = encode_request(
        &definition,
        operation,
        &json!({
            "calendar_id": "primary",
            "event_id": "event-1",
            "response_status": "accepted"
        }),
    )
    .expect("request");

    assert_eq!(
        request.body,
        Some(json!({
            "attendees": [{"responseStatus": "accepted"}],
            "attendeesOmitted": true
        }))
    );
    assert!(
        encode_request(
            &definition,
            operation,
            &json!({
                "calendar_id": "primary",
                "event_id": "event-1",
                "response_status": {"arbitrary": "body"}
            }),
        )
        .is_err()
    );
}

#[test]
fn reviewed_luau_decorates_only_safe_sensitive_headers_and_query() {
    let cases = [
        (
            "return function(input) return { headers = { ['X-API-Key'] = input.credentials.key } } end",
            "X-API-Key",
            "secret-marker",
            None,
        ),
        (
            "return function(input) return { headers = { Authorization = 'Basic ' .. encoding.base64(input.credentials.key .. ':password') } } end",
            "Authorization",
            "Basic c2VjcmV0LW1hcmtlcjpwYXNzd29yZA==",
            None,
        ),
        (
            "return function(input) return { query = { api_key = input.credentials.key } } end",
            "",
            "",
            Some("api_key=secret-marker"),
        ),
    ];
    for (source, header, expected, query) in cases {
        let mut definition = definition();
        definition.authentication = credential_auth(source);
        let mut request = encode_request(
            &definition,
            &definition.operations[0],
            &json!({"item_id": "one", "label": "two"}),
        )
        .expect("request");
        apply_credential_auth(
            &definition,
            &definition.operations[0],
            &mut request,
            &crate::AdapterCredentialMaterial::Credential {
                fields: BTreeMap::from([("key".to_string(), "secret-marker".to_string())]),
            },
        )
        .expect("auth decoration");
        if !header.is_empty() {
            assert_eq!(
                request.sensitive_headers.get(header).map(String::as_str),
                Some(expected)
            );
        }
        if let Some(query) = query {
            assert!(
                request
                    .url
                    .query()
                    .is_some_and(|value| value.contains(query))
            );
        }
        assert!(!format!("{request:?}").contains("secret-marker"));
    }

    for source in [
        "return function(input) return { headers = { Host = input.credentials.key } } end",
        "return function(input) return { headers = { accept = input.credentials.key } } end",
        "return function(input) return { query = { label = input.credentials.key } } end",
    ] {
        let mut definition = definition();
        definition.authentication = credential_auth(source);
        let mut request = encode_request(
            &definition,
            &definition.operations[0],
            &json!({"item_id": "one", "label": "two"}),
        )
        .expect("request");
        assert!(
            apply_credential_auth(
                &definition,
                &definition.operations[0],
                &mut request,
                &crate::AdapterCredentialMaterial::Credential {
                    fields: BTreeMap::from([("key".to_string(), "secret-marker".to_string())]),
                },
            )
            .is_err()
        );
    }
}

fn credential_auth(source: &str) -> crate::AuthenticationSchemeV4 {
    serde_json::from_value(json!({
        "kind": "credential",
        "setup": {
            "credential_type": "API key",
            "setup_url": "https://developers.example.test/keys",
            "instructions": ["Create an API key."],
            "input": {"kind": "fields", "fields": [{"id": "key", "label": "API key"}]}
        },
        "request_auth": {"language": "luau", "source": source}
    }))
    .expect("credential auth")
}
