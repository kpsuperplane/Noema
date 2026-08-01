//! GraphQL projections for paired client credentials.

use async_graphql::{Result, SimpleObject};

use super::{RequestPrincipal, errors::graphql_error, schema::GraphqlState};

/// Paired client metadata, with no credential material.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Client")]
pub struct GraphqlClient {
    /// Opaque identifier carried by the client bearer credential.
    pub client_id: String,
    /// Human-visible client label.
    pub display_name: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Revocation timestamp, when this credential is no longer accepted.
    pub revoked_at: Option<String>,
    /// Whether this row represents the credential used by the current request.
    pub is_current: bool,
}

impl GraphqlClient {
    fn from_record(record: noema_store::ClientRecord, principal: &RequestPrincipal) -> Self {
        let is_current = principal.client_id() == Some(record.client_id.as_str());
        Self {
            client_id: record.client_id,
            display_name: record.display_name,
            created_at: record.created_at,
            revoked_at: record.revoked_at,
            is_current,
        }
    }
}

pub(super) async fn clients(
    state: &GraphqlState,
    principal: &RequestPrincipal,
) -> Result<Vec<GraphqlClient>> {
    let records = state
        .store()?
        .list_clients(principal.subject_id())
        .await
        .map_err(graphql_error)?;
    Ok(records
        .into_iter()
        .map(|record| GraphqlClient::from_record(record, principal))
        .collect())
}

pub(super) async fn revoke_client(
    state: &GraphqlState,
    principal: &RequestPrincipal,
    client_id: String,
) -> Result<GraphqlClient> {
    let client = state
        .store()?
        .revoke_client(principal.subject_id(), client_id.trim())
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("client is unavailable"))?;
    Ok(GraphqlClient::from_record(client, principal))
}

#[cfg(test)]
mod tests {
    use crate::graphql::{GraphqlState, RequestPrincipal, build_schema};

    #[tokio::test]
    async fn client_graphql_lists_and_identifies_the_current_revocation() {
        let store = crate::test_support::test_store().await;
        store
            .insert_client("client-one", "human:local", "Test client", [4_u8; 32])
            .await
            .expect("insert client");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let principal = RequestPrincipal::client("client-one");
        let list = schema
            .execute(
                async_graphql::Request::new(
                    "{ clients { clientId displayName isCurrent revokedAt } }",
                )
                .data(principal.clone()),
            )
            .await;
        assert!(list.errors.is_empty(), "{:?}", list.errors);
        assert_eq!(
            list.data.into_json().expect("list JSON")["clients"][0]["isCurrent"],
            true
        );

        let revoked = schema
            .execute(
                async_graphql::Request::new(
                    "mutation { revokeClient(clientId: \"client-one\") { clientId isCurrent revokedAt } }",
                )
                .data(principal),
            )
            .await;
        assert!(revoked.errors.is_empty(), "{:?}", revoked.errors);
        let payload = revoked.data.into_json().expect("revocation JSON")["revokeClient"].clone();
        assert_eq!(payload["clientId"], "client-one");
        assert_eq!(payload["isCurrent"], true);
        assert!(payload["revokedAt"].as_str().is_some());
    }
}
