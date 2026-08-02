//! Thin GraphQL review surface for filesystem-canonical adapter definitions.

use async_graphql::{InputObject, SimpleObject};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use noema_capability_adapters::{
    AdapterConnectionRevisions, AdapterConnectionStore, AdapterDefinitionStore, AdapterOperation,
    AuthenticationMode, AuthenticationSchemeV4, CredentialInput, CredentialSetup, LuauTransform,
    Oauth2CallbackMode, ResponseTransform, StoredAdapterDefinition,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::GraphqlState;

const MAX_CREDENTIAL_DOCUMENT_BYTES: usize = 128 * 1024;
const MAX_CREDENTIAL_DOCUMENT_BASE64_BYTES: usize = 176 * 1024;
const MAX_CREDENTIAL_FIELDS: usize = 16;
const MAX_CREDENTIAL_VALUE_BYTES: usize = 16 * 1024;

/// One exact adapter operation proposed for human review.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOperation")]
pub struct GraphqlAdapterOperation {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub read_only: Option<bool>,
    pub idempotent: Option<bool>,
    pub destructive: Option<bool>,
    pub open_world: Option<bool>,
    pub argument_names: Vec<String>,
    pub response_transform: Option<GraphqlAdapterResponseTransform>,
}

/// Exact reviewed response transform safe to disclose before activation.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterResponseTransform")]
pub struct GraphqlAdapterResponseTransform {
    pub language: String,
    pub source_digest: String,
    pub source: String,
    pub accepted_content_types: Vec<String>,
    pub output_schema_json: String,
}

/// One write-only provider credential field.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterCredentialField")]
pub struct GraphqlAdapterCredentialField {
    pub field_id: String,
    pub label: String,
}

/// Exact reviewed Luau safe to disclose under technical details.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterCredentialTransform")]
pub struct GraphqlAdapterCredentialTransform {
    pub language: String,
    pub source_digest: String,
    pub source: String,
}

/// Active provider credential setup selected for this Noema serving mode.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterCredentialSetup")]
pub struct GraphqlAdapterCredentialSetup {
    pub credential_type: String,
    pub setup_url: String,
    pub instructions: Vec<String>,
    pub input_kind: String,
    pub fields: Vec<GraphqlAdapterCredentialField>,
    pub document_media_type: Option<String>,
    pub redirect_uri: Option<String>,
    pub normalization_transform: Option<GraphqlAdapterCredentialTransform>,
    pub request_auth_transform: Option<GraphqlAdapterCredentialTransform>,
}

/// One non-secret filesystem connection for an exact adapter definition.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterConnection")]
pub struct GraphqlAdapterConnection {
    pub connection_id: String,
    pub status: String,
    pub account_kind: String,
    pub connection_revision: u64,
    pub credential_revision: u64,
    pub grant_revision: u64,
    pub policy_revision: u64,
    pub granted_scopes: Vec<String>,
    pub allowed_operations: Vec<String>,
    pub policy_configured: bool,
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
    pub credential_setup: Option<GraphqlAdapterCredentialSetup>,
    pub account_identity_operation_id: Option<String>,
    pub operations: Vec<GraphqlAdapterOperation>,
    pub manifest_json: String,
    pub connection_count: i32,
    pub connections: Vec<GraphqlAdapterConnection>,
    pub reviewed: bool,
    pub superseded: bool,
}

/// Exact immutable pending definition selected by the local human.
#[derive(Clone, InputObject)]
#[graphql(name = "ApproveAdapterDefinitionInput")]
pub struct GraphqlApproveAdapterDefinitionInput {
    pub semantic_digest: String,
}

/// Exact current pending definition abandoned by the local human.
#[derive(Clone, InputObject)]
#[graphql(name = "CancelAdapterDefinitionInput")]
pub struct GraphqlCancelAdapterDefinitionInput {
    pub semantic_digest: String,
}

/// One write-only credential field value.
#[derive(Clone, InputObject)]
#[graphql(name = "AdapterCredentialFieldValueInput")]
pub struct GraphqlAdapterCredentialFieldValueInput {
    pub field_id: String,
    pub value: String,
}

/// Transient credential input for one exact reviewed definition.
#[derive(Clone, InputObject)]
#[graphql(name = "SetupAdapterConnectionInput")]
pub struct GraphqlSetupAdapterConnectionInput {
    pub semantic_digest: String,
    #[graphql(default)]
    pub field_values: Vec<GraphqlAdapterCredentialFieldValueInput>,
    pub document_base64: Option<String>,
}

#[cfg(test)]
struct GraphqlImportAdapterOauthClientJsonInput {
    semantic_digest: String,
    client_json_base64: String,
}

/// Delete one exact filesystem-canonical adapter connection revision.
#[derive(Clone, InputObject)]
#[graphql(name = "DeleteAdapterConnectionInput")]
pub struct GraphqlDeleteAdapterConnectionInput {
    pub connection_id: String,
    pub expected_connection_revision: u64,
}

/// Delete one exact adapter service family selection.
#[derive(Clone, InputObject)]
#[graphql(name = "DeleteAdapterServiceInput")]
pub struct GraphqlDeleteAdapterServiceInput {
    pub definition_id: String,
    pub expected_source_revision: String,
}

/// Start browser OAuth against one exact filesystem connection revision.
#[derive(Clone, InputObject)]
#[graphql(name = "StartAdapterOauthSetupInput")]
pub struct GraphqlStartAdapterOauthSetupInput {
    pub connection_id: String,
    pub expected_connection_revision: u64,
    pub expected_credential_revision: u64,
    pub expected_grant_revision: u64,
    pub expected_policy_revision: u64,
}

/// Opaque browser handoff for a provider-neutral adapter OAuth attempt.
#[derive(Clone, SimpleObject)]
#[graphql(name = "AdapterOauthSetupAttempt")]
pub struct GraphqlAdapterOauthSetupAttempt {
    pub attempt_id: String,
    pub authorization_url: String,
    pub expires_at_epoch_seconds: u64,
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
    let mut connections_by_digest = BTreeMap::<String, Vec<GraphqlAdapterConnection>>::new();
    for connection in connections.connections {
        connections_by_digest
            .entry(connection.descriptor.semantic_digest.clone())
            .or_default()
            .push(connection_view(&connection.descriptor));
    }
    for connections in connections_by_digest.values_mut() {
        connections.sort_by(|left, right| left.connection_id.cmp(&right.connection_id));
    }
    let superseded = store
        .superseded_pending_digests(&scan)
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    let oauth_callback = state
        .adapter_oauth_callback_url()
        .ok()
        .and_then(|url| adapter_callback_mode(url).ok().map(|mode| (url, mode)));
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
                connections_by_digest
                    .get(digest)
                    .cloned()
                    .unwrap_or_default(),
                oauth_callback,
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

pub(super) async fn start_adapter_oauth_setup(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlStartAdapterOauthSetupInput,
) -> async_graphql::Result<GraphqlAdapterOauthSetupAttempt> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new(
            "adapter OAuth setup is unauthorized",
        ));
    }
    let callback_url = state.adapter_oauth_callback_url()?;
    let callback_mode = adapter_callback_mode(callback_url)?;
    let attempt = state
        .adapter_operations()?
        .start_oauth_setup(
            principal,
            &input.connection_id,
            AdapterConnectionRevisions {
                connection: input.expected_connection_revision,
                credential: input.expected_credential_revision,
                grant: input.expected_grant_revision,
                policy: input.expected_policy_revision,
            },
            callback_mode,
            callback_url,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(GraphqlAdapterOauthSetupAttempt {
        attempt_id: attempt.attempt_id,
        authorization_url: attempt.authorization_url,
        expires_at_epoch_seconds: attempt.expires_at_epoch_seconds,
    })
}

