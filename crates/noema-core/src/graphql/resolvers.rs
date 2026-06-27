#[cfg(test)]
mod tests {
    use async_graphql::{Request, Value};

    use crate::graphql::{GraphqlState, build_schema};

    #[tokio::test]
    async fn local_status_query_returns_running_codex() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(Request::new(
                "{ localStatus { localService assistantConnection memoryStorage } }",
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
                    "memoryStorage": "READY"
                }
            }))
            .expect("valid json")
        );
    }
}
