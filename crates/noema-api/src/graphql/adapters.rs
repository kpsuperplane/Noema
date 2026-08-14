//! Thin GraphQL review surface for filesystem-canonical adapter definitions.

use async_graphql::{InputObject, SimpleObject};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use noema_capability_adapters::{
    AdapterConnectionAuthenticationV1, AdapterOAuthAttemptEvent, AdapterOAuthAttemptStatus,
    AdapterOAuthAuthorizationRequest, AdapterOAuthServiceSelection, AdapterOAuthSetupError,
    AdapterOperation, AuthenticationMode, AuthenticationSchemeV4, AuthorizationGrantStatus,
    CredentialInput, CredentialSetup, LuauTransform, Oauth2CallbackMode, OauthApplicationStatus,
    ResponseTransform, StoredAdapterDefinition,
};
#[cfg(test)]
use noema_capability_adapters::{AdapterConnectionStore, AdapterDefinitionStore};
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
    pub accepted_scope_sets: Vec<GraphqlAdapterScopeSet>,
}

/// One complete accepted OAuth scope set for an operation.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterScopeSet")]
pub struct GraphqlAdapterScopeSet {
    pub scopes: Vec<String>,
}

/// Derived access for one operation on one connection.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOperationAccess")]
pub struct GraphqlAdapterOperationAccess {
    pub operation_id: String,
    pub status: String,
    pub missing_scopes: Vec<String>,
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
    pub grant_id: Option<String>,
    pub account_id: Option<String>,
    pub connection_revision: u64,
    pub credential_revision: Option<u64>,
    pub grant_revision: Option<u64>,
    pub policy_revision: u64,
    pub granted_scopes: Vec<String>,
    pub allowed_operations: Vec<String>,
    pub policy_configured: bool,
    pub operation_access: Vec<GraphqlAdapterOperationAccess>,
}

/// One reviewed public OAuth profile.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOauthProfile")]
pub struct GraphqlAdapterOauthProfile {
    pub profile_digest: String,
    pub profile_id: String,
    pub display_name: String,
    pub grant_audience: String,
    pub credential_setup: Option<GraphqlAdapterCredentialSetup>,
}

/// One non-secret reusable OAuth application.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOauthApplication")]
pub struct GraphqlAdapterOauthApplication {
    pub application_id: String,
    pub profile_digest: String,
    pub provider_display_name: String,
    pub callback_mode: String,
    pub client_id: String,
    pub project_label: Option<String>,
    pub revision: u64,
    pub status: String,
    pub grant_count: i32,
    pub account_count: i32,
}

/// One stable external account identity.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterExternalAccount")]
pub struct GraphqlAdapterExternalAccount {
    pub account_id: String,
    pub profile_digest: String,
    pub account_label: Option<String>,
    pub revision: u64,
    pub grant_ids: Vec<String>,
}

/// One reusable account authorization and its dependent connections.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterAuthorizationGrant")]
pub struct GraphqlAdapterAuthorizationGrant {
    pub grant_id: String,
    pub application_id: String,
    pub account_id: Option<String>,
    pub account_label: Option<String>,
    pub provider_display_name: String,
    pub audience: String,
    pub desired_scopes: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub authority_revision: u64,
    pub token_revision: u64,
    pub status: String,
    pub connection_ids: Vec<String>,
}

/// Complete non-secret OAuth settings state.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterOauthState")]
pub struct GraphqlAdapterOauthState {
    pub profiles: Vec<GraphqlAdapterOauthProfile>,
    pub applications: Vec<GraphqlAdapterOauthApplication>,
    pub accounts: Vec<GraphqlAdapterExternalAccount>,
    pub grants: Vec<GraphqlAdapterAuthorizationGrant>,
}

/// One server-selected safe action for an adapter definition.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterNextAction")]
pub struct GraphqlAdapterNextAction {
    pub kind: String,
    pub semantic_digest: String,
    pub application_id: Option<String>,
    pub expected_application_revision: Option<u64>,
    pub grant_id: Option<String>,
    pub expected_grant_revision: Option<u64>,
    pub connection_id: Option<String>,
    pub expected_connection_revision: Option<u64>,
    pub expected_policy_revision: Option<u64>,
    pub operation_ids: Vec<String>,
    pub missing_scopes: Vec<String>,
}

/// Human-reviewable impact of one immutable definition revision.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "AdapterDefinitionTransition")]
pub struct GraphqlAdapterDefinitionTransition {
    pub added_operations: Vec<String>,
    pub changed_operations: Vec<String>,
    pub removed_operations: Vec<String>,
    pub authentication_changed: bool,
    pub affected_connections: i32,
    pub affected_schedules: i32,
    pub authentication_required_connections: i32,
    pub consolidated_connections: i32,
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
    pub oauth_profile_digest: Option<String>,
    pub scopes: Vec<String>,
    pub credential_setup: Option<GraphqlAdapterCredentialSetup>,
    pub account_identity_operation_id: Option<String>,
    pub operations: Vec<GraphqlAdapterOperation>,
    pub transition: GraphqlAdapterDefinitionTransition,
    pub manifest_json: String,
    pub connection_count: i32,
    pub connections: Vec<GraphqlAdapterConnection>,
    pub reviewed: bool,
    pub superseded: bool,
    pub next_action: Option<GraphqlAdapterNextAction>,
    pub connection_actions: Vec<GraphqlAdapterNextAction>,
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
    pub replacement_connection_id: Option<String>,
    #[graphql(default)]
    pub field_values: Vec<GraphqlAdapterCredentialFieldValueInput>,
    pub document_base64: Option<String>,
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
    pub application_id: String,
    pub expected_application_revision: u64,
    pub grant_id: Option<String>,
    pub expected_grant_revision: Option<u64>,
    pub semantic_digest: String,
    pub operation_ids: Vec<String>,
    pub additional_services: Option<Vec<GraphqlAdapterOauthServiceSelectionInput>>,
}

/// One additional reviewed API included in the same OAuth authorization.
#[derive(Clone, InputObject)]
#[graphql(name = "AdapterOauthServiceSelectionInput")]
pub struct GraphqlAdapterOauthServiceSelectionInput {
    pub semantic_digest: String,
    pub operation_ids: Vec<String>,
}

/// Import one reusable OAuth application document.
#[derive(Clone, InputObject)]
#[graphql(name = "ImportAdapterOauthApplicationInput")]
pub struct GraphqlImportAdapterOauthApplicationInput {
    pub profile_digest: String,
    pub project_label: Option<String>,
    pub client_document_base64: String,
}

/// Replace one exact OAuth application document.
#[derive(Clone, InputObject)]
#[graphql(name = "ReplaceAdapterOauthApplicationInput")]
pub struct GraphqlReplaceAdapterOauthApplicationInput {
    pub application_id: String,
    pub expected_revision: u64,
    pub client_document_base64: String,
}

/// Attach one reviewed API definition to one reusable grant.
#[derive(Clone, InputObject)]
#[graphql(name = "AttachAdapterOauthConnectionInput")]
pub struct GraphqlAttachAdapterOauthConnectionInput {
    pub semantic_digest: String,
    pub grant_id: String,
    pub expected_grant_revision: u64,
    pub replacement_connection_id: Option<String>,
}

/// Disconnect one exact reusable grant revision.
#[derive(Clone, InputObject)]
#[graphql(name = "DisconnectAdapterOauthGrantInput")]
pub struct GraphqlDisconnectAdapterOauthGrantInput {
    pub grant_id: String,
    pub expected_authority_revision: u64,
}

/// Label one grant when the provider exposes no stable account identity.
#[derive(Clone, InputObject)]
#[graphql(name = "SaveAdapterOauthGrantLabelInput")]
pub struct GraphqlSaveAdapterOauthGrantLabelInput {
    pub grant_id: String,
    pub expected_authority_revision: u64,
    pub account_label: Option<String>,
}

/// Delete one unreferenced OAuth application revision.
#[derive(Clone, InputObject)]
#[graphql(name = "DeleteAdapterOauthApplicationInput")]
pub struct GraphqlDeleteAdapterOauthApplicationInput {
    pub application_id: String,
    pub expected_revision: u64,
}

/// Suspend or resume one exact API connection.
#[derive(Clone, InputObject)]
#[graphql(name = "SetAdapterConnectionActiveInput")]
pub struct GraphqlSetAdapterConnectionActiveInput {
    pub connection_id: String,
    pub expected_connection_revision: u64,
    pub active: bool,
}

/// Opaque browser handoff for a provider-neutral adapter OAuth attempt.
#[derive(Clone, SimpleObject)]
#[graphql(name = "AdapterOauthSetupAttempt")]
pub struct GraphqlAdapterOauthSetupAttempt {
    pub attempt_id: String,
    pub authorization_url: String,
    pub expires_at_epoch_seconds: u64,
}

