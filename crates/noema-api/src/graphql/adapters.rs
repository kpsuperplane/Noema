//! Thin GraphQL review surface for filesystem-canonical adapter definitions.

use async_graphql::{InputObject, SimpleObject};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use noema_capability_adapters::{
    AdapterCompiler, AdapterConnectionStore, AdapterDefinitionStore, AdapterOperation,
    AdmissionMode, AuthenticationMode, CredentialImportKind, OperationEffect,
    StoredAdapterDefinition,
};
use std::collections::{BTreeMap, BTreeSet};

use super::GraphqlState;

const MAX_CLIENT_JSON_BYTES: usize = 32 * 1024;
const MAX_CLIENT_JSON_BASE64_BYTES: usize = 44 * 1024;

/// One exact adapter operation proposed for human review.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOperation")]
pub struct GraphqlAdapterOperation {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub effect: String,
    pub admission: String,
    pub argument_names: Vec<String>,
}

/// One filesystem-canonical adapter definition safe to show in Settings.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterDefinition")]
pub struct GraphqlAdapterDefinition {
    pub semantic_digest: String,
    pub definition_id: String,
    pub adapter_id: String,
    pub display_name: String,
    pub definition_revision: String,
    pub source_reference: String,
    pub origin: String,
    pub authentication_mode: String,
    pub scopes: Vec<String>,
    pub operations: Vec<GraphqlAdapterOperation>,
    pub manifest_json: String,
    pub accepts_oauth_client_json: bool,
    pub connection_count: i32,
    pub reviewed: bool,
    pub superseded: bool,
}

/// Exact immutable pending definition selected by the local human.
#[derive(Clone, InputObject)]
#[graphql(name = "ApproveAdapterDefinitionInput")]
pub struct GraphqlApproveAdapterDefinitionInput {
    pub semantic_digest: String,
}

/// One transient, human-selected OAuth client document for an exact definition.
#[derive(Clone, InputObject)]
#[graphql(name = "ImportAdapterOauthClientJsonInput")]
pub struct GraphqlImportAdapterOauthClientJsonInput {
    pub semantic_digest: String,
    pub client_json_base64: String,
}

pub(super) async fn adapter_definitions(
    state: &GraphqlState,
) -> async_graphql::Result<Vec<GraphqlAdapterDefinition>> {
    let store = AdapterDefinitionStore::new(state.noema_paths()?.clone());
    let scan = store
        .scan()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    let connections = AdapterConnectionStore::new(state.noema_paths()?.clone())
        .scan(&scan.definitions)
        .map_err(|_| async_graphql::Error::new("adapter connections are unavailable"))?;
    let adapter_ids = scan
        .definitions
        .iter()
        .map(|definition| {
            (
                definition.compiled.semantic_digest.as_str(),
                definition.compiled.adapter_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let connection_counts = connections.connections.iter().fold(
        BTreeMap::<&str, i32>::new(),
        |mut counts, connection| {
            if let Some(adapter_id) =
                adapter_ids.get(connection.descriptor.semantic_digest.as_str())
            {
                *counts.entry(*adapter_id).or_default() += 1;
            }
            counts
        },
    );
    let superseded = superseded_draft_digests(&store, &scan.definitions)?;
    let mut definitions = scan
        .definitions
        .iter()
        .map(|install| {
            let digest = install.compiled.semantic_digest.as_str();
            let stored = store
                .load(digest)
                .map_err(|_| async_graphql::Error::new("adapter definition is unavailable"))?;
            Ok(definition_view(
                digest,
                &stored,
                superseded.contains(digest),
                connection_counts
                    .get(stored.manifest.adapter_id.as_str())
                    .copied()
                    .unwrap_or_default(),
            ))
        })
        .collect::<async_graphql::Result<Vec<_>>>()?;
    definitions.sort_by(|left, right| {
        left.reviewed
            .cmp(&right.reviewed)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.semantic_digest.cmp(&right.semantic_digest))
    });
    Ok(definitions)
}

pub(super) async fn import_adapter_oauth_client_json(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlImportAdapterOauthClientJsonInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new(
            "adapter credential import is unauthorized",
        ));
    }
    if input.client_json_base64.is_empty()
        || input.client_json_base64.len() > MAX_CLIENT_JSON_BASE64_BYTES
    {
        return Err(async_graphql::Error::new(
            "OAuth client JSON is invalid or too large",
        ));
    }
    let bytes = BASE64_STANDARD
        .decode(input.client_json_base64.as_bytes())
        .map_err(|_| async_graphql::Error::new("OAuth client JSON is invalid or too large"))?;
    if bytes.is_empty() || bytes.len() > MAX_CLIENT_JSON_BYTES {
        return Err(async_graphql::Error::new(
            "OAuth client JSON is invalid or too large",
        ));
    }
    state
        .adapter_operations()?
        .import_oauth_client_json(&input.semantic_digest, &bytes)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let definition_store = AdapterDefinitionStore::new(state.noema_paths()?.clone());
    let definitions = definition_store
        .scan()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    let connections = AdapterConnectionStore::new(state.noema_paths()?.clone())
        .scan(&definitions.definitions)
        .map_err(|_| async_graphql::Error::new("adapter connections are unavailable"))?;
    state
        .store()?
        .reconcile_adapter_connections(&connections.projections())
        .await
        .map_err(|_| async_graphql::Error::new("adapter connection index could not be updated"))?;
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| definition.semantic_digest == input.semantic_digest)
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
}

