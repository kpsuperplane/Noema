//! GraphQL HTTP transport for CLI requests.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// JSON body sent to the Noema GraphQL endpoint.
#[derive(Debug, Serialize)]
pub(crate) struct GraphqlRequest {
    pub(crate) query: &'static str,
    pub(crate) variables: Value,
}

impl GraphqlRequest {
    /// Build a GraphQL request body.
    pub(crate) const fn new(query: &'static str, variables: Value) -> Self {
        Self { query, variables }
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

/// Execute a GraphQL request and return the decoded `data` payload.
pub(crate) async fn execute<T: DeserializeOwned>(
    base_url: &str,
    request: GraphqlRequest,
) -> Result<T, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/graphql"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let body = response
        .json::<GraphqlResponse<T>>()
        .await
        .map_err(|error| error.to_string())?;

    if let Some(error) = body.errors.and_then(|errors| errors.into_iter().next()) {
        return Err(error.message);
    }

    body.data
        .ok_or_else(|| "GraphQL response did not include data".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_body_includes_query_and_variables() {
        let body = GraphqlRequest::new(
            "query Ping { localStatus { localService } }",
            serde_json::json!({}),
        );

        assert!(body.query.contains("localStatus"));
        assert_eq!(body.variables, serde_json::json!({}));
    }
}