/// Current lifecycle state of one browser OAuth attempt.
#[derive(Clone, SimpleObject)]
#[graphql(name = "AdapterOauthAttemptEvent")]
pub struct GraphqlAdapterOauthAttemptEvent {
    pub attempt_id: String,
    pub semantic_digest: Option<String>,
    pub grant_id: Option<String>,
    pub grant_revision: Option<u64>,
    pub status: String,
}

impl From<AdapterOAuthAttemptEvent> for GraphqlAdapterOauthAttemptEvent {
    fn from(event: AdapterOAuthAttemptEvent) -> Self {
        Self {
            attempt_id: event.attempt_id,
            semantic_digest: event.semantic_digest,
            grant_id: event.grant_id,
            grant_revision: event.grant_revision,
            status: oauth_attempt_status_label(event.status).to_string(),
        }
    }
}

pub(super) const fn oauth_attempt_terminal(status: AdapterOAuthAttemptStatus) -> bool {
    !matches!(status, AdapterOAuthAttemptStatus::Authorizing)
}

const fn oauth_attempt_status_label(status: AdapterOAuthAttemptStatus) -> &'static str {
    match status {
        AdapterOAuthAttemptStatus::Authorizing => "authorizing",
        AdapterOAuthAttemptStatus::Completed => "completed",
        AdapterOAuthAttemptStatus::Denied => "denied",
        AdapterOAuthAttemptStatus::Expired => "expired",
        AdapterOAuthAttemptStatus::Superseded => "superseded",
        AdapterOAuthAttemptStatus::Failed => "failed",
    }
}

pub(super) fn adapter_oauth_attempt(
    state: &GraphqlState,
    attempt_id: &str,
) -> async_graphql::Result<Option<GraphqlAdapterOauthAttemptEvent>> {
    Ok(state
        .adapter_operations()?
        .oauth_attempt_status(attempt_id)
        .map(Into::into))
}