/// Complete a state-bound adapter OAuth callback from a serving-shell route.
///
/// # Errors
///
/// Returns an error when the callback is not owned by this process, OAuth
/// completion fails, or the canonical filesystem state cannot be reconciled.
pub async fn complete_adapter_oauth_setup(
    state: &GraphqlState,
    callback_url: &str,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    let mut callback = url::Url::parse(callback_url).map_err(|_| {
        async_graphql::Error::new("adapter OAuth callback does not match this Noema process")
    })?;
    callback.set_query(None);
    callback.set_fragment(None);
    let expected = url::Url::parse(state.adapter_oauth_callback_url()?).map_err(|_| {
        async_graphql::Error::new("adapter OAuth callback does not match this Noema process")
    })?;
    if callback != expected {
        return Err(async_graphql::Error::new(
            "adapter OAuth callback does not match this Noema process",
        ));
    }
    let completed = state
        .adapter_operations()?
        .complete_oauth_callback(callback_url)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    let definition = adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| {
            definition.semantic_digest == completed.connection.descriptor.semantic_digest
        })
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))?;
    state
        .runtime()?
        .resume_mcp_authentication_attempt(completed.attempt_id)
        .await?;
    publish_primary_interventions_changed(state).await;
    if completed.newly_activated {
        queue_ready_adapter_setup(
            state,
            &definition.display_name,
            &completed.connection.descriptor,
        );
    }
    Ok(definition)
}