pub(super) async fn approve_adapter_definition(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlApproveAdapterDefinitionInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new("adapter review is unauthorized"));
    }
    let store = AdapterDefinitionStore::new(state.noema_paths()?.clone());
    let stored = store
        .load(&input.semantic_digest)
        .map_err(|_| async_graphql::Error::new("pending adapter definition was not found"))?;
    if stored.manifest.reviewed {
        return Err(async_graphql::Error::new(
            "adapter definition is already reviewed",
        ));
    }
    let mut reviewed = stored.manifest.clone();
    reviewed.reviewed = true;
    let source = stored
        .source
        .as_ref()
        .map(|(bytes, extension)| (bytes.as_slice(), extension.as_str()));
    let installed = store
        .install(
            &reviewed,
            &stored.provenance.source_reference,
            stored.provenance.imported_at.as_deref(),
            source,
        )
        .map_err(|_| async_graphql::Error::new("adapter definition could not be approved"))?;
    let scan = store
        .scan()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    state
        .store()?
        .reconcile_adapter_definitions(&scan.projections())
        .await
        .map_err(|_| async_graphql::Error::new("adapter definition index could not be updated"))?;
    let reviewed_stored = StoredAdapterDefinition {
        manifest: reviewed,
        provenance: stored.provenance,
        source: stored.source,
    };
    Ok(definition_view(
        installed.compiled.semantic_digest.as_str(),
        &reviewed_stored,
        false,
        0,
    ))
}

fn superseded_draft_digests(
    store: &AdapterDefinitionStore,
    definitions: &[noema_capability_adapters::DefinitionInstall],
) -> async_graphql::Result<BTreeSet<String>> {
    definitions
        .iter()
        .filter(|definition| definition.compiled.reviewed)
        .map(|definition| {
            let mut manifest = store
                .load(definition.compiled.semantic_digest.as_str())
                .map_err(|_| async_graphql::Error::new("adapter definition is unavailable"))?
                .manifest;
            manifest.reviewed = false;
            AdapterCompiler::compile(&manifest)
                .map(|compiled| compiled.semantic_digest.to_string())
                .map_err(|_| async_graphql::Error::new("adapter definition is unavailable"))
        })
        .collect()
}