pub(super) async fn adapter_definitions(
    state: &GraphqlState,
) -> async_graphql::Result<Vec<GraphqlAdapterDefinition>> {
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    let grants = snapshot
        .oauth_authorities
        .grants
        .iter()
        .map(|grant| (grant.grant_id.as_str(), grant))
        .collect::<BTreeMap<_, _>>();
    let mut connections_by_digest =
        BTreeMap::<String, Vec<noema_capability_adapters::AdapterConnectionV4>>::new();
    for connection in &snapshot.connections.connections {
        connections_by_digest
            .entry(connection.descriptor.semantic_digest.clone())
            .or_default()
            .push(connection.descriptor.clone());
    }
    for connections in connections_by_digest.values_mut() {
        connections.sort_by(|left, right| left.connection_id.cmp(&right.connection_id));
    }
    let oauth_callback = state
        .adapter_oauth_callback_url()
        .ok()
        .and_then(|url| adapter_callback_mode(url).ok().map(|mode| (url, mode)));
    let mut definitions = snapshot
        .definitions
        .definitions
        .iter()
        .filter(|definition| {
            let digest = definition.compiled.semantic_digest.as_str();
            let has_connections = connections_by_digest
                .get(digest)
                .is_some_and(|connections| !connections.is_empty());
            !snapshot.superseded_pending_digests.contains(digest)
                && (has_connections || !snapshot.replaced_definition_digests.contains(digest))
        })
        .map(|definition| {
            let digest = definition.compiled.semantic_digest.as_str();
            let stored = state
                .adapter_operations()?
                .stored_definition(digest)
                .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))?;
            let connections: Vec<GraphqlAdapterConnection> = connections_by_digest
                .get(digest)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|connection| connection_view(connection, &grants, &stored.manifest))
                .collect();
            let profile = stored.manifest.authentication.oauth2().and_then(|oauth| {
                snapshot
                    .oauth_authorities
                    .profiles
                    .iter()
                    .find(|profile| profile.profile_digest == oauth.profile_digest)
            });
            let superseded = false;
            let next_action = definition_next_action(
                digest,
                &definition.compiled,
                &stored.manifest,
                superseded,
                &connections,
                &snapshot.oauth_authorities,
            );
            let connection_actions = definition_connection_actions(
                digest,
                &definition.compiled,
                &stored.manifest,
                superseded,
                &connections,
                &snapshot.oauth_authorities,
            );
            let transition = stored.provenance.transition.clone().unwrap_or_default();
            Ok(definition_view(
                digest,
                &stored,
                superseded,
                connections,
                oauth_callback,
                profile,
                (next_action, connection_actions, transition_view(transition)),
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

pub(super) async fn adapter_oauth_state(
    state: &GraphqlState,
) -> async_graphql::Result<GraphqlAdapterOauthState> {
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|_| async_graphql::Error::new("adapter OAuth state is unavailable"))?;
    let callback = state
        .adapter_oauth_callback_url()
        .ok()
        .and_then(|url| adapter_callback_mode(url).ok().map(|mode| (url, mode)));
    Ok(oauth_state_view(&snapshot, callback))
}

pub(super) async fn import_adapter_oauth_application(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlImportAdapterOauthApplicationInput,
) -> async_graphql::Result<GraphqlAdapterOauthApplication> {
    require_local_human(
        principal,
        "adapter OAuth application import is unauthorized",
    )?;
    let document = decode_client_document(&input.client_document_base64)?;
    let redirect_uri = state.adapter_oauth_callback_url()?;
    let callback_mode = adapter_callback_mode(redirect_uri)?;
    let application = state
        .adapter_operations()?
        .import_oauth_application(
            &input.profile_digest,
            callback_mode,
            redirect_uri,
            &document,
            input.project_label,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    let oauth = adapter_oauth_state(state).await?;
    oauth
        .applications
        .into_iter()
        .find(|candidate| candidate.application_id == application.application_id)
        .ok_or_else(|| async_graphql::Error::new("adapter OAuth application is unavailable"))
}

pub(super) async fn replace_adapter_oauth_application(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlReplaceAdapterOauthApplicationInput,
) -> async_graphql::Result<GraphqlAdapterOauthApplication> {
    require_local_human(
        principal,
        "adapter OAuth application replacement is unauthorized",
    )?;
    let document = decode_client_document(&input.client_document_base64)?;
    let redirect_uri = state.adapter_oauth_callback_url()?;
    let application = state
        .adapter_operations()?
        .replace_oauth_application(
            &input.application_id,
            input.expected_revision,
            redirect_uri,
            &document,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_oauth_state(state)
        .await?
        .applications
        .into_iter()
        .find(|candidate| candidate.application_id == application.application_id)
        .ok_or_else(|| async_graphql::Error::new("adapter OAuth application is unavailable"))
}

pub(super) async fn attach_adapter_oauth_connection(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlAttachAdapterOauthConnectionInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    require_local_human(principal, "adapter account attachment is unauthorized")?;
    state
        .adapter_operations()?
        .attach_oauth_connection(
            &input.semantic_digest,
            &input.grant_id,
            input.expected_grant_revision,
            input.replacement_connection_id.as_deref(),
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| definition.semantic_digest == input.semantic_digest)
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
}

pub(super) async fn disconnect_adapter_oauth_grant(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlDisconnectAdapterOauthGrantInput,
) -> async_graphql::Result<GraphqlAdapterAuthorizationGrant> {
    require_local_human(principal, "adapter account disconnect is unauthorized")?;
    let grant = state
        .adapter_operations()?
        .disconnect_oauth_grant(&input.grant_id, input.expected_authority_revision)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_oauth_state(state)
        .await?
        .grants
        .into_iter()
        .find(|candidate| candidate.grant_id == grant.grant_id)
        .ok_or_else(|| async_graphql::Error::new("adapter account is unavailable"))
}

pub(super) async fn save_adapter_oauth_grant_label(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlSaveAdapterOauthGrantLabelInput,
) -> async_graphql::Result<GraphqlAdapterAuthorizationGrant> {
    require_local_human(principal, "adapter account label update is unauthorized")?;
    let grant = state
        .adapter_operations()?
        .save_oauth_grant_label(
            &input.grant_id,
            input.expected_authority_revision,
            input.account_label,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_oauth_state(state)
        .await?
        .grants
        .into_iter()
        .find(|candidate| candidate.grant_id == grant.grant_id)
        .ok_or_else(|| async_graphql::Error::new("adapter account is unavailable"))
}

pub(super) async fn delete_adapter_oauth_application(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlDeleteAdapterOauthApplicationInput,
) -> async_graphql::Result<bool> {
    require_local_human(
        principal,
        "adapter OAuth application deletion is unauthorized",
    )?;
    let deleted = state
        .adapter_operations()?
        .delete_oauth_application(&input.application_id, input.expected_revision)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    Ok(deleted)
}

pub(super) async fn set_adapter_connection_active(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlSetAdapterConnectionActiveInput,
) -> async_graphql::Result<GraphqlAdapterDefinition> {
    require_local_human(principal, "adapter connection update is unauthorized")?;
    let connection = state
        .adapter_operations()?
        .set_connection_active(
            &input.connection_id,
            input.expected_connection_revision,
            input.active,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| definition.semantic_digest == connection.descriptor.semantic_digest)
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
}

fn require_local_human(principal: &str, message: &'static str) -> async_graphql::Result<()> {
    if principal == "human:local" {
        Ok(())
    } else {
        Err(async_graphql::Error::new(message))
    }
}

fn decode_client_document(encoded: &str) -> async_graphql::Result<Vec<u8>> {
    if encoded.is_empty() || encoded.len() > MAX_CREDENTIAL_DOCUMENT_BASE64_BYTES {
        return Err(async_graphql::Error::new(
            "adapter OAuth client document is invalid or too large",
        ));
    }
    let bytes = BASE64_STANDARD.decode(encoded.as_bytes()).map_err(|_| {
        async_graphql::Error::new("adapter OAuth client document is invalid or too large")
    })?;
    if bytes.is_empty() || bytes.len() > MAX_CREDENTIAL_DOCUMENT_BYTES {
        return Err(async_graphql::Error::new(
            "adapter OAuth client document is invalid or too large",
        ));
    }
    Ok(bytes)
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
        .start_oauth_authorization(
            principal,
            AdapterOAuthAuthorizationRequest {
                application_id: input.application_id,
                expected_application_revision: input.expected_application_revision,
                grant_id: input.grant_id,
                expected_grant_revision: input.expected_grant_revision,
                semantic_digest: input.semantic_digest,
                operation_ids: input.operation_ids,
                additional_services: input
                    .additional_services
                    .unwrap_or_default()
                    .into_iter()
                    .map(|selection| AdapterOAuthServiceSelection {
                        semantic_digest: selection.semantic_digest,
                        operation_ids: selection.operation_ids,
                    })
                    .collect(),
                callback_mode,
                redirect_uri: callback_url.to_string(),
            },
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
) -> Result<GraphqlAdapterDefinition, AdapterOAuthSetupError> {
    let mut callback =
        url::Url::parse(callback_url).map_err(|_| AdapterOAuthSetupError::Invalid)?;
    callback.set_query(None);
    callback.set_fragment(None);
    let expected = url::Url::parse(
        state
            .adapter_oauth_callback_url()
            .map_err(|_| AdapterOAuthSetupError::Unavailable)?,
    )
    .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
    if callback != expected {
        return Err(AdapterOAuthSetupError::Invalid);
    }
    let completed = state
        .adapter_operations()
        .map_err(|_| AdapterOAuthSetupError::Unavailable)?
        .complete_oauth_callback(callback_url)
        .await?;
    reconcile_adapter_connections(state)
        .await
        .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
    let definition = adapter_definitions(state)
        .await
        .map_err(|_| AdapterOAuthSetupError::Unavailable)?
        .into_iter()
        .find(|definition| definition.semantic_digest == completed.semantic_digest)
        .ok_or(AdapterOAuthSetupError::Unavailable)?;
    state
        .runtime()
        .map_err(|_| AdapterOAuthSetupError::Unavailable)?
        .resume_mcp_authentication_attempt(completed.attempt_id)
        .await
        .map_err(|_| AdapterOAuthSetupError::Unavailable)?;
    publish_primary_interventions_changed(state).await;
    Ok(definition)
}

/// Return actionable callback copy without disclosing OAuth response details.
#[must_use]
pub const fn adapter_oauth_failure_message(error: AdapterOAuthSetupError) -> &'static str {
    match error {
        AdapterOAuthSetupError::Invalid => {
            "Noema no longer recognizes this connection attempt. Return to Noema and start again."
        }
        AdapterOAuthSetupError::Superseded => {
            "A newer connection attempt replaced this one. Return to Noema and continue there."
        }
        AdapterOAuthSetupError::Expired => {
            "This connection attempt expired. Return to Noema and start again."
        }
        AdapterOAuthSetupError::Denied => {
            "The provider rejected this connection. Return to Noema and try again."
        }
        AdapterOAuthSetupError::Unavailable => {
            "Noema could not finish activating this connection. Return to Noema to review its status or try again."
        }
    }
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
    descriptor: &noema_capability_adapters::AdapterConnectionV4,
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
        connection_revision: descriptor.connection_revision.to_string(),
        granted_scopes: Vec::new(),
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
        .setup_connection(
            &input.semantic_digest,
            input.replacement_connection_id.as_deref(),
            field_values,
            document.as_deref(),
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    reconcile_adapter_connections(state).await?;
    adapter_definitions(state)
        .await?
        .into_iter()
        .find(|definition| definition.semantic_digest == input.semantic_digest)
        .ok_or_else(|| async_graphql::Error::new("adapter definition is unavailable"))
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
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|_| async_graphql::Error::new("adapter connections are unavailable"))?;
    let connections = snapshot.connections.projections();
    let store = state.store()?;
    store
        .reconcile_complete_adapter_state(&snapshot.oauth_authorities, &connections)
        .await
        .map_err(|_| async_graphql::Error::new("adapter index could not be updated"))
}

async fn reconcile_adapter_definitions(state: &GraphqlState) -> async_graphql::Result<()> {
    let snapshot = state
        .adapter_operations()?
        .management_snapshot()
        .map_err(|_| async_graphql::Error::new("adapter definitions are unavailable"))?;
    state
        .store()?
        .reconcile_adapter_definitions(&snapshot.definitions.projections())
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
        .review_definition_and_adopt(&input.semantic_digest)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    if installed.compiled.authentication.mode() == AuthenticationMode::None {
        state
            .adapter_operations()?
            .ensure_credential_free_connection(installed.compiled.semantic_digest.as_str())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    }
    reconcile_adapter_definitions(state).await?;
    reconcile_adapter_connections(state).await?;
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
    oauth_profile: Option<&noema_capability_adapters::OauthProfileInstall>,
    actions: (
        Option<GraphqlAdapterNextAction>,
        Vec<GraphqlAdapterNextAction>,
        GraphqlAdapterDefinitionTransition,
    ),
) -> GraphqlAdapterDefinition {
    let manifest = &stored.manifest;
    let (next_action, connection_actions, transition) = actions;
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
        oauth_profile_digest: manifest
            .authentication
            .oauth2()
            .map(|oauth| oauth.profile_digest.clone()),
        scopes: manifest
            .operations
            .iter()
            .flat_map(|operation| {
                operation
                    .authorization
                    .accepted_scope_sets()
                    .iter()
                    .flatten()
            })
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        credential_setup: match &manifest.authentication {
            AuthenticationSchemeV4::None => None,
            AuthenticationSchemeV4::Credential(config) => Some(credential_setup_view(
                &config.setup,
                None,
                Some(&config.request_auth),
            )),
            AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(_) => {
                oauth_callback.and_then(|(redirect_uri, mode)| {
                    oauth_profile.and_then(|profile| {
                        profile
                            .profile
                            .setups
                            .iter()
                            .find(|setup| setup.callback_mode == mode)
                            .map(|setup| {
                                credential_setup_view(&setup.setup, Some(redirect_uri), None)
                            })
                    })
                })
            }
        },
        account_identity_operation_id: oauth_profile
            .and_then(|profile| profile.profile.account_identity.as_ref())
            .map(|identity| identity.operation_id.clone()),
        operations: manifest.operations.iter().map(operation_view).collect(),
        transition,
        manifest_json: serde_json::to_string_pretty(manifest)
            .unwrap_or_else(|_| "adapter definition could not be displayed".to_string()),
        connection_count: i32::try_from(connections.len()).unwrap_or(i32::MAX),
        connections,
        reviewed: manifest.reviewed,
        superseded,
        next_action,
        connection_actions,
    }
}

fn transition_view(
    transition: noema_capability_adapters::AdapterDefinitionTransition,
) -> GraphqlAdapterDefinitionTransition {
    GraphqlAdapterDefinitionTransition {
        added_operations: transition.added_operations,
        changed_operations: transition.changed_operations,
        removed_operations: transition.removed_operations,
        authentication_changed: transition.authentication_changed,
        affected_connections: i32::try_from(transition.affected_connections).unwrap_or(i32::MAX),
        affected_schedules: i32::try_from(transition.affected_schedules).unwrap_or(i32::MAX),
        authentication_required_connections: i32::try_from(
            transition.authentication_required_connections,
        )
        .unwrap_or(i32::MAX),
        consolidated_connections: i32::try_from(transition.consolidated_connections)
            .unwrap_or(i32::MAX),
    }
}

fn definition_next_action(
    semantic_digest: &str,
    compiled: &noema_capability_adapters::CompiledAdapterDefinition,
    manifest: &noema_capability_adapters::AdapterManifest,
    superseded: bool,
    connections: &[GraphqlAdapterConnection],
    oauth: &noema_capability_adapters::OauthAuthoritySnapshot,
) -> Option<GraphqlAdapterNextAction> {
    if superseded {
        return None;
    }
    if !manifest.reviewed {
        return Some(adapter_next_action("review_definition", semantic_digest));
    }
    if let Some(connection) = connections.iter().find(|connection| {
        matches!(
            connection.status.as_str(),
            "authentication_required" | "revoked" | "blocked"
        )
    }) {
        let mut action = if connection.grant_id.is_none()
            && manifest.authentication.mode() == AuthenticationMode::Oauth2AuthorizationCodePkce
        {
            definition_connection_actions(semantic_digest, compiled, manifest, false, &[], oauth)
                .into_iter()
                .next()
                .unwrap_or_else(|| adapter_next_action("reconnect_account", semantic_digest))
        } else {
            adapter_next_action("reconnect_account", semantic_digest)
        };
        if manifest.authentication.mode() == AuthenticationMode::Credential {
            action.kind = "set_up_credential".to_string();
        } else if action.kind != "import_application" {
            action.kind = "reconnect_account".to_string();
        }
        action.grant_id.clone_from(&connection.grant_id);
        action.expected_grant_revision = connection.grant_revision;
        action.connection_id = Some(connection.connection_id.clone());
        action.expected_connection_revision = Some(connection.connection_revision);
        action.operation_ids = if connection.allowed_operations.is_empty() {
            manifest
                .operations
                .iter()
                .map(|operation| operation.operation_id.clone())
                .collect()
        } else {
            connection.allowed_operations.clone()
        };
        populate_grant_authority(&mut action, oauth);
        return Some(action);
    }
    if let Some(connection) = connections.iter().find(|connection| {
        connection
            .operation_access
            .iter()
            .any(|access| access.status == "add_access")
    }) {
        let mut action = adapter_next_action("add_access", semantic_digest);
        action.grant_id.clone_from(&connection.grant_id);
        action.expected_grant_revision = connection.grant_revision;
        action.connection_id = Some(connection.connection_id.clone());
        action.expected_connection_revision = Some(connection.connection_revision);
        action.operation_ids = connection
            .operation_access
            .iter()
            .filter(|access| access.status == "add_access")
            .map(|access| access.operation_id.clone())
            .collect();
        action.missing_scopes = connection
            .operation_access
            .iter()
            .filter(|access| access.status == "add_access")
            .flat_map(|access| access.missing_scopes.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        populate_grant_authority(&mut action, oauth);
        return Some(action);
    }
    if let Some(connection) = connections
        .iter()
        .find(|connection| !connection.policy_configured)
    {
        let mut action = adapter_next_action("review_connection_policy", semantic_digest);
        action.connection_id = Some(connection.connection_id.clone());
        action.expected_connection_revision = Some(connection.connection_revision);
        action.expected_policy_revision = Some(connection.policy_revision);
        return Some(action);
    }
    if !connections.is_empty() {
        return None;
    }
    definition_connection_actions(
        semantic_digest,
        compiled,
        manifest,
        superseded,
        connections,
        oauth,
    )
    .into_iter()
    .next()
}

fn definition_connection_actions(
    semantic_digest: &str,
    compiled: &noema_capability_adapters::CompiledAdapterDefinition,
    manifest: &noema_capability_adapters::AdapterManifest,
    superseded: bool,
    connections: &[GraphqlAdapterConnection],
    oauth: &noema_capability_adapters::OauthAuthoritySnapshot,
) -> Vec<GraphqlAdapterNextAction> {
    if superseded || !manifest.reviewed {
        return Vec::new();
    }
    let AuthenticationSchemeV4::Oauth2AuthorizationCodePkce(config) = &manifest.authentication
    else {
        return match &manifest.authentication {
            AuthenticationSchemeV4::Credential(_) if connections.is_empty() => {
                vec![adapter_next_action("set_up_credential", semantic_digest)]
            }
            _ => Vec::new(),
        };
    };
    let applications = oauth
        .applications
        .iter()
        .filter(|application| {
            application.profile_digest == config.profile_digest
                && application.status == OauthApplicationStatus::Active
        })
        .collect::<Vec<_>>();
    if applications.is_empty() {
        return vec![adapter_next_action("import_application", semantic_digest)];
    }
    let connected_grants = connections
        .iter()
        .filter_map(|connection| connection.grant_id.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    let operation_ids = manifest
        .operations
        .iter()
        .map(|operation| operation.operation_id.clone())
        .collect::<Vec<_>>();
    let mut attach = Vec::new();
    let mut expand = Vec::new();
    let mut reconnect = Vec::new();
    for grant in oauth.grants.iter().filter(|grant| {
        !connected_grants.contains(grant.grant_id.as_str())
            && applications
                .iter()
                .any(|application| application.application_id == grant.application_id)
    }) {
        let Some(application) = applications
            .iter()
            .find(|application| application.application_id == grant.application_id)
        else {
            continue;
        };
        let mut action = if grant.status != AuthorizationGrantStatus::Active {
            adapter_next_action("reconnect_account", semantic_digest)
        } else if manifest.operations.iter().all(|operation| {
            operation
                .authorization
                .is_satisfied_by(&grant.granted_scopes)
        }) {
            adapter_next_action("attach_account", semantic_digest)
        } else if let Some(target) = compiled.scope_target(&operation_ids, &grant.granted_scopes) {
            let granted = grant
                .granted_scopes
                .iter()
                .collect::<std::collections::BTreeSet<_>>();
            let mut action = adapter_next_action("add_access", semantic_digest);
            action.missing_scopes = target
                .into_iter()
                .filter(|scope| !granted.contains(scope))
                .collect();
            action
        } else {
            continue;
        };
        action.application_id = Some(application.application_id.clone());
        action.expected_application_revision = Some(application.revision);
        action.grant_id = Some(grant.grant_id.clone());
        action.expected_grant_revision = Some(grant.authority_revision);
        action.operation_ids.clone_from(&operation_ids);
        match action.kind.as_str() {
            "attach_account" => attach.push(action),
            "add_access" => expand.push(action),
            _ => reconnect.push(action),
        }
    }
    let mut actions = attach;
    actions.extend(expand);
    actions.extend(reconnect);
    actions.extend(applications.into_iter().map(|application| {
        let mut action = adapter_next_action("add_account", semantic_digest);
        action.application_id = Some(application.application_id.clone());
        action.expected_application_revision = Some(application.revision);
        action.operation_ids.clone_from(&operation_ids);
        action
    }));
    actions
}

fn populate_grant_authority(
    action: &mut GraphqlAdapterNextAction,
    oauth: &noema_capability_adapters::OauthAuthoritySnapshot,
) {
    let Some(grant) = action
        .grant_id
        .as_deref()
        .and_then(|grant_id| oauth.grants.iter().find(|grant| grant.grant_id == grant_id))
    else {
        return;
    };
    let Some(application) = oauth
        .applications
        .iter()
        .find(|application| application.application_id == grant.application_id)
    else {
        return;
    };
    action.application_id = Some(application.application_id.clone());
    action.expected_application_revision = Some(application.revision);
}

fn adapter_next_action(kind: &str, semantic_digest: &str) -> GraphqlAdapterNextAction {
    GraphqlAdapterNextAction {
        kind: kind.to_string(),
        semantic_digest: semantic_digest.to_string(),
        application_id: None,
        expected_application_revision: None,
        grant_id: None,
        expected_grant_revision: None,
        connection_id: None,
        expected_connection_revision: None,
        expected_policy_revision: None,
        operation_ids: Vec::new(),
        missing_scopes: Vec::new(),
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
    descriptor: &noema_capability_adapters::AdapterConnectionV4,
    grants: &BTreeMap<&str, &noema_capability_adapters::AuthorizationGrantV1>,
    manifest: &noema_capability_adapters::AdapterManifest,
) -> GraphqlAdapterConnection {
    let (grant_id, account_id, grant_revision, granted_scopes, credential_revision) =
        match &descriptor.authentication {
            AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => {
                let grant = grants.get(grant_id.as_str()).copied();
                (
                    Some(grant_id.clone()),
                    grant.and_then(|grant| grant.account_id.clone()),
                    grant.map(|grant| grant.authority_revision),
                    grant.map_or_else(Vec::new, |grant| grant.granted_scopes.clone()),
                    None,
                )
            }
            AdapterConnectionAuthenticationV1::Credential { revision, .. } => {
                (None, None, None, Vec::new(), Some(*revision))
            }
            AdapterConnectionAuthenticationV1::Pending
            | AdapterConnectionAuthenticationV1::None => (None, None, None, Vec::new(), None),
        };
    let grant = grant_id
        .as_deref()
        .and_then(|grant_id| grants.get(grant_id).copied());
    let status =
        if descriptor.status == noema_capability_adapters::AdapterConnectionStatus::Suspended {
            "suspended"
        } else {
            match grant.map(|grant| grant.status) {
                Some(AuthorizationGrantStatus::AuthenticationRequired) => "authentication_required",
                Some(AuthorizationGrantStatus::Revoked) => "revoked",
                Some(AuthorizationGrantStatus::Blocked) => "blocked",
                _ => descriptor.status.as_str(),
            }
        };
    let operation_access = manifest
        .operations
        .iter()
        .map(|operation| operation_access_view(descriptor, operation, grant))
        .collect();
    GraphqlAdapterConnection {
        connection_id: descriptor.connection_id.clone(),
        status: status.to_string(),
        grant_id,
        account_id,
        connection_revision: descriptor.connection_revision,
        credential_revision,
        grant_revision,
        policy_revision: descriptor.policy_revision,
        granted_scopes,
        allowed_operations: descriptor.allowed_operations.clone(),
        policy_configured: descriptor.policy.is_some(),
        operation_access,
    }
}

fn operation_access_view(
    descriptor: &noema_capability_adapters::AdapterConnectionV4,
    operation: &AdapterOperation,
    grant: Option<&noema_capability_adapters::AuthorizationGrantV1>,
) -> GraphqlAdapterOperationAccess {
    if descriptor.status == noema_capability_adapters::AdapterConnectionStatus::Suspended
        || !descriptor
            .allowed_operations
            .contains(&operation.operation_id)
    {
        return GraphqlAdapterOperationAccess {
            operation_id: operation.operation_id.clone(),
            status: "disabled".to_string(),
            missing_scopes: Vec::new(),
        };
    }
    if let Some(grant) = grant {
        if grant.status != AuthorizationGrantStatus::Active {
            return GraphqlAdapterOperationAccess {
                operation_id: operation.operation_id.clone(),
                status: "reconnect".to_string(),
                missing_scopes: Vec::new(),
            };
        }
        if !operation
            .authorization
            .is_satisfied_by(&grant.granted_scopes)
        {
            let granted = grant
                .granted_scopes
                .iter()
                .collect::<std::collections::BTreeSet<_>>();
            let missing_scopes = operation
                .authorization
                .accepted_scope_sets()
                .iter()
                .map(|set| {
                    set.iter()
                        .filter(|scope| !granted.contains(scope))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .min_by_key(|missing| (missing.len(), missing.join("\0")))
                .unwrap_or_default();
            return GraphqlAdapterOperationAccess {
                operation_id: operation.operation_id.clone(),
                status: "add_access".to_string(),
                missing_scopes,
            };
        }
    }
    GraphqlAdapterOperationAccess {
        operation_id: operation.operation_id.clone(),
        status: "available".to_string(),
        missing_scopes: Vec::new(),
    }
}

fn oauth_state_view(
    snapshot: &noema_capability_adapters::AdapterManagementSnapshot,
    callback: Option<(&str, Oauth2CallbackMode)>,
) -> GraphqlAdapterOauthState {
    let profiles = snapshot
        .oauth_authorities
        .profiles
        .iter()
        .map(|profile| GraphqlAdapterOauthProfile {
            profile_digest: profile.profile_digest.clone(),
            profile_id: profile.profile.profile_id.clone(),
            display_name: profile.profile.display_name.clone(),
            grant_audience: profile.profile.grant_audience.clone(),
            credential_setup: callback.and_then(|(redirect_uri, mode)| {
                profile
                    .profile
                    .setups
                    .iter()
                    .find(|setup| setup.callback_mode == mode)
                    .map(|setup| credential_setup_view(&setup.setup, Some(redirect_uri), None))
            }),
        })
        .collect::<Vec<_>>();
    let profile_names = profiles
        .iter()
        .map(|profile| {
            (
                profile.profile_digest.as_str(),
                profile.display_name.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let applications = snapshot
        .oauth_authorities
        .applications
        .iter()
        .map(|application| {
            let grants = snapshot
                .oauth_authorities
                .grants
                .iter()
                .filter(|grant| grant.application_id == application.application_id)
                .collect::<Vec<_>>();
            let account_count = grants
                .iter()
                .map(|grant| grant.account_id.as_deref().unwrap_or(&grant.grant_id))
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            GraphqlAdapterOauthApplication {
                application_id: application.application_id.clone(),
                profile_digest: application.profile_digest.clone(),
                provider_display_name: profile_names
                    .get(application.profile_digest.as_str())
                    .copied()
                    .unwrap_or("OAuth provider")
                    .to_string(),
                callback_mode: callback_mode_label(application.callback_mode).to_string(),
                client_id: application.client_id.clone(),
                project_label: application.project_label.clone(),
                revision: application.revision,
                status: application_status_label(application.status).to_string(),
                grant_count: i32::try_from(grants.len()).unwrap_or(i32::MAX),
                account_count: i32::try_from(account_count).unwrap_or(i32::MAX),
            }
        })
        .collect();
    let accounts = snapshot
        .oauth_authorities
        .accounts
        .iter()
        .map(|account| GraphqlAdapterExternalAccount {
            account_id: account.account_id.clone(),
            profile_digest: account.profile_digest.clone(),
            account_label: account.account_label.clone(),
            revision: account.revision,
            grant_ids: snapshot
                .oauth_authorities
                .grants
                .iter()
                .filter(|grant| grant.account_id.as_deref() == Some(account.account_id.as_str()))
                .map(|grant| grant.grant_id.clone())
                .collect(),
        })
        .collect();
    let account_labels = snapshot
        .oauth_authorities
        .accounts
        .iter()
        .map(|account| {
            (
                account.account_id.as_str(),
                account.account_label.as_deref(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let grants = snapshot
        .oauth_authorities
        .grants
        .iter()
        .map(|grant| {
            let application = snapshot
                .oauth_authorities
                .applications
                .iter()
                .find(|application| application.application_id == grant.application_id);
            let provider_display_name = application
                .and_then(|application| profile_names.get(application.profile_digest.as_str()))
                .copied()
                .unwrap_or("OAuth provider")
                .to_string();
            GraphqlAdapterAuthorizationGrant {
                grant_id: grant.grant_id.clone(),
                application_id: grant.application_id.clone(),
                account_id: grant.account_id.clone(),
                account_label: grant
                    .account_id
                    .as_deref()
                    .and_then(|account_id| account_labels.get(account_id).copied().flatten())
                    .map(str::to_string)
                    .or_else(|| grant.account_label.clone()),
                provider_display_name,
                audience: grant.audience.clone(),
                desired_scopes: grant.desired_scopes.clone(),
                granted_scopes: grant.granted_scopes.clone(),
                authority_revision: grant.authority_revision,
                token_revision: grant.token_revision,
                status: grant_status_label(grant.status).to_string(),
                connection_ids: snapshot
                    .connections
                    .connections
                    .iter()
                    .filter_map(|connection| match &connection.descriptor.authentication {
                        AdapterConnectionAuthenticationV1::OauthGrant { grant_id }
                            if grant_id == &grant.grant_id =>
                        {
                            Some(connection.descriptor.connection_id.clone())
                        }
                        _ => None,
                    })
                    .collect(),
            }
        })
        .collect();
    GraphqlAdapterOauthState {
        profiles,
        applications,
        accounts,
        grants,
    }
}

const fn callback_mode_label(mode: Oauth2CallbackMode) -> &'static str {
    match mode {
        Oauth2CallbackMode::Loopback => "loopback",
        Oauth2CallbackMode::Hosted => "hosted",
    }
}

const fn application_status_label(status: OauthApplicationStatus) -> &'static str {
    match status {
        OauthApplicationStatus::Active => "active",
        OauthApplicationStatus::Suspended => "suspended",
    }
}

const fn grant_status_label(status: AuthorizationGrantStatus) -> &'static str {
    match status {
        AuthorizationGrantStatus::Active => "active",
        AuthorizationGrantStatus::AuthenticationRequired => "authentication_required",
        AuthorizationGrantStatus::Revoked => "revoked",
        AuthorizationGrantStatus::Blocked => "blocked",
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
        response_transform: operation.response.transform.as_ref().map(|transform| {
            let ResponseTransform::Luau { source } = transform;
            GraphqlAdapterResponseTransform {
                language: "luau".to_string(),
                source_digest: sha256_hex(source.as_bytes()),
                source: source.clone(),
                accepted_content_types: operation.response.accepted_content_types.clone(),
                output_schema_json: serde_json::to_string_pretty(&operation.response.output_schema)
                    .unwrap_or_else(|_| "response schema could not be displayed".to_string()),
            }
        }),
        accepted_scope_sets: operation
            .authorization
            .accepted_scope_sets()
            .iter()
            .map(|scopes| GraphqlAdapterScopeSet {
                scopes: scopes.clone(),
            })
            .collect(),
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
    use noema_capabilities::{
        CapabilityBindingSource, CapabilityInvocation, CapabilityInvoker, ToolName,
    };
    use noema_capability_adapters::{
        AdapterCompiler, AdapterManifest, AuthorizationGrantV1, OauthApplicationV1,
        OauthAuthoritySnapshot,
    };
    use noema_home::NoemaPaths;
    use serde_json::json;

    fn pending_manifest() -> AdapterManifest {
        serde_json::from_value(json!({
            "schema_version": 9,
            "definition_id": "definition:review_fixture",
            "adapter_id": "review_fixture",
            "display_name": "Review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {"kind": "none"},
            "operations": [{
                "operation_id": "list_items",
                "description": "List available items.",
                "method": "GET",
                "path": "/v1/items",
                "authorization": {"kind": "none"},
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
            }]
        }))
        .expect("manifest")
    }

    fn oauth_pending_manifest() -> AdapterManifest {
        serde_json::from_value(json!({
            "schema_version": 9,
            "definition_id": "definition:oauth_review_fixture",
            "adapter_id": "oauth_review_fixture",
            "display_name": "OAuth review fixture",
            "definition_revision": "v1",
            "reviewed": false,
            "origin": "https://api.example.test/",
            "authentication": {
                "kind": "oauth2_authorization_code_pkce",
                "profile_digest": noema_capability_adapters::reviewed_google_oauth_profile_digest()
            },
            "operations": [{
                "operation_id": "list_items",
                "description": "List available items.",
                "method": "GET",
                "path": "/v1/items",
                "authorization": {"kind": "oauth_scopes", "accepted_scope_sets": [["https://www.googleapis.com/auth/userinfo.email"]]},
                "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
                "retry": "transport_safe_read",
                "pagination": {"kind": "none"},
                "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
            }]
        }))
        .expect("OAuth manifest")
    }

    fn revision_proposal_arguments(
        manifest: &AdapterManifest,
        base_semantic_digest: &str,
        source_reference: &str,
        remove_operation_ids: &[&str],
    ) -> serde_json::Value {
        let operations = manifest
            .operations
            .iter()
            .map(|operation| {
                json!({
                    "operation_id": operation.operation_id,
                    "description": operation.description,
                    "source_description": operation.source_description,
                    "method": operation.method,
                    "path": operation.path,
                    "authorization": operation.authorization,
                    "fixed_headers": operation.fixed_headers,
                    "fixed_query": operation.fixed_query,
                    "arguments": operation.arguments,
                    "json_body_template": operation.json_body_template,
                    "read_only": operation.behavior.read_only.value.unwrap_or(false),
                    "idempotent": operation.behavior.idempotent.value.unwrap_or(false),
                    "destructive": operation.behavior.destructive.value.unwrap_or(true),
                    "open_world": operation.behavior.open_world.value.unwrap_or(true),
                    "pagination": operation.pagination,
                    "response": {
                        "kind": "custom",
                        "accepted_content_types": operation.response.accepted_content_types,
                        "transform": operation.response.transform,
                        "output_schema": operation.response.output_schema
                    }
                })
            })
            .collect::<Vec<_>>();
        json!({
            "source_reference": source_reference,
            "base_semantic_digest": base_semantic_digest,
            "revision": {
                "definition_revision": manifest.definition_revision,
                "display_name": manifest.display_name,
                "origin": manifest.origin,
                "authentication": manifest.authentication
            },
            "upsert_operations": operations,
            "remove_operation_ids": remove_operation_ids
        })
    }

    fn oauth_application(id: &str, profile_digest: &str) -> OauthApplicationV1 {
        OauthApplicationV1 {
            schema_version: 1,
            application_id: id.to_string(),
            profile_digest: profile_digest.to_string(),
            callback_mode: Oauth2CallbackMode::Loopback,
            client_id: format!("client-{id}"),
            project_label: None,
            credential_generation: format!("generation-{id}"),
            revision: 1,
            status: OauthApplicationStatus::Active,
        }
    }

    fn oauth_grant(id: &str, application_id: &str) -> AuthorizationGrantV1 {
        AuthorizationGrantV1 {
            schema_version: 1,
            grant_id: id.to_string(),
            application_id: application_id.to_string(),
            account_id: None,
            account_label: Some(format!("Account {id}")),
            audience: "google-apis".to_string(),
            desired_scopes: vec!["https://www.googleapis.com/auth/userinfo.email".to_string()],
            granted_scopes: vec!["https://www.googleapis.com/auth/userinfo.email".to_string()],
            authority_revision: 1,
            token_generation: Some(format!("token-{id}")),
            token_revision: 1,
            status: AuthorizationGrantStatus::Active,
        }
    }

    fn oauth_connection(grant_id: &str, status: &str) -> GraphqlAdapterConnection {
        GraphqlAdapterConnection {
            connection_id: format!("connection-{grant_id}"),
            status: status.to_string(),
            grant_id: Some(grant_id.to_string()),
            account_id: None,
            connection_revision: 1,
            credential_revision: None,
            grant_revision: Some(1),
            policy_revision: 1,
            granted_scopes: Vec::new(),
            allowed_operations: vec!["list_items".to_string()],
            policy_configured: true,
            operation_access: Vec::new(),
        }
    }

    #[test]
    fn connection_actions_keep_existing_accounts_and_applications_selectable() {
        let mut manifest = oauth_pending_manifest();
        manifest.reviewed = true;
        let compiled = AdapterCompiler::compile(&manifest).expect("reviewed OAuth manifest");
        let profile_digest = noema_capability_adapters::reviewed_google_oauth_profile_digest();
        let first_application = oauth_application("application-a", &profile_digest);
        let second_application = oauth_application("application-b", &profile_digest);
        let first_grant = oauth_grant("grant-a", &first_application.application_id);
        let second_grant = oauth_grant("grant-b", &first_application.application_id);
        let oauth = OauthAuthoritySnapshot {
            profiles: Vec::new(),
            applications: vec![first_application, second_application],
            accounts: Vec::new(),
            grants: vec![first_grant, second_grant],
        };
        let connections = vec![oauth_connection("grant-a", "active")];

        let actions = definition_connection_actions(
            compiled.semantic_digest.as_str(),
            &compiled,
            &manifest,
            false,
            &connections,
            &oauth,
        );

        assert_eq!(
            actions
                .iter()
                .map(|action| action.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["attach_account", "add_account", "add_account"]
        );
        assert_eq!(actions[0].grant_id.as_deref(), Some("grant-b"));
        assert!(
            actions
                .iter()
                .all(|action| action.grant_id.as_deref() != Some("grant-a"))
        );
        assert_eq!(
            actions
                .iter()
                .filter_map(|action| action.application_id.as_deref())
                .collect::<Vec<_>>(),
            vec!["application-a", "application-a", "application-b"]
        );
    }

    #[test]
    fn existing_grant_recovery_includes_exact_application_revision() {
        let mut manifest = oauth_pending_manifest();
        manifest.reviewed = true;
        let compiled = AdapterCompiler::compile(&manifest).expect("reviewed OAuth manifest");
        let profile_digest = noema_capability_adapters::reviewed_google_oauth_profile_digest();
        let application = oauth_application("application-a", &profile_digest);
        let mut grant = oauth_grant("grant-a", &application.application_id);
        grant.status = AuthorizationGrantStatus::AuthenticationRequired;
        let oauth = OauthAuthoritySnapshot {
            profiles: Vec::new(),
            applications: vec![application],
            accounts: Vec::new(),
            grants: vec![grant],
        };

        let action = definition_next_action(
            compiled.semantic_digest.as_str(),
            &compiled,
            &manifest,
            false,
            &[oauth_connection("grant-a", "authentication_required")],
            &oauth,
        )
        .expect("reconnect action");

        assert_eq!(action.kind, "reconnect_account");
        assert_eq!(action.application_id.as_deref(), Some("application-a"));
        assert_eq!(action.expected_application_revision, Some(1));
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

    async fn oauth_client_setup_interventions(
        state: &GraphqlState,
    ) -> Vec<crate::graphql::human_interventions::GraphqlAdapterOauthClientSetupIntervention> {
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
        .into_iter()
        .filter_map(|intervention| {
            match intervention {
            crate::graphql::human_interventions::GraphqlHumanIntervention::AdapterOauthClientSetup(
                setup,
            ) => Some(setup),
            _ => None,
        }
        })
        .collect()
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
    async fn approval_rejects_unknown_and_reconciles_reviewed_authority_idempotently() {
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
        let reconciled = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: reviewed.semantic_digest.clone(),
            },
        )
        .await
        .expect("idempotent reviewed approval");
        assert_eq!(reconciled.semantic_digest, reviewed.semantic_digest);
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
    async fn reviewed_replacement_supersedes_prior_setup_intervention() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths);
        let pending = definitions
            .install(
                &pending_manifest(),
                "https://developers.example.test/api-v1",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment);
        let first = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve first definition");
        let mut replacement = pending_manifest();
        replacement.definition_revision = "v2".to_string();
        replacement.operations[0].path = "/v2/items".to_string();
        let service = state.adapter_operations().expect("adapter operations");
        let catalog = CapabilityBindingSource::catalog(service)
            .await
            .expect("adapter catalog");
        let binding = catalog
            .snapshot
            .resolve("adapter.propose_definition")
            .expect("proposal binding");
        let proposal = CapabilityInvoker::invoke(
            service,
            CapabilityInvocation {
                operation: ToolName::new("adapter.propose_definition").expect("tool name"),
                operation_token: binding.target().operation_token().clone(),
                arguments: revision_proposal_arguments(
                    &replacement,
                    &first.semantic_digest,
                    "https://developers.example.test/oauth-v2",
                    &[],
                ),
                reviewed_authorization: None,
            },
        )
        .await
        .expect("replacement proposal");
        let second = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: proposal.payload["semantic_digest"]
                    .as_str()
                    .expect("replacement digest")
                    .to_string(),
            },
        )
        .await
        .expect("approve replacement");
        let visible = adapter_definitions(&state)
            .await
            .expect("visible definitions");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].semantic_digest, second.semantic_digest);
        assert!(!has_adapter_intervention(&state, &first.semantic_digest).await);
        assert!(has_adapter_intervention(&state, &second.semantic_digest).await);
    }

    #[tokio::test]
    async fn breaking_revision_migrates_one_connection_without_enabling_new_tools() {
        let (_environment, state, pending_digest) = fixture().await;
        let first = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending_digest,
            },
        )
        .await
        .expect("approve first definition");
        let service = state.adapter_operations().expect("adapter operations");
        let first_connection = first.connections.first().expect("automatic connection");
        assert_eq!(first_connection.allowed_operations, ["list_items"]);

        let mut replacement = pending_manifest();
        replacement.definition_revision = "v2".to_string();
        replacement.operations[0].operation_id = "get_item".to_string();
        replacement.operations[0].description = "Get one available item.".to_string();
        replacement.operations[0].path = "/v2/item".to_string();
        let catalog = CapabilityBindingSource::catalog(service)
            .await
            .expect("adapter catalog");
        let binding = catalog
            .snapshot
            .resolve("adapter.propose_definition")
            .expect("proposal binding");
        let proposal = CapabilityInvoker::invoke(
            service,
            CapabilityInvocation {
                operation: ToolName::new("adapter.propose_definition").expect("tool name"),
                operation_token: binding.target().operation_token().clone(),
                arguments: revision_proposal_arguments(
                    &replacement,
                    &first.semantic_digest,
                    "https://developers.example.test/items-v2",
                    &["list_items"],
                ),
                reviewed_authorization: None,
            },
        )
        .await
        .expect("replacement proposal");
        let replacement_digest = proposal.payload["semantic_digest"]
            .as_str()
            .expect("replacement digest");
        let pending = adapter_definitions(&state)
            .await
            .expect("definitions")
            .into_iter()
            .find(|definition| definition.semantic_digest == replacement_digest)
            .expect("pending replacement");
        assert_eq!(pending.transition.added_operations, ["get_item"]);
        assert_eq!(pending.transition.removed_operations, ["list_items"]);
        assert_eq!(pending.transition.affected_connections, 1);

        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: replacement_digest.to_string(),
            },
        )
        .await
        .expect("approve replacement");
        assert_eq!(reviewed.connections.len(), 1);
        assert_eq!(
            reviewed.connections[0].connection_id,
            first_connection.connection_id
        );
        assert!(reviewed.connections[0].allowed_operations.is_empty());
        assert!(matches!(
            service
                .ensure_credential_free_connection(&first.semantic_digest)
                .await,
            Err(noema_capability_adapters::AdapterConnectionSetupError::DefinitionUnavailable)
        ));
    }

    #[tokio::test]
    async fn authentication_revision_keeps_connection_and_stops_access() {
        let (_environment, state, pending_digest) = fixture().await;
        let first = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending_digest,
            },
        )
        .await
        .expect("approve first definition");
        let first_connection = first.connections.first().expect("automatic connection");
        let service = state.adapter_operations().expect("adapter operations");
        let mut replacement = oauth_pending_manifest();
        replacement.definition_id = "definition:review_fixture".to_string();
        replacement.adapter_id = "review_fixture".to_string();
        replacement.display_name = Some("Review fixture".to_string());
        replacement.definition_revision = "v2".to_string();
        let catalog = CapabilityBindingSource::catalog(service)
            .await
            .expect("adapter catalog");
        let binding = catalog
            .snapshot
            .resolve("adapter.propose_definition")
            .expect("proposal binding");
        let proposal = CapabilityInvoker::invoke(
            service,
            CapabilityInvocation {
                operation: ToolName::new("adapter.propose_definition").expect("tool name"),
                operation_token: binding.target().operation_token().clone(),
                arguments: revision_proposal_arguments(
                    &replacement,
                    &first.semantic_digest,
                    "https://developers.example.test/oauth-v2",
                    &[],
                ),
                reviewed_authorization: None,
            },
        )
        .await
        .expect("authentication proposal");
        let replacement_digest = proposal.payload["semantic_digest"]
            .as_str()
            .expect("replacement digest");
        let pending = adapter_definitions(&state)
            .await
            .expect("definitions")
            .into_iter()
            .find(|definition| definition.semantic_digest == replacement_digest)
            .expect("pending replacement");
        assert!(pending.transition.authentication_changed);
        assert_eq!(pending.transition.authentication_required_connections, 1);

        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: replacement_digest.to_string(),
            },
        )
        .await
        .expect("approve authentication replacement");
        assert_eq!(reviewed.connections.len(), 1);
        assert_eq!(
            reviewed.connections[0].connection_id,
            first_connection.connection_id
        );
        assert_eq!(reviewed.connections[0].status, "authentication_required");
        assert!(reviewed.connections[0].allowed_operations.is_empty());
    }

    #[tokio::test]
    async fn oauth_application_import_is_reusable_and_secret_safe() {
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
        state
            .adapter_operations()
            .expect("adapter operations")
            .prepare_filesystem()
            .expect("prepare adapter filesystem");
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
            .expect("OAuth application setup");
        assert_eq!(setup.credential_type, "Desktop app OAuth client");
        assert_eq!(
            setup.redirect_uri.as_deref(),
            Some("http://localhost:43123/adapter/oauth/callback")
        );
        assert_eq!(
            reviewed
                .next_action
                .as_ref()
                .map(|action| action.kind.as_str()),
            Some("import_application")
        );
        let wrong_client_type = import_adapter_oauth_application(
            &state,
            "human:local",
            GraphqlImportAdapterOauthApplicationInput {
                profile_digest: noema_capability_adapters::reviewed_google_oauth_profile_digest(),
                project_label: None,
                client_document_base64: BASE64_STANDARD.encode(
                    br#"{"web":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
                ),
            },
        )
        .await
        .expect_err("wrong callback client type");
        let message = wrong_client_type.message;
        assert!(message.contains("requested client type"));
        assert!(!message.contains("client-marker"));
        assert!(!message.contains("secret-marker"));
        let setup_interventions = oauth_client_setup_interventions(&state).await;
        assert_eq!(setup_interventions.len(), 1);
        assert_eq!(setup_interventions[0].dependent_definitions.len(), 1);
        assert_eq!(
            setup_interventions[0].dependent_definitions[0].semantic_digest,
            reviewed.semantic_digest
        );

        let upload = br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker","discard":"raw-upload-marker"}}"#;
        let imported = import_adapter_oauth_application(
            &state,
            "human:local",
            GraphqlImportAdapterOauthApplicationInput {
                profile_digest: noema_capability_adapters::reviewed_google_oauth_profile_digest(),
                project_label: Some("Personal APIs".to_string()),
                client_document_base64: BASE64_STANDARD.encode(upload),
            },
        )
        .await
        .expect("import application");
        assert_eq!(imported.client_id, "client-marker");
        assert_eq!(imported.project_label.as_deref(), Some("Personal APIs"));
        assert_eq!(imported.grant_count, 0);
        assert_eq!(imported.account_count, 0);
        let after_import = adapter_definitions(&state)
            .await
            .expect("definitions after application import")
            .into_iter()
            .find(|definition| definition.semantic_digest == reviewed.semantic_digest)
            .expect("reviewed definition after application import");
        let next_action = after_import.next_action.expect("add-account action");
        assert_eq!(next_action.kind, "add_account");
        assert_eq!(
            next_action.application_id,
            Some(imported.application_id.clone())
        );
        assert!(oauth_client_setup_interventions(&state).await.is_empty());

        let state_view = adapter_oauth_state(&state).await.expect("OAuth state");
        assert_eq!(state_view.applications.len(), 1);
        assert!(format!("{state_view:?}").contains("client-marker"));
        assert!(!format!("{state_view:?}").contains("secret-marker"));

        assert!(
            start_adapter_oauth_setup(
                &state,
                "human:other",
                GraphqlStartAdapterOauthSetupInput {
                    application_id: imported.application_id.clone(),
                    expected_application_revision: imported.revision,
                    grant_id: None,
                    expected_grant_revision: None,
                    semantic_digest: reviewed.semantic_digest.clone(),
                    operation_ids: vec!["list_items".to_string()],
                    additional_services: None,
                },
            )
            .await
            .is_err()
        );
        let started = start_adapter_oauth_setup(
            &state,
            "human:local",
            GraphqlStartAdapterOauthSetupInput {
                application_id: imported.application_id.clone(),
                expected_application_revision: imported.revision,
                grant_id: None,
                expected_grant_revision: None,
                semantic_digest: reviewed.semantic_digest,
                operation_ids: vec!["list_items".to_string()],
                additional_services: None,
            },
        )
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

        let snapshot = state
            .adapter_operations()
            .expect("adapter operations")
            .management_snapshot()
            .expect("management snapshot");
        let application = &snapshot.oauth_authorities.applications[0];
        let credential_path = paths
            .adapter_oauth_application_dir(&application.application_id)
            .expect("application path")
            .join("credentials")
            .join(format!("{}.json", application.credential_generation));
        let stored = std::fs::read_to_string(credential_path).expect("credential");
        assert!(stored.contains("secret-marker"));
        assert!(!stored.contains("raw-upload-marker"));

        let repeated = import_adapter_oauth_application(
            &state,
            "human:local",
            GraphqlImportAdapterOauthApplicationInput {
                profile_digest: noema_capability_adapters::reviewed_google_oauth_profile_digest(),
                project_label: Some("Personal APIs".to_string()),
                client_document_base64: BASE64_STANDARD.encode(upload),
            },
        )
        .await
        .expect("repeated import");
        assert_eq!(repeated.application_id, imported.application_id);
        assert_eq!(
            adapter_oauth_state(&state)
                .await
                .expect("OAuth state")
                .applications
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn oauth_client_setup_is_grouped_by_profile_after_definition_reviews() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths);
        let first = definitions
            .install(
                &oauth_pending_manifest(),
                "https://developers.example.test/oauth-a",
                None,
                None,
            )
            .expect("first pending definition");
        let mut second_manifest = oauth_pending_manifest();
        second_manifest.definition_id = "definition:oauth_review_fixture_two".to_string();
        second_manifest.adapter_id = "oauth_review_fixture_two".to_string();
        second_manifest.display_name = Some("Second OAuth fixture".to_string());
        second_manifest.operations[0].path = "/v2/items".to_string();
        let second = definitions
            .install(
                &second_manifest,
                "https://developers.example.test/oauth-b",
                None,
                None,
            )
            .expect("second pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        state
            .adapter_operations()
            .expect("adapter operations")
            .prepare_filesystem()
            .expect("prepare adapter filesystem");

        assert!(oauth_client_setup_interventions(&state).await.is_empty());
        let first_reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: first.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve first");
        let partial_interventions =
            crate::graphql::human_interventions::pending_human_interventions(
                &state,
                "human:local",
                Some("conversation:fixture".to_string()),
                None,
                None,
                Some(50),
            )
            .await
            .expect("partial interventions");
        let review_position = partial_interventions
            .iter()
            .position(|intervention| {
                matches!(
                intervention,
                crate::graphql::human_interventions::GraphqlHumanIntervention::AdapterDefinition(_)
            )
            })
            .expect("pending definition review");
        let setup_position = partial_interventions
            .iter()
            .position(|intervention| matches!(
                intervention,
                crate::graphql::human_interventions::GraphqlHumanIntervention::AdapterOauthClientSetup(_)
            ))
            .expect("shared OAuth setup");
        assert!(review_position < setup_position);
        let second_reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: second.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve second");

        let setups = oauth_client_setup_interventions(&state).await;
        assert_eq!(setups.len(), 1);
        assert_eq!(setups[0].dependent_definitions.len(), 2);
        let digests = setups[0]
            .dependent_definitions
            .iter()
            .map(|definition| definition.semantic_digest.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            digests,
            std::collections::BTreeSet::from([
                first_reviewed.semantic_digest.as_str(),
                second_reviewed.semantic_digest.as_str(),
            ])
        );
    }

    #[tokio::test]
    async fn integration_projection_keeps_reviewed_authority_when_a_newer_draft_exists() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let paths = NoemaPaths::from_noema_home(environment.root()).expect("paths");
        let definitions = AdapterDefinitionStore::new(paths.clone());
        let pending = definitions
            .install(
                &pending_manifest(),
                "https://developers.example.test/api-v1",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        state
            .adapter_operations()
            .expect("adapter operations")
            .prepare_filesystem()
            .expect("prepare adapter filesystem");
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        let mut newer_draft = pending_manifest();
        newer_draft.definition_revision = "v2".to_string();
        newer_draft.operations[0].path = "/v2/items".to_string();
        definitions
            .install(
                &newer_draft,
                "https://developers.example.test/api-v2",
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
            .find(|integration| integration.definition_id == "definition:review_fixture")
            .expect("API integration");
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
                &pending_manifest(),
                "https://developers.example.test/api",
                None,
                None,
            )
            .expect("pending definition");
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment);
        let reviewed = approve_adapter_definition(
            &state,
            "human:local",
            GraphqlApproveAdapterDefinitionInput {
                semantic_digest: pending.compiled.semantic_digest.to_string(),
            },
        )
        .await
        .expect("approve");
        let connection = &reviewed.connections[0];
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
                &pending_manifest(),
                "https://developers.example.test/api",
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
        let repeated_connection = repeated_reviewed
            .connections
            .first()
            .expect("repeated credential-free connection");
        assert!(
            delete_adapter_connection(
                &state,
                "human:local",
                GraphqlDeleteAdapterConnectionInput {
                    connection_id: repeated_connection.connection_id.clone(),
                    expected_connection_revision: repeated_connection.connection_revision,
                },
            )
            .await
            .expect("delete repeated connection")
        );
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
    async fn concurrent_imports_reuse_one_oauth_application() {
        let environment = crate::test_support::TestEnvironment::new();
        let store = crate::test_support::test_store_for_environment(&environment).await;
        let state = GraphqlState::for_tests_with_store_and_environment(store, environment)
            .with_adapter_oauth_callback_url("http://localhost:43123/adapter/oauth/callback");
        state
            .adapter_operations()
            .expect("adapter operations")
            .prepare_filesystem()
            .expect("prepare adapter filesystem");
        state
            .adapter_operations()
            .expect("adapter operations")
            .set_oauth_callback_mode(Oauth2CallbackMode::Loopback);
        let upload = BASE64_STANDARD.encode(
            br#"{"installed":{"client_id":"client-marker","client_secret":"secret-marker"}}"#,
        );
        let input = GraphqlImportAdapterOauthApplicationInput {
            profile_digest: noema_capability_adapters::reviewed_google_oauth_profile_digest(),
            project_label: None,
            client_document_base64: upload,
        };
        let (first, second) = tokio::join!(
            import_adapter_oauth_application(&state, "human:local", input.clone()),
            import_adapter_oauth_application(&state, "human:local", input),
        );
        let first = first.expect("first import");
        let second = second.expect("second import");
        assert_eq!(first.application_id, second.application_id);
        assert_eq!(
            adapter_oauth_state(&state)
                .await
                .expect("OAuth state")
                .applications
                .len(),
            1
        );
    }
}