pub(super) async fn publish_primary_interventions_changed(state: &GraphqlState) {
    let Some(store) = state.optional_store() else {
        return;
    };
    let Ok(Some(conversation)) = store.primary_conversation_for_human("human:local").await else {
        return;
    };
    state.subscriptions().publish_conversation(
        noema_runtime::ConversationRuntimeEvent::HumanInterventionsChanged {
            conversation_id: conversation.conversation_id,
        },
    );
}

pub(super) fn queue_ready_adapter_setup(
    state: &GraphqlState,
    integration_name: &str,
    descriptor: &noema_capability_adapters::AdapterConnectionV3,
) {
    if descriptor.status != noema_capability_adapters::AdapterConnectionStatus::Active
        || descriptor.policy.is_none()
    {
        return;
    }
    let Some(runtime) = state.optional_runtime().cloned() else {
        return;
    };
    let completion = noema_runtime::CapabilitySetupCompletion {
        human_id: "human:local".to_string(),
        integration_kind: noema_runtime::CapabilityIntegrationKind::Api,
        integration_name: integration_name.to_string(),
        connection_id: descriptor.connection_id.clone(),
        connection_revision: descriptor.revisions.credential.to_string(),
        granted_scopes: descriptor.granted_scopes.clone(),
        enabled_tool_count: descriptor.allowed_operations.len(),
    };
    tokio::spawn(async move {
        let _ = runtime
            .narrate_capability_setup_completion(completion)
            .await;
    });
}

pub(super) async fn setup_adapter_connection(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlSetupAdapterConnectionInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new(
            "adapter credential setup is unauthorized",
        ));
    }
    if input.field_values.len() > MAX_CREDENTIAL_FIELDS {
        return Err(async_graphql::Error::new(
            "adapter credential input is invalid",
        ));
    }
    let mut field_values = BTreeMap::new();
    for field in input.field_values {
        if field.field_id.is_empty()
            || field.value.is_empty()
            || field.value.len() > MAX_CREDENTIAL_VALUE_BYTES
            || field_values.insert(field.field_id, field.value).is_some()
        {
            return Err(async_graphql::Error::new(
                "adapter credential input is invalid",
            ));
        }
    }
    let document = input
        .document_base64
        .map(|encoded| {
            if encoded.is_empty() || encoded.len() > MAX_CREDENTIAL_DOCUMENT_BASE64_BYTES {
                return Err(async_graphql::Error::new(
                    "adapter credential document is invalid or too large",
                ));
            }
            let bytes = BASE64_STANDARD.decode(encoded.as_bytes()).map_err(|_| {
                async_graphql::Error::new("adapter credential document is invalid or too large")
            })?;
            if bytes.is_empty() || bytes.len() > MAX_CREDENTIAL_DOCUMENT_BYTES {
                return Err(async_graphql::Error::new(
                    "adapter credential document is invalid or too large",
                ));
            }
            Ok(bytes)
        })
        .transpose()?;
    state
        .adapter_operations()?
        .setup_connection(&input.semantic_digest, field_values, document.as_deref())
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| definition.semantic_digest == input.semantic_digest)
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
}

#[cfg(test)]
async fn import_adapter_oauth_client_json(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlImportAdapterOauthClientJsonInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    setup_adapter_connection(
        state,
        principal,
        GraphqlSetupAdapterConnectionInput {
            semantic_digest: input.semantic_digest,
            field_values: Vec::new(),
            document_base64: Some(input.client_json_base64),
        },
    )
    .await
}

pub(super) async fn delete_adapter_connection(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlDeleteAdapterConnectionInput,
) -> async_graphql::Result<bool> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new(
            "adapter connection deletion is unauthorized",
        ));
    }
    let deleted = state
        .adapter_operations()?
        .quarantine_connection(&input.connection_id, input.expected_connection_revision)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if deleted {
        reconcile_adapter_connections(state).await?;
        if let Some(runtime) = state.optional_runtime() {
            runtime.publish_capability_authentication_origins().await?;
        }
    }
    Ok(deleted)
}

pub(super) async fn cancel_adapter_definition(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlCancelAdapterDefinitionInput,
) -> async_graphql::Result<bool> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new("adapter review is unauthorized"));
    }
    let cancelled = state
        .adapter_operations()?
        .cancel_definition_proposal(&input.semantic_digest)
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if cancelled {
        reconcile_adapter_definitions(state).await?;
        publish_primary_interventions_changed(state).await;
    }
    Ok(cancelled)
}