fn definition_view(
    semantic_digest: &str,
    stored: &StoredAdapterDefinition,
    superseded: bool,
    connection_count: i32,
) -> GraphqlAdapterDefinition {
    let manifest = &stored.manifest;
    GraphqlAdapterDefinition {
        semantic_digest: semantic_digest.to_string(),
        definition_id: manifest.definition_id.clone(),
        adapter_id: manifest.adapter_id.clone(),
        display_name: manifest
            .display_name
            .clone()
            .unwrap_or_else(|| humanize_adapter_id(&manifest.adapter_id)),
        definition_revision: manifest.definition_revision.clone(),
        source_reference: stored.provenance.source_reference.clone(),
        origin: manifest.origin.clone(),
        authentication_mode: authentication_label(manifest.authentication.mode).to_string(),
        scopes: manifest.authentication.scopes.clone(),
        operations: manifest.operations.iter().map(operation_view).collect(),
        manifest_json: serde_json::to_string_pretty(manifest)
            .unwrap_or_else(|_| "adapter definition could not be displayed".to_string()),
        accepts_oauth_client_json: manifest
            .authentication
            .credential_import
            .as_ref()
            .is_some_and(|schema| schema.kind == CredentialImportKind::OauthClientJson),
        connection_count,
        reviewed: manifest.reviewed,
        superseded,
    }
}

fn humanize_adapter_id(adapter_id: &str) -> String {
    let words = adapter_id
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let mut label = words.join(" ");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}

fn operation_view(operation: &AdapterOperation) -> GraphqlAdapterOperation {
    GraphqlAdapterOperation {
        operation_id: operation.operation_id.clone(),
        method: format!("{:?}", operation.method).to_ascii_uppercase(),
        path: operation.path.clone(),
        effect: match operation.effect {
            OperationEffect::ReadOnly => "read_only",
            OperationEffect::ExternalWrite => "external_write",
            OperationEffect::ExternalExport => "external_export",
            OperationEffect::ExternalWriteAndExport => "external_write_and_export",
        }
        .to_string(),
        admission: match operation.admission {
            AdmissionMode::Direct => "direct",
            AdmissionMode::ReviewerMayApprove => "reviewer_may_approve",
            AdmissionMode::AlwaysAsk => "always_ask",
        }
        .to_string(),
        argument_names: operation
            .arguments
            .iter()
            .map(|argument| argument.name.clone())
            .collect(),
    }
}

