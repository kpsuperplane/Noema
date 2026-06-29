#[cfg(test)]
mod tests {
    use async_graphql::{Request, Value};

    use crate::graphql::{GraphqlState, build_schema};

    #[tokio::test]
    async fn local_status_query_returns_running_codex() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(Request::new(
                "{ localStatus { localService assistantConnection memoryStorage primaryAgentDisplayName } }",
            ))
            .await
            .into_result()
            .expect("query should succeed");

        assert_eq!(
            response.data,
            Value::from_json(serde_json::json!({
                "localStatus": {
                    "localService": "RUNNING",
                    "assistantConnection": "CODEX",
                    "memoryStorage": "READY",
                    "primaryAgentDisplayName": null
                }
            }))
            .expect("valid json")
        );
    }

    #[tokio::test]
    async fn local_status_query_returns_primary_agent_display_name() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .update_agent_display_name("agent:primary", "Fred")
            .await
            .expect("agent name");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(Request::new(
                "{ localStatus { localService assistantConnection memoryStorage primaryAgentDisplayName } }",
            ))
            .await
            .into_result()
            .expect("query should succeed");

        assert_eq!(
            response.data,
            Value::from_json(serde_json::json!({
                "localStatus": {
                    "localService": "RUNNING",
                    "assistantConnection": "CODEX",
                    "memoryStorage": "READY",
                    "primaryAgentDisplayName": "Fred"
                }
            }))
            .expect("valid json")
        );
    }
}
