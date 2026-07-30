#![allow(missing_docs)]

use noema_runtime::a2ui::{A2UIValidationCode, NOEMA_A2UI_CATALOG_ID, parse_and_reduce};
use serde_json::json;

fn stream(messages: &[serde_json::Value]) -> String {
    messages
        .iter()
        .map(|message| serde_json::to_string(message).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a2ui_validation_and_reduction_cases() {
    let valid = stream(&[
        json!({
            "version": "v0.9.1",
            "createSurface": {
                "surfaceId": "main",
                "catalogId": NOEMA_A2UI_CATALOG_ID,
                "sendDataModel": true
            }
        }),
        json!({
            "version": "v0.9.1",
            "updateComponents": {
                "surfaceId": "main",
                "components": [
                    {"id": "submit", "component": "Button", "child": "label", "action": {"event": {"name": "submit"}}},
                    {"id": "label", "component": "Text", "text": "Save"},
                    {"id": "root", "component": "Column", "children": ["label", "submit"]}
                ]
            }
        }),
        json!({
            "version": "v0.9.1",
            "updateDataModel": {
                "surfaceId": "main",
                "path": "/form/name",
                "value": "Ada"
            }
        }),
    ]);
    let cases = vec![
        ("valid_reduces_and_namespaces", valid, None),
        (
            "unsupported_version",
            stream(&[
                json!({"version": "v0.9", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
            ]),
            Some(A2UIValidationCode::UnsupportedVersion),
        ),
        (
            "ordering",
            stream(&[
                json!({"version": "v0.9.1", "updateDataModel": {"surfaceId": "main", "path": "/", "value": {}}}),
            ]),
            Some(A2UIValidationCode::InvalidOrdering),
        ),
        (
            "single_surface_protocol_subset",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "secondary", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
            ]),
            Some(A2UIValidationCode::BoundsExceeded),
        ),
        (
            "reference_and_root",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Card", "child": "missing"}]}}),
            ]),
            Some(A2UIValidationCode::InvalidReference),
        ),
        (
            "unreachable_action",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [
                    {"id": "root", "component": "Text", "text": "Visible"},
                    {"id": "hidden", "component": "Button", "child": "hidden_label", "action": {"event": {"name": "hidden"}}},
                    {"id": "hidden_label", "component": "Text", "text": "Hidden"}
                ]}}),
            ]),
            Some(A2UIValidationCode::InvalidReference),
        ),
        (
            "interactive_input_requires_binding",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID, "sendDataModel": true}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [
                    {"id": "field", "component": "TextField", "label": "Name", "value": "Ada"},
                    {"id": "submit", "component": "Button", "child": "label", "action": {"event": {"name": "submit"}}},
                    {"id": "label", "component": "Text", "text": "Save"},
                    {"id": "root", "component": "Column", "children": ["field", "submit"]}
                ]}}),
            ]),
            Some(A2UIValidationCode::InvalidDataPath),
        ),
        (
            "interactive_input_requires_synchronized_model",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [
                    {"id": "field", "component": "TextField", "label": "Name", "value": {"path": "/form/name"}},
                    {"id": "submit", "component": "Button", "child": "label", "action": {"event": {"name": "submit"}}},
                    {"id": "label", "component": "Text", "text": "Save"},
                    {"id": "root", "component": "Column", "children": ["field", "submit"]}
                ]}}),
            ]),
            Some(A2UIValidationCode::InvalidProtocol),
        ),
        (
            "unsafe_html",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Text", "text": "<script>alert(1)</script>"}]}}),
            ]),
            Some(A2UIValidationCode::UnsafeContent),
        ),
        (
            "client_function",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Button", "child": "label", "action": {"functionCall": {"call": "openUrl"}}}, {"id": "label", "component": "Text", "text": "Go"}]}}),
            ]),
            Some(A2UIValidationCode::UnsafeContent),
        ),
        (
            "unsupported_media",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Image", "url": "https://example.test/a.png"}]}}),
            ]),
            Some(A2UIValidationCode::UnsupportedCatalog),
        ),
        (
            "invalid_path",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Text", "text": "x"}]}}),
                json!({"version": "v0.9.1", "updateDataModel": {"surfaceId": "main", "path": "/bad~2path", "value": true}}),
            ]),
            Some(A2UIValidationCode::InvalidDataPath),
        ),
        (
            "catalog_required_field",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Text"}]}}),
            ]),
            Some(A2UIValidationCode::InvalidComponent),
        ),
        (
            "catalog_dynamic_type",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "CheckBox", "label": "Ready", "value": []}]}}),
            ]),
            Some(A2UIValidationCode::InvalidComponent),
        ),
        (
            "catalog_unknown_field",
            stream(&[
                json!({"version": "v0.9.1", "createSurface": {"surfaceId": "main", "catalogId": NOEMA_A2UI_CATALOG_ID}}),
                json!({"version": "v0.9.1", "updateComponents": {"surfaceId": "main", "components": [{"id": "root", "component": "Divider", "extra": true}]}}),
            ]),
            Some(A2UIValidationCode::InvalidProtocol),
        ),
    ];

    for (name, jsonl, expected_error) in cases {
        let result = parse_and_reduce("conversation/one", &jsonl);
        match expected_error {
            None => {
                let batch = result.unwrap_or_else(|error| panic!("{name}: {error:?}"));
                let surface = batch.surfaces.values().next().expect("active surface");
                assert_eq!(surface.surface_id, "main", "{name}");
                assert!(surface.namespaced_surface_id.contains("conversation%2Fone"));
                assert_eq!(surface.data_model["form"]["name"], "Ada");
                assert_eq!(surface.actions[0].name, "submit");
            }
            Some(code) => {
                let error = result.expect_err(name);
                assert_eq!(error.status, "VALIDATION_FAILED", "{name}");
                assert_eq!(error.code, code, "{name}");
            }
        }
    }
}
