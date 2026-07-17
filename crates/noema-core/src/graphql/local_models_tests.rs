use async_graphql::Request;
use serde_json::json;

use super::{GraphqlState, build_schema};

#[tokio::test]
async fn catalog_query_exposes_bundled_model_and_machine_fit() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema
        .execute(Request::new(
            r#"
            {
              localModelCatalog {
                modelId
                name
                license
                priority
                selectedBuild { file downloadGb backends }
                compatibleBackend
                hardwareFit { backend ramGb vramGb unifiedMemory explanation }
                isRecommended
              }
            }
            "#,
        ))
        .await
        .into_result()
        .expect("catalog query");
    let data = response.data.into_json().expect("catalog json");
    let catalog = data["localModelCatalog"].as_array().expect("catalog");

    assert_eq!(catalog.len(), 1);
    assert_eq!(
        catalog
            .iter()
            .map(|model| model["modelId"].as_str().expect("model id"))
            .collect::<Vec<_>>(),
        vec!["gemma-4-e4b-it"]
    );
    assert_eq!(catalog[0]["name"], "Gemma 4 E4B IT");
    assert_eq!(catalog[0]["license"], "Apache-2.0");
    assert_eq!(catalog[0]["priority"], 100);

    let recommended = catalog
        .iter()
        .filter(|model| model["isRecommended"] == true)
        .collect::<Vec<_>>();
    assert!(recommended.len() <= 1);
    if let Some(model) = recommended.first() {
        assert!(model["selectedBuild"].is_object());
        assert!(
            model["hardwareFit"]["explanation"]
                .as_str()
                .is_some_and(|explanation| explanation.contains("Recommended because"))
        );
    }
}

#[tokio::test]
async fn default_preference_mutation_is_provider_neutral() {
    let store = crate::test_support::test_store().await;
    store
        .ensure_default_provider_account()
        .await
        .expect("Codex provider account");
    let schema = build_schema(GraphqlState::for_tests_with_store(store));

    let response = schema
        .execute(Request::new(
            r#"
            mutation {
              saveDefaultModelPreference(input: {
                providerKind: "codex"
                providerAccountId: "provider_account:codex:default"
                modelProfile: "gpt-5.6-luna"
                reasoningEffort: "high"
              }) {
                providerKind
                providerAccountId
                modelProfile
                reasoningEffort
              }
            }
            "#,
        ))
        .await
        .into_result()
        .expect("save default mutation");

    assert_eq!(
        response.data.into_json().expect("mutation json"),
        json!({
            "saveDefaultModelPreference": {
                "providerKind": "codex",
                "providerAccountId": "provider_account:codex:default",
                "modelProfile": "gpt-5.6-luna",
                "reasoningEffort": "high"
            }
        })
    );
}