pub(super) async fn delete_adapter_service(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlDeleteAdapterServiceInput,
) -> async_graphql::Result<bool> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new(
            "adapter service deletion is unauthorized",
        ));
    }
    let deleted = state
        .adapter_operations()?
        .quarantine_definition_family(&input.definition_id, &input.expected_source_revision)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if deleted {
        reconcile_adapter_definitions(state).await?;
    }
    Ok(deleted)
}

pub(super) async fn reconcile_adapter_connections(
    state: &GraphqlState,
) -> async_graphql::Result<()> {
    let definitions = AdapterDefinitionStore::new(state.noema_paths()?.clone())
        .scan()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    let connections = AdapterConnectionStore::new(state.noema_paths()?.clone())
        .scan(&definitions.definitions)
        .map_err(|_| async_graphql::Error::new("adapter connections are unavailable"))?;
    state
        .store()?
        .reconcile_adapter_connections(&connections.projections())
        .await
        .map_err(|_| async_graphql::Error::new("adapter connection index could not be updated"))
}

async fn reconcile_adapter_definitions(state: &GraphqlState) -> async_graphql::Result<()> {
    let scan = AdapterDefinitionStore::new(state.noema_paths()?.clone())
        .scan()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    state
        .store()?
        .reconcile_adapter_definitions(&scan.projections())
        .await
        .map_err(|_| async_graphql::Error::new("adapter definition index could not be updated"))
}

pub(super) async fn approve_adapter_definition(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlApproveAdapterDefinitionInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    if principal != "human:local" {
        return Err(async_graphql::Error::new("adapter review is unauthorized"));
    }
    let installed = state
        .adapter_operations()?
        .review_definition(&input.semantic_digest)
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if installed.compiled.authentication.mode() == AuthenticationMode::None {
        state
            .adapter_operations()?
            .ensure_credential_free_connection(installed.compiled.semantic_digest.as_str())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    }
    reconcile_adapter_definitions(state).await?;
    if installed.compiled.authentication.mode() == AuthenticationMode::None {
        reconcile_adapter_connections(state).await?;
    }
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| {
            definition.semantic_digest == installed.compiled.semantic_digest.as_str()
        })
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
}

fn definition_view(
    semantic_digest: &str,
    stored: &StoredAdapterDefinition,
    superseded: bool,
    connections: Vec<GraphqlAdapterConnection>,
    oauth_callback: Option<(&str, Oauth2CallbackMode)>,
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
        authentication_mode: authentication_label(manifest.authentication.mode()).to_string(),
        scopes: manifest.authentication.scopes().to_vec(),
        credential_setup: match &manifest.authentication {
            AuthenticationSchemeV4::None => None,
            AuthenticationSchemeV4::Credential(config) => Some(credential_setup_view(
                &config.setup,
                None,
                Some(&config.request_auth),
            )),
            AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(config) => {
                oauth_callback.and_then(|(url, mode)| {
                    config
                        .setups
                        .iter()
                        .find(|setup| setup.callback_mode == mode)
                        .map(|setup| credential_setup_view(&setup.setup, Some(url), None))
                })
            }
        },
        account_identity_operation_id: manifest
            .authentication
            .account_identity()
            .map(|probe| probe.operation_id.clone()),
        operations: manifest.operations.iter().map(operation_view).collect(),
        manifest_json: serde_json::to_string_pretty(manifest)
            .unwrap_or_else(|_| "adapter definition could not be displayed".to_string()),
        connection_count: i32::try_from(connections.len()).unwrap_or(i32::MAX),
        connections,
        reviewed: manifest.reviewed,
        superseded,
    }
}

fn credential_setup_view(
    setup: &CredentialSetup,
    redirect_uri: Option<&str>,
    request_auth: Option<&LuauTransform>,
) -> GraphqlAdapterCredentialSetup {
    let (input_kind, document_media_type, normalization_transform) = match &setup.input {
        CredentialInput::Fields { .. } => ("fields", None, None),
        CredentialInput::Document {
            media_type,
            normalize,
            ..
        } => (
            "document",
            Some(media_type.clone()),
            Some(credential_transform_view(normalize)),
        ),
    };
    GraphqlAdapterCredentialSetup {
        credential_type: setup.credential_type.clone(),
        setup_url: setup.setup_url.clone(),
        instructions: setup.instructions.clone(),
        input_kind: input_kind.to_string(),
        fields: setup
            .input
            .fields()
            .iter()
            .map(|field| GraphqlAdapterCredentialField {
                field_id: field.id.clone(),
                label: field.label.clone(),
            })
            .collect(),
        document_media_type,
        redirect_uri: redirect_uri.map(str::to_string),
        normalization_transform,
        request_auth_transform: request_auth.map(credential_transform_view),
    }
}