const fn authentication_label(mode: AuthenticationMode) -> &'static str {
    match mode {
        AuthenticationMode::None => "none",
        AuthenticationMode::StaticBearer => "static_bearer",
        AuthenticationMode::Oauth2AuthorizationCodePkce => "oauth2_authorization_code_pkce",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capability_adapters::AdapterManifestV1;
    use noema_home::NoemaPaths;
    use serde_json::json;

    fn pending_manifest() -> AdapterManifestV1 {
        serde_json::from_value(json!({
            "schema_version": 1,
            "definition_id": "definition:review_fixture",
            "adapter_id": "review_fixture",
            "display_name": "Review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {"mode": "none", "scopes": []},
            "provider_data_policy": {"retention_allowed": true, "deletion_supported": true},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_items",
                "method": "GET",
                "path": "/v1/items",
                "effect": "read_only",
                "admission": "direct",
                "result": {
                    "classification": "public",
                    "model_route": "any_known_route",
                    "model_payload": "full",
                    "provider_retention": "allow",
                    "persistence": "redacted"
                },
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            }]
        }))
        .expect("manifest")
    }

    fn oauth_pending_manifest() -> AdapterManifestV1 {
        serde_json::from_value(json!({
            "schema_version": 1,
            "definition_id": "definition:oauth_review_fixture",
            "adapter_id": "oauth_review_fixture",
            "display_name": "OAuth review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {
                "mode": "oauth2_authorization_code_pkce",
                "scopes": ["https://scope.example.test/read"],
                "credential_import": {
                    "kind": "oauth_client_json",
                    "alternatives": [{
                        "client_id_pointer": "/installed/client_id",
                        "client_secret_pointer": "/installed/client_secret"
                    }]
                },
                "oauth2": {
                    "authorization_endpoint": "https://accounts.example.test/authorize",
                    "token_endpoint": "https://accounts.example.test/token",
                    "client_authentication": "client_secret_post",
                    "callback_modes": ["loopback"],
                    "extra_authorization_parameters": {}
                }
            },
            "provider_data_policy": {"retention_allowed": true, "deletion_supported": true},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_items",
                "method": "GET",
                "path": "/v1/items",
                "effect": "read_only",
                "admission": "direct",
                "result": {
                    "classification": "private",
                    "model_route": "local_only",
                    "model_payload": "full",
                    "provider_retention": "deny",
                    "persistence": "omit"
                },
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"}
            }]
        }))
        .expect("OAuth manifest")
    }

    async fn fixture() -> (crate::test_support::TestEnvironment, GraphqlState, String) {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let pending = AdapterDefinitionStore::new(paths)
            .install(
                &pending_manifest(),
                "https://developers.example.test/api",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment.clone());
        (
            environment,
            state,
            pending.compiled.semantic_digest.to_string(),
        )
    }

    #[tokio::test]
    async fn approval_publishes_reviewed_definition_and_reconciles_projection() {
        let (environment, state, pending_digest) = fixture().await;
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending_digest.clone(),
            },
        )
        .await
        .expect("approve");
        assert!(reviewed.reviewed);
        assert_ne!(reviewed.semantic_digest, pending_digest);

        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let scan = AdapterDefinitionStore::new(paths).scan().expect("scan");
        assert_eq!(scan.definitions.len(), 2);
        assert_eq!(
            scan.definitions
                .iter()
                .filter(|definition| definition.compiled.reviewed)
                .count(),
            1
        );
        let rows = state
            .store()
            .expect("store")
            .adapter_definitions()
            .await
            .expect("projection");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.review_status == "reviewed"));
    }

    #[tokio::test]
    async fn approval_rejects_unknown_or_already_reviewed_authority() {
        let (_environment, state, pending_digest) = fixture().await;
        assert!(
            approve_adapter_definition(
                &state,
                "human:local",
                GraphqlApproveAdapterDefinitionInput {
                    semantic_digest: "0".repeat(64),
                },
            )
            .await
            .is_err()
        );
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending_digest,
            },
        )
        .await
        .expect("approve");
        assert!(
            approve_adapter_definition(
                &state,
                "human:local",
                GraphqlApproveAdapterDefinitionInput {
                    semantic_digest: reviewed.semantic_digest,
                },
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn client_json_import_publishes_only_extracted_metadata_once() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let pending = AdapterDefinitionStore::new(paths.clone())
            .install(
                &oauth_pending_manifest(),
                "https://developers.example.test/oauth",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment.clone());
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        let upload = br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker","discard":"raw-upload-marker"}}"#;
        let imported = import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: reviewed.semantic_digest.clone(),
                client_json_base64: BASE64_STANDARD.encode(upload),
            },
        )
        .await
        .expect("import");
        assert_eq!(imported.connection_count, 1);

        let definitions = AdapterDefinitionStore::new(paths.clone())
            .scan()
            .expect("definitions");
        let connections = AdapterConnectionStore::new(paths)
            .scan(&definitions.definitions)
            .expect("connections");
        let projection = &connections.connections[0].projection;
        let credential_path = environment.root().join(
            projection
                .credential_relative_path
                .as_deref()
                .expect("credential path"),
        );
        let stored = std::fs::read_to_string(credential_path).expect("credential");
        assert!(stored.contains("client-marker"));
        assert!(stored.contains("secret-marker"));
        assert!(!stored.contains("raw-upload-marker"));
        assert!(
            import_adapter_oauth_client_json(
                &state,
                "human:local",
                GraphqlImportAdapterOauthClientJsonInput {
                    semantic_digest: reviewed.semantic_digest,
                    client_json_base64: BASE64_STANDARD.encode(upload),
                },
            )
            .await
            .is_err()
        );
    }
}