fn credential_transform_view(transform: &LuauTransform) -> GraphqlAdapterCredentialTransform {
    GraphqlAdapterCredentialTransform {
        language: "luau".to_string(),
        source_digest: sha256_hex(transform.source().as_bytes()),
        source: transform.source().to_string(),
    }
}

fn connection_view(
    descriptor: &noema_capability_adapters::AdapterConnectionV3,
) -> GraphqlAdapterConnection {
    GraphqlAdapterConnection {
        connection_id: descriptor.connection_id.clone(),
        status: descriptor.status.as_str().to_string(),
        account_kind: descriptor.account_kind.clone(),
        connection_revision: descriptor.revisions.connection,
        credential_revision: descriptor.revisions.credential,
        grant_revision: descriptor.revisions.grant,
        policy_revision: descriptor.revisions.policy,
        granted_scopes: descriptor.granted_scopes.clone(),
        allowed_operations: descriptor.allowed_operations.clone(),
        policy_configured: descriptor.policy.is_some(),
    }
}

pub(super) fn adapter_callback_mode(
    callback_url: &str,
) -> async_graphql::Result<Oauth2CallbackMode> {
    let parsed = url::Url::parse(callback_url)
        .map_err(|_| async_graphql::Error::new("Noema adapter OAuth callback is unavailable"))?;
    let loopback = matches!(parsed.host(), Some(url::Host::Domain(host)) if host.eq_ignore_ascii_case("localhost"))
        || matches!(parsed.host(), Some(url::Host::Ipv4(address)) if address.is_loopback())
        || matches!(parsed.host(), Some(url::Host::Ipv6(address)) if address.is_loopback());
    if matches!(parsed.scheme(), "http" | "https") && loopback {
        return Ok(Oauth2CallbackMode::Loopback);
    }
    if parsed.scheme() == "https" {
        return Ok(Oauth2CallbackMode::Hosted);
    }
    Err(async_graphql::Error::new(
        "Noema adapter OAuth callback is unavailable",
    ))
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
        read_only: operation.behavior.read_only.value,
        idempotent: operation.behavior.idempotent.value,
        destructive: operation.behavior.destructive.value,
        open_world: operation.behavior.open_world.value,
        argument_names: operation
            .arguments
            .iter()
            .map(|argument| argument.name.clone())
            .collect(),
        response_transform: operation.response.as_ref().map(|response| {
            let ResponseTransform::Luau { source } = &response.transform;
            GraphqlAdapterResponseTransform {
                language: "luau".to_string(),
                source_digest: sha256_hex(source.as_bytes()),
                source: source.clone(),
                accepted_content_types: response.accepted_content_types.clone(),
                output_schema_json: serde_json::to_string_pretty(&response.output_schema)
                    .unwrap_or_else(|_| "response schema could not be displayed".to_string()),
            }
        }),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in ring::digest::digest(&ring::digest::SHA256, bytes).as_ref() {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

const fn authentication_label(mode: AuthenticationMode) -> &'static str {
    match mode {
        AuthenticationMode::None => "none",
        AuthenticationMode::Credential => "credential",
        AuthenticationMode::Oauth2AuthorizationCodePkce => "oauth2_authorization_code_pkce",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noema_capability_adapters::AdapterManifestV5;
    use noema_home::NoemaPaths;
    use serde_json::json;

    fn pending_manifest() -> AdapterManifestV5 {
        serde_json::from_value(json!({
            "schema_version": 5,
            "definition_id": "definition:review_fixture",
            "adapter_id": "review_fixture",
            "display_name": "Review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {"kind": "none"},
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_items",
                "method": "GET",
                "path": "/v1/items",
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
            }]
        }))
        .expect("manifest")
    }

    fn oauth_pending_manifest() -> AdapterManifestV5 {
        serde_json::from_value(json!({
            "schema_version": 5,
            "definition_id": "definition:oauth_review_fixture",
            "adapter_id": "oauth_review_fixture",
            "display_name": "OAuth review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {
                "kind": "oauth2_authorization_code_pkce",
                "scopes": ["https://scope.example.test/read"],
                "authorization_endpoint": "https://accounts.example.test/authorize",
                "token_endpoint": "https://accounts.example.test/token",
                "client_authentication": "client_secret_post",
                "setups": [{"callback_mode": "loopback", "setup": {
                    "credential_type": "Desktop app",
                    "setup_url": "https://developers.example.test/oauth/clients/new",
                    "instructions": ["Create a Desktop app OAuth client and download its JSON."],
                    "input": {"kind": "document", "media_type": "application/json", "fields": [
                        {"id": "client_id", "label": "Client ID"}, {"id": "client_secret", "label": "Client secret"}
                    ], "normalize": {"language": "luau", "source": "return function(input) local d = json.decode(input.document) return { client_id = d.installed.client_id, client_secret = d.installed.client_secret } end"}}
                }}],
                "extra_authorization_parameters": {}
            },
            "quota": {"cost_class": "free"},
            "operations": [{
                "operation_id": "list_items",
                "method": "GET",
                "path": "/v1/items",
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
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

    async fn has_adapter_intervention(state: &GraphqlState, semantic_digest: &str) -> bool {
        crate::graphql::human_interventions::pending_human_interventions(
            state,
            "human:local",
            Some("conversation:fixture".to_string()),
            None,
            None,
            Some(50),
        )
        .await
        .expect("pending interventions")
        .iter()
        .any(|intervention| match intervention {
            crate::graphql::human_interventions::GraphqlHumanIntervention::AdapterDefinition(
                definition,
            ) => definition.semantic_digest == semantic_digest,
            _ => false,
        })
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
        assert_eq!(reviewed.connection_count, 1);
        assert_eq!(reviewed.connections[0].status, "active");
        assert!(!reviewed.connections[0].policy_configured);

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
        let connections = state
            .store()
            .expect("store")
            .adapter_connections()
            .await
            .expect("connection projections");
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].status, "active");
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
    async fn cancellation_removes_pending_review_and_projection() {
        let (environment, state, pending_digest) = fixture().await;
        assert!(has_adapter_intervention(&state, &pending_digest).await);
        assert!(
            cancel_adapter_definition(
                &state,
                "human:local",
                GraphqlCancelAdapterDefinitionInput {
                    semantic_digest: pending_digest,
                },
            )
            .await
            .expect("cancel proposal")
        );
        assert!(
            AdapterDefinitionStore::new(
                NoemaPaths::from_noema_home(environment.root()).expect("paths")
            )
            .scan()
            .expect("scan")
            .definitions
            .is_empty()
        );
        assert!(
            state
                .store()
                .expect("store")
                .adapter_definitions()
                .await
                .expect("projections")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn policyless_active_connection_remains_a_chat_intervention() {
        let (_environment, state, pending_digest) = fixture().await;
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending_digest,
            },
        )
        .await
        .expect("approve");
        let connection = &reviewed.connections[0];

        assert!(has_adapter_intervention(&state, &reviewed.semantic_digest).await);

        crate::graphql::capability_integrations::save_connection_policy(
            &state,
            crate::graphql::capability_integration_models::GraphqlSaveCapabilityConnectionPolicyInput {
                kind: crate::graphql::capability_integration_models::GraphqlCapabilityIntegrationKind::Api,
                connection_id: connection.connection_id.clone(),
                expected_connection_revision: connection.connection_revision.to_string(),
                expected_policy_revision: connection.policy_revision,
                data_sharing_policy: "allow_automatically".to_string(),
                unsafe_action_policy: "reviewer_may_approve".to_string(),
            },
        )
        .await
        .expect("save policy");
        assert!(!has_adapter_intervention(&state, &reviewed.semantic_digest).await);
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
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment.clone())
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        let setup = reviewed
            .credential_setup
            .as_ref()
            .expect("credential setup");
        assert_eq!(setup.credential_type, "Desktop app");
        assert_eq!(
            setup.setup_url,
            "https://developers.example.test/oauth/clients/new"
        );
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
        assert_eq!(imported.connections.len(), 1);
        let connection = &imported.connections[0];
        assert_eq!(connection.status, "authentication_required");
        assert_eq!(connection.connection_revision, 1);
        assert_eq!(connection.credential_revision, 1);
        assert_eq!(connection.grant_revision, 1);
        assert_eq!(connection.policy_revision, 1);
        assert_eq!(connection.allowed_operations, ["list_items"]);
        assert!(connection.granted_scopes.is_empty());
        let interventions = crate::graphql::human_interventions::pending_human_interventions(
            &state,
            "human:local",
            Some("conversation:fixture".to_string()),
            None,
            None,
            Some(50),
        )
        .await
        .expect("pending interventions");
        assert!(interventions.iter().any(|intervention| {
            matches!(
                intervention,
                crate::graphql::human_interventions::GraphqlHumanIntervention::AdapterDefinition(
                    definition
                ) if definition.semantic_digest == imported.semantic_digest
            )
        }));

        let callback_state = state
            .clone()
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        let projected = adapter_definitions(&callback_state)
            .await
            .expect("adapter projection");
        assert_eq!(
            projected
                .iter()
                .find(|definition| definition.semantic_digest == imported.semantic_digest)
                .and_then(|definition| definition.credential_setup.as_ref())
                .and_then(|setup| setup.redirect_uri.as_deref()),
            Some("http://localhost:43123/adapter/oauth/callback")
        );
        let hosted_state = state
            .clone()
            .with_adapter_oauth_callback_url("https://noema.example.test/adapter/oauth/callback");
        assert!(
            adapter_definitions(&hosted_state)
                .await
                .expect("hosted projection")
                .iter()
                .find(|definition| definition.semantic_digest == imported.semantic_digest)
                .and_then(|definition| definition.credential_setup.as_ref())
                .is_none()
        );
        let mut incomplete = AdapterDefinitionStore::new(paths.clone())
            .load(&imported.semantic_digest)
            .expect("stored definition");
        incomplete.manifest.authentication = AuthenticationSchemeV4::None;
        assert!(
            definition_view(
                &imported.semantic_digest,
                &incomplete,
                false,
                Vec::new(),
                Some((
                    "http://localhost:43123/adapter/oauth/callback",
                    Oauth2CallbackMode::Loopback,
                )),
            )
            .credential_setup
            .is_none()
        );
        let start_input = GraphqlStartAdapterOauthSetupInput {
            connection_id: connection.connection_id.clone(),
            expected_connection_revision: connection.connection_revision,
            expected_credential_revision: connection.credential_revision,
            expected_grant_revision: connection.grant_revision,
            expected_policy_revision: connection.policy_revision,
        };
        assert!(
            start_adapter_oauth_setup(&callback_state, "human:other", start_input.clone())
                .await
                .is_err()
        );
        let started =
            start_adapter_oauth_setup(&callback_state, "human:local", start_input.clone())
                .await
                .expect("start OAuth");
        let authorization_url = url::Url::parse(&started.authorization_url).expect("OAuth URL");
        assert_eq!(
            authorization_url
                .query_pairs()
                .find(|(name, _)| name == "redirect_uri")
                .map(|(_, value)| value.into_owned())
                .as_deref(),
            Some("http://localhost:43123/adapter/oauth/callback")
        );
        let mut stale_input = start_input;
        stale_input.expected_policy_revision += 1;
        assert!(
            start_adapter_oauth_setup(&callback_state, "human:local", stale_input)
                .await
                .is_err()
        );

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
        state
            .adapter_operations()
            .expect("adapter operations")
            .set_oauth_callback_mode(Oauth2CallbackMode::Loopback);
        let sibling = import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: reviewed.semantic_digest,
                client_json_base64: BASE64_STANDARD.encode(upload),
            },
        )
        .await
        .expect("sibling connection");
        assert_eq!(sibling.connection_count, 2);
    }

    #[tokio::test]
    async fn integration_projection_keeps_reviewed_authority_when_a_newer_draft_exists() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths.clone());
        let pending = definitions
            .install(
                &oauth_pending_manifest(),
                "https://developers.example.test/oauth-v1",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: reviewed.semantic_digest.clone(),
                client_json_base64: BASE64_STANDARD.encode(
                    br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
                ),
            },
        )
        .await
        .expect("import");

        let mut newer_draft = oauth_pending_manifest();
        newer_draft.definition_revision = "v2".to_string();
        newer_draft.operations[0].path = "/v2/items".to_string();
        definitions
            .install(
                &newer_draft,
                "https://developers.example.test/oauth-v2",
                None,
                None,
            )
            .expect("newer draft");

        let integrations = crate::graphql::capability_integrations::integrations(
            &state,
            crate::graphql::capability_integration_models::GraphqlCapabilityIntegrationKind::Api,
        )
        .await
        .expect("integrations");
        let integration = integrations
            .iter()
            .find(|integration| integration.definition_id == "definition:oauth_review_fixture")
            .expect("OAuth integration");
        assert!(integration.reviewed);
        assert_eq!(integration.source_revision, reviewed.semantic_digest);
        assert_eq!(integration.connections.len(), 1);
    }

    #[tokio::test]
    async fn adapter_deletion_guards_references_and_reconciles_projections() {
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
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        let imported = import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: reviewed.semantic_digest.clone(),
                client_json_base64: BASE64_STANDARD.encode(
                    br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
                ),
            },
        )
        .await
        .expect("import");
        let connection = &imported.connections[0];
        assert!(
            delete_adapter_service(
                &state,
                "human:local",
                GraphqlDeleteAdapterServiceInput {
                    definition_id: reviewed.definition_id.clone(),
                    expected_source_revision: reviewed.semantic_digest.clone(),
                },
            )
            .await
            .is_err()
        );
        assert!(
            delete_adapter_connection(
                &state,
                "human:local",
                GraphqlDeleteAdapterConnectionInput {
                    connection_id: connection.connection_id.clone(),
                    expected_connection_revision: connection.connection_revision + 1,
                },
            )
            .await
            .is_err()
        );
        assert!(
            delete_adapter_connection(
                &state,
                "human:local",
                GraphqlDeleteAdapterConnectionInput {
                    connection_id: connection.connection_id.clone(),
                    expected_connection_revision: connection.connection_revision,
                },
            )
            .await
            .expect("delete")
        );
        let definitions = AdapterDefinitionStore::new(paths.clone())
            .scan()
            .expect("definitions");
        assert!(
            AdapterConnectionStore::new(paths.clone())
                .scan(&definitions.definitions)
                .expect("connections")
                .connections
                .is_empty()
        );
        assert!(
            state
                .store()
                .expect("store")
                .adapter_connections()
                .await
                .expect("connection projections")
                .is_empty()
        );
        assert!(
            delete_adapter_service(
                &state,
                "human:local",
                GraphqlDeleteAdapterServiceInput {
                    definition_id: reviewed.definition_id,
                    expected_source_revision: reviewed.semantic_digest,
                },
            )
            .await
            .expect("delete service")
        );
        assert!(
            AdapterDefinitionStore::new(paths.clone())
                .scan()
                .expect("definitions after service deletion")
                .definitions
                .is_empty()
        );
        assert!(
            state
                .store()
                .expect("store")
                .adapter_definitions()
                .await
                .expect("definition projections")
                .is_empty()
        );

        let repeated_pending = AdapterDefinitionStore::new(paths.clone())
            .install(
                &oauth_pending_manifest(),
                "https://developers.example.test/oauth",
                None,
                None,
            )
            .expect("repeated pending definition");
        let repeated_reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: repeated_pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve repeated definition");
        assert!(
            delete_adapter_service(
                &state,
                "human:local",
                GraphqlDeleteAdapterServiceInput {
                    definition_id: repeated_reviewed.definition_id,
                    expected_source_revision: repeated_reviewed.semantic_digest,
                },
            )
            .await
            .expect("delete repeated service")
        );
        assert!(
            AdapterDefinitionStore::new(paths)
                .scan()
                .expect("definitions after repeated deletion")
                .definitions
                .is_empty()
        );
        assert!(
            state
                .store()
                .expect("store")
                .adapter_definitions()
                .await
                .expect("definition projections after repeated deletion")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn concurrent_revisions_publish_independent_connections() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths.clone());
        let first = definitions
            .install(
                &oauth_pending_manifest(),
                "https://developers.example.test/oauth-v1",
                None,
                None,
            )
            .expect("first definition");
        let mut second_manifest = oauth_pending_manifest();
        second_manifest.definition_revision = "v2".to_string();
        second_manifest.operations[0].path = "/v2/items".to_string();
        let second = definitions
            .install(
                &second_manifest,
                "https://developers.example.test/oauth-v2",
                None,
                None,
            )
            .expect("second definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        let first = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: first.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve first");
        let second = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: second.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve second");
        let upload = BASE64_STANDARD.encode(
            br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
        );
        let first_import = import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: first.semantic_digest,
                client_json_base64: upload.clone(),
            },
        );
        let second_import = import_adapter_oauth_client_json(
            &state,
            "human:local",
            GraphqlImportAdapterOauthClientJsonInput {
                semantic_digest: second.semantic_digest,
                client_json_base64: upload,
            },
        );
        let (first_result, second_result) = tokio::join!(first_import, second_import);
        assert!(first_result.is_ok());
        assert!(second_result.is_ok());

        let scan = definitions.scan().expect("definitions");
        let connections = AdapterConnectionStore::new(paths)
            .scan(&scan.definitions)
            .expect("connections");
        assert_eq!(connections.connections.len(), 2);
    }
}
