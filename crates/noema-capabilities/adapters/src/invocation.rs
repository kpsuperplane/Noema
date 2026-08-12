//! Adapter invocation with live filesystem authority revalidation.

use crate::{
    AdapterCapabilityService, AdapterConnectionAuthenticationV1, AdapterConnectionStatus,
    AdapterConnectionV4, AdapterCredentialGenerationV2, AdapterManagementError,
    AdapterManagementFence, AuthenticationMode, AuthorizationGrantStatus, AuthorizationGrantV1,
    CompiledAdapterDefinition, CompiledOperation, CursorBinding, OauthApplicationCredentialV1,
    OauthApplicationV1, OauthGrantTokenV1, OauthProfileV1, PaginationPolicy,
    catalog::{AdapterOperationAuthorityV1, canonical_name, effective_behavior},
    network::{
        AdapterBearerCredential, AdapterHttpError, AdapterHttpResponse, AdapterOAuthTokenError,
        AdapterOAuthTokenGrant, AdapterOAuthTokenRequest,
    },
    request::{apply_credential_auth, encode_request, inject_pagination_token},
};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeKind, CapabilityError, CapabilityExecutionDecision,
    CapabilityFailure, CapabilityFailureKind, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput, CapabilityRecovery, resolve_capability_execution_decision,
    sanitize_standard_credentials_with_additional_names, tool_enablement_name,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

const TOKEN_REFRESH_SKEW_SECONDS: u64 = 60;

impl CapabilityInvoker for AdapterCapabilityService {
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async move { self.invoke_capability(invocation).await })
    }
}

impl AdapterCapabilityService {
    async fn invoke_capability(
        &self,
        invocation: CapabilityInvocation,
    ) -> Result<CapabilityOutput, CapabilityError> {
        if crate::setup::is_definition_template_invocation(
            invocation.operation.as_str(),
            &invocation.operation_token,
        ) {
            return self.definition_template(invocation.arguments);
        }
        if crate::setup::is_proposal_invocation(
            invocation.operation.as_str(),
            &invocation.operation_token,
        ) {
            return self.propose_definition(invocation.arguments);
        }
        let authority =
            AdapterOperationAuthorityV1::from_operation_token(&invocation.operation_token)
                .map_err(|_| CapabilityError::UnknownOperation)?;
        let disabled_name = noema_capabilities::ToolName::new(&authority.canonical_name)
            .map_err(|_| CapabilityError::UnknownOperation)?;
        if tool_enablement_name(&disabled_name).is_ok_and(|name| name == invocation.operation) {
            return self.enable_disabled_tool(invocation, authority).await;
        }
        if invocation.operation.as_str() != authority.canonical_name {
            return Err(CapabilityError::UnknownOperation);
        }

        let lock_id = authority.grant_id.as_deref().map_or_else(
            || authority.connection_id.clone(),
            |grant_id| format!("oauth-grant:{grant_id}"),
        );
        let lock = self
            .connection_lock(&lock_id)
            .map_err(|_| CapabilityError::Unavailable)?;
        let mut _guard = Some(lock.read().await);
        let mut current = {
            let service = self.clone();
            let authority = authority.clone();
            tokio::task::spawn_blocking(move || service.current_plan_with_credential(&authority))
                .await
                .map_err(|_| CapabilityError::Unavailable)??
        };
        match current.connection.status {
            AdapterConnectionStatus::Suspended => return Err(CapabilityError::Denied),
            AdapterConnectionStatus::AuthenticationRequired => {
                return Err(authentication_required(&authority, current.auth_mode));
            }
            AdapterConnectionStatus::Active => {}
        }
        if current.auth_mode == AuthenticationMode::Oauth2AuthorizationCodePkce
            && token_needs_refresh(current.oauth_token.as_ref(), current_epoch_seconds()?)
        {
            _guard.take();
            self.refresh_oauth_if_needed(&authority, None).await?;
            _guard = Some(lock.read().await);
            current = {
                let service = self.clone();
                let authority = authority.clone();
                tokio::task::spawn_blocking(move || {
                    service.current_plan_with_credential(&authority)
                })
                .await
                .map_err(|_| CapabilityError::Unavailable)??
            };
        }
        let (model_arguments, continuation_reference) =
            split_continuation_arguments(&invocation.arguments, &current.operation.pagination)?;
        let arguments_sha256 = crate::digest::canonical_value_sha256(&model_arguments)
            .map_err(|_| CapabilityError::InvalidArguments)?;
        if current.execution_decision()?.requires_review() {
            let Some(authorization) = invocation.reviewed_authorization.as_ref() else {
                return Err(CapabilityError::Denied);
            };
            if authorization.action_id == "observed_url"
                || !authorization.matches_arguments(&invocation.arguments)
            {
                return Err(CapabilityError::Denied);
            }
        }
        let mut request = encode_request(&current.definition, &current.operation, &model_arguments)
            .map_err(|_| CapabilityError::InvalidArguments)?;
        let now_epoch_seconds = current_epoch_seconds()?;
        let cursor_binding = CursorBinding {
            connection_id: current.connection.connection_id.clone(),
            semantic_digest: current.definition.semantic_digest.to_string(),
            operation_id: current.operation.operation_id.clone(),
            grant_id: current.grant.as_ref().map(|grant| grant.grant_id.clone()),
            account_id: current
                .grant
                .as_ref()
                .and_then(|grant| grant.account_id.clone()),
            grant_revision: current
                .grant
                .as_ref()
                .map_or(current.connection.connection_revision, |grant| {
                    grant.authority_revision
                }),
            arguments_sha256,
        };
        let _cursor_guard = if continuation_reference.is_some() {
            Some(self.inner.cursor_lock.lock().await)
        } else {
            None
        };
        let bearer = match current.auth_mode {
            AuthenticationMode::None => None,
            AuthenticationMode::Credential => {
                let credential = current
                    .credential
                    .as_ref()
                    .ok_or_else(|| authentication_required(&authority, current.auth_mode))?;
                let material = credential.material.clone();
                let definition = current.definition.clone();
                let operation = current.operation.clone();
                request = tokio::task::spawn_blocking(move || {
                    apply_credential_auth(&definition, &operation, &mut request, &material)
                        .map(|()| request)
                })
                .await
                .map_err(|_| CapabilityError::Unavailable)?
                .map_err(|_| CapabilityError::Failed)?;
                None
            }
            AuthenticationMode::Oauth2AuthorizationCodePkce => {
                Some(Self::oauth_bearer(&current, &authority)?)
            }
        };
        if let Some(reference) = continuation_reference.as_deref() {
            let (_, token) = self
                .inner
                .cursors
                .resolve(reference, &cursor_binding, now_epoch_seconds)
                .map_err(|_| CapabilityError::InvalidArguments)?;
            inject_pagination_token(&mut request, &current.operation, token.as_str())
                .map_err(|_| CapabilityError::InvalidArguments)?;
        }

        let behavior = effective_behavior(&current.connection, &current.operation)
            .map_err(|_| CapabilityError::UnknownOperation)?;
        let retry = if behavior.idempotent {
            current.operation.retry
        } else {
            crate::RetryPolicy::Never
        };
        let additional_sensitive_fields = request
            .sensitive_headers
            .keys()
            .chain(request.sensitive_query_names.iter())
            .map(|name| name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let additional_sensitive_query_names = request.sensitive_query_names.clone();
        let retry_request = request.clone();
        let mut response = map_http_result(
            self.inner
                .http
                .execute(current.operation.method, retry, request, bearer)
                .await,
            behavior.read_only,
        )?;
        if response.status == 401 {
            if current.auth_mode == AuthenticationMode::None {
                return Err(CapabilityError::Failed);
            }
            if current.auth_mode == AuthenticationMode::Credential {
                return Err(authentication_required(&authority, current.auth_mode));
            }
            let used_generation = current
                .oauth_token
                .as_ref()
                .map(|token| token.generation_id.clone());
            drop(_guard.take());
            self.refresh_oauth_if_needed(&authority, used_generation.as_deref())
                .await?;
            if !behavior.read_only && !behavior.idempotent {
                return Err(CapabilityError::Failed);
            }
            _guard = Some(lock.read().await);
            current = {
                let service = self.clone();
                let authority = authority.clone();
                tokio::task::spawn_blocking(move || {
                    service.current_plan_with_credential(&authority)
                })
                .await
                .map_err(|_| CapabilityError::Unavailable)??
            };
            let bearer = Self::oauth_bearer(&current, &authority)?;
            response = map_http_result(
                self.inner
                    .http
                    .execute(current.operation.method, retry, retry_request, Some(bearer))
                    .await,
                behavior.read_only,
            )?;
            if response.status == 401 {
                return Err(authentication_required(&authority, current.auth_mode));
            }
        }
        if (300..400).contains(&response.status) {
            return invalid_response(behavior.read_only);
        }
        if !(200..300).contains(&response.status) {
            if response.status >= 500 && !behavior.read_only {
                return Err(CapabilityError::OutcomeUncertain);
            }
            return Ok(remote_failure(
                response.status,
                crate::response::json(&response).ok().as_ref(),
                &additional_sensitive_fields,
                &additional_sensitive_query_names,
            ));
        }
        let (next_provider_token, response) = match &current.operation.pagination {
            PaginationPolicy::ResponseToken {
                response_pointer, ..
            } => crate::response::extract_pagination_token(&response, response_pointer).map_err(
                |_| {
                    if behavior.read_only {
                        CapabilityError::Failed
                    } else {
                        CapabilityError::OutcomeUncertain
                    }
                },
            )?,
            _ => (None, response),
        };
        let mut payload =
            match crate::response::success(&response, &current.operation.response).await {
                Ok(payload) => payload,
                Err(AdapterHttpError::ResponseTransformFailed(message)) if behavior.read_only => {
                    return Ok(CapabilityOutput::failed(json!({
                        "error": "response_transform_failed",
                        "message": message
                    })));
                }
                Err(_) if behavior.read_only => return Err(CapabilityError::Failed),
                Err(_) => return Err(CapabilityError::OutcomeUncertain),
            };
        if matches!(
            current.operation.pagination,
            PaginationPolicy::ResponseToken { .. }
        ) {
            let next_reference = if let Some(token) = next_provider_token {
                let reference =
                    crate::private_fs::random_hex(24).map_err(|_| CapabilityError::Unavailable)?;
                let handle = crate::CursorHandle {
                    secret_reference: reference.clone(),
                    binding: cursor_binding,
                    expires_at_epoch_seconds: now_epoch_seconds.saturating_add(60 * 60),
                };
                self.inner
                    .cursors
                    .put(&handle, &token)
                    .map_err(|_| CapabilityError::Unavailable)?;
                Some(reference)
            } else {
                None
            };
            payload = crate::response::inject_continuation(payload, next_reference.as_deref())
                .map_err(|_| CapabilityError::Failed)?;
            if let Some(reference) = continuation_reference.as_deref() {
                self.inner
                    .cursors
                    .retire(reference)
                    .map_err(|_| CapabilityError::Unavailable)?;
            }
        }
        Ok(CapabilityOutput::success(
            sanitize_standard_credentials_with_additional_names(
                &payload,
                &additional_sensitive_fields,
                &additional_sensitive_query_names,
            ),
        ))
    }

    async fn enable_disabled_tool(
        &self,
        invocation: CapabilityInvocation,
        authority: AdapterOperationAuthorityV1,
    ) -> Result<CapabilityOutput, CapabilityError> {
        if !invocation
            .arguments
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            return Err(CapabilityError::InvalidArguments);
        }
        let Some(authorization) = invocation.reviewed_authorization.as_ref() else {
            return Err(CapabilityError::Denied);
        };
        if !authorization.matches_arguments(&invocation.arguments) {
            return Err(CapabilityError::Denied);
        }
        self.set_management_tool_enabled(
            AdapterManagementFence {
                connection_id: authority.connection_id.clone(),
                expected_connection_revision: authority.connection_revision,
                expected_policy_revision: authority.policy_revision,
            },
            authority.operation_id.clone(),
            authority.operation_digest.clone(),
            true,
        )
        .await
        .map_err(|error| match error {
            AdapterManagementError::Unavailable => CapabilityError::Unavailable,
            AdapterManagementError::NotFound
            | AdapterManagementError::Invalid
            | AdapterManagementError::Conflict => CapabilityError::UnknownOperation,
        })?;
        Ok(CapabilityOutput::success(json!({
            "enabled_capability": authority.canonical_name
        })))
    }

    fn current_plan_with_credential(
        &self,
        authority: &AdapterOperationAuthorityV1,
    ) -> Result<CurrentPlan, CapabilityError> {
        let definitions = self
            .definition_registry()
            .map_err(|_| CapabilityError::Unavailable)?;
        let definition = definitions
            .definitions
            .iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == authority.semantic_digest
            })
            .cloned()
            .ok_or(CapabilityError::UnknownOperation)?;
        let operation = definition
            .compiled
            .operations
            .iter()
            .find(|operation| operation.operation_id == authority.operation_id)
            .cloned()
            .ok_or(CapabilityError::UnknownOperation)?;
        let (descriptor, credential) = self
            .inner
            .connections
            .load_for_invocation(&authority.connection_id, &definition.compiled)
            .map_err(|_| CapabilityError::UnknownOperation)?;
        let (grant, oauth_token, application, application_credential, profile) =
            match &descriptor.authentication {
                AdapterConnectionAuthenticationV1::OauthGrant { grant_id } => {
                    let (grant, token) = self
                        .inner
                        .oauth_authorities
                        .load_grant_authority(grant_id)
                        .map_err(|_| CapabilityError::UnknownOperation)?;
                    let (application, application_credential) = self
                        .inner
                        .oauth_authorities
                        .load_application_authority(&grant.application_id)
                        .map_err(|_| CapabilityError::UnknownOperation)?;
                    let profile = self
                        .inner
                        .oauth_authorities
                        .load_profile(&application.profile_digest)
                        .map_err(|_| CapabilityError::UnknownOperation)?
                        .profile;
                    (
                        Some(grant),
                        token,
                        Some(application),
                        Some(application_credential),
                        Some(profile),
                    )
                }
                AdapterConnectionAuthenticationV1::Pending
                | AdapterConnectionAuthenticationV1::None
                | AdapterConnectionAuthenticationV1::Credential { .. } => {
                    (None, None, None, None, None)
                }
            };
        if !authority_matches(
            authority,
            &definition.compiled,
            &descriptor,
            &operation,
            grant.as_ref(),
        ) {
            return Err(CapabilityError::UnknownOperation);
        }
        if grant.as_ref().is_some_and(|grant| {
            grant.status != AuthorizationGrantStatus::Active
                || !operation
                    .authorization
                    .is_satisfied_by(&grant.granted_scopes)
        }) {
            return Err(authentication_required(
                authority,
                AuthenticationMode::Oauth2AuthorizationCodePkce,
            ));
        }
        Ok(CurrentPlan {
            auth_mode: definition.compiled.authentication.mode(),
            definition: definition.compiled,
            connection: descriptor,
            operation,
            credential,
            grant,
            oauth_token,
            application,
            application_credential,
            profile,
        })
    }

    async fn refresh_oauth_if_needed(
        &self,
        authority: &AdapterOperationAuthorityV1,
        force_generation: Option<&str>,
    ) -> Result<(), CapabilityError> {
        let lock_id = authority.grant_id.as_deref().map_or_else(
            || authority.connection_id.clone(),
            |grant_id| format!("oauth-grant:{grant_id}"),
        );
        let lock = self
            .connection_lock(&lock_id)
            .map_err(|_| CapabilityError::Unavailable)?;
        let _guard = lock.write().await;
        let current = {
            let service = self.clone();
            let authority = authority.clone();
            tokio::task::spawn_blocking(move || service.current_plan_with_credential(&authority))
                .await
                .map_err(|_| CapabilityError::Unavailable)??
        };
        match current.connection.status {
            AdapterConnectionStatus::Suspended => return Err(CapabilityError::Denied),
            AdapterConnectionStatus::AuthenticationRequired => {
                return Err(authentication_required(authority, current.auth_mode));
            }
            AdapterConnectionStatus::Active => {}
        }
        if current.auth_mode != AuthenticationMode::Oauth2AuthorizationCodePkce {
            return force_generation.map_or(Ok(()), |_| {
                Err(authentication_required(authority, current.auth_mode))
            });
        }
        let generation_matches = force_generation.is_some_and(|generation| {
            current
                .oauth_token
                .as_ref()
                .map(|token| token.generation_id.as_str())
                == Some(generation)
        });
        let now_epoch_seconds = current_epoch_seconds()?;
        if force_generation.is_some() && !generation_matches
            || force_generation.is_none()
                && !token_needs_refresh(current.oauth_token.as_ref(), now_epoch_seconds)
        {
            return Ok(());
        }
        let Some(grant) = current.grant.as_ref() else {
            return Err(authentication_required(authority, current.auth_mode));
        };
        let Some(OauthGrantTokenV1 {
            refresh_token: Some(refresh_token),
            ..
        }) = current.oauth_token.as_ref()
        else {
            return Err(authentication_required(authority, current.auth_mode));
        };
        let application = current
            .application
            .as_ref()
            .ok_or(CapabilityError::Failed)?;
        let application_credential = current
            .application_credential
            .as_ref()
            .ok_or(CapabilityError::Failed)?;
        let profile = current.profile.as_ref().ok_or(CapabilityError::Failed)?;
        let token_result = self
            .inner
            .http
            .exchange_oauth_token(AdapterOAuthTokenRequest {
                token_endpoint: url::Url::parse(&profile.token_endpoint)
                    .map_err(|_| CapabilityError::Failed)?,
                client_authentication: profile.client_authentication,
                client_id: application.client_id.clone(),
                client_secret: application_credential.client_secret.clone(),
                grant: AdapterOAuthTokenGrant::RefreshToken {
                    refresh_token: refresh_token.clone(),
                },
                expected_scopes: grant.granted_scopes.clone(),
                omitted_scope_policy: profile.omitted_scope_policy,
                now_epoch_seconds,
            })
            .await;
        let token = match token_result {
            Ok(token) => token,
            Err(AdapterOAuthTokenError::Rejected) => {
                self.inner
                    .oauth_authorities
                    .deactivate_grant(grant, AuthorizationGrantStatus::AuthenticationRequired)
                    .map_err(|_| CapabilityError::Unavailable)?;
                return Err(authentication_required(authority, current.auth_mode));
            }
            Err(AdapterOAuthTokenError::Unavailable) => return Err(CapabilityError::Unavailable),
            Err(
                AdapterOAuthTokenError::InvalidRequest | AdapterOAuthTokenError::InvalidResponse,
            ) => {
                return Err(CapabilityError::Failed);
            }
        };
        let generation_id =
            crate::private_fs::random_hex(16).map_err(|_| CapabilityError::Unavailable)?;
        let grant_token = OauthGrantTokenV1 {
            schema_version: 1,
            generation_id: generation_id.clone(),
            access_token: token.access_token,
            refresh_token: token.refresh_token.or_else(|| Some(refresh_token.clone())),
            expires_at_epoch_seconds: token.expires_at_epoch_seconds,
        };
        let mut replacement = grant.clone();
        replacement.token_revision = replacement
            .token_revision
            .checked_add(1)
            .ok_or(CapabilityError::Unavailable)?;
        replacement.token_generation = Some(generation_id);
        replacement.granted_scopes = token.granted_scopes;
        self.inner
            .oauth_authorities
            .refresh_grant(grant, &replacement, &grant_token)
            .map_err(|_| CapabilityError::Unavailable)?;
        Ok(())
    }

    fn oauth_bearer(
        current: &CurrentPlan,
        authority: &AdapterOperationAuthorityV1,
    ) -> Result<AdapterBearerCredential, CapabilityError> {
        current
            .oauth_token
            .as_ref()
            .map(|token| AdapterBearerCredential::new(token.access_token.clone()))
            .ok_or_else(|| authentication_required(authority, current.auth_mode))
    }
}

fn split_continuation_arguments(
    arguments: &Value,
    pagination: &PaginationPolicy,
) -> Result<(Value, Option<String>), CapabilityError> {
    if !matches!(pagination, PaginationPolicy::ResponseToken { .. }) {
        return Ok((arguments.clone(), None));
    }
    let mut arguments = match arguments {
        Value::Object(arguments) => arguments.clone(),
        Value::Null => serde_json::Map::new(),
        _ => return Err(CapabilityError::InvalidArguments),
    };
    let continuation = arguments.remove("continuation");
    let continuation = match continuation {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if !value.is_empty() && value.len() <= 128 => Some(value),
        Some(_) => return Err(CapabilityError::InvalidArguments),
    };
    Ok((Value::Object(arguments), continuation))
}

fn current_epoch_seconds() -> Result<u64, CapabilityError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|now| now.as_secs())
        .map_err(|_| CapabilityError::Unavailable)
}

fn invalid_response(read_only: bool) -> Result<CapabilityOutput, CapabilityError> {
    Err(if read_only {
        CapabilityError::Failed
    } else {
        CapabilityError::OutcomeUncertain
    })
}

fn remote_failure(
    status: u16,
    payload: Option<&serde_json::Value>,
    additional_sensitive_fields: &BTreeSet<String>,
    additional_sensitive_query_names: &BTreeSet<String>,
) -> CapabilityOutput {
    let mut failure = json!({"error": "remote_request_failed", "status": status});
    if let Some(payload) = payload {
        failure["response"] = sanitize_standard_credentials_with_additional_names(
            payload,
            additional_sensitive_fields,
            additional_sensitive_query_names,
        );
    }
    if serde_json::to_vec(&failure).map_or(true, |bytes| bytes.len() > 4 * 1024) {
        failure = json!({
            "error": "remote_request_failed",
            "status": status,
            "response_omitted": "too_large"
        });
    }
    let semantics = remote_failure_semantics(status);
    let output = CapabilityOutput::failed_with_recovery(failure, semantics);
    if serde_json::to_vec(&output.payload).is_ok_and(|bytes| bytes.len() <= 4 * 1024) {
        output
    } else {
        CapabilityOutput::failed_with_recovery(
            json!({
                "error": "remote_request_failed",
                "status": status,
                "response_omitted": "too_large"
            }),
            semantics,
        )
    }
}

fn remote_failure_semantics(status: u16) -> CapabilityFailure {
    let (kind, recovery) = match status {
        400 | 405 | 406 | 411 | 413..=415 | 422 => (
            CapabilityFailureKind::InvalidRequest,
            CapabilityRecovery::CorrectArguments,
        ),
        403 => (
            CapabilityFailureKind::PermissionDenied,
            CapabilityRecovery::Stop,
        ),
        404 | 410 => (
            CapabilityFailureKind::ResourceNotFound,
            CapabilityRecovery::ResolveResource,
        ),
        409 | 412 => (
            CapabilityFailureKind::Conflict,
            CapabilityRecovery::ResolveResource,
        ),
        429 => (
            CapabilityFailureKind::RateLimited,
            CapabilityRecovery::RetryLater,
        ),
        500..=599 => (
            CapabilityFailureKind::RemoteUnavailable,
            CapabilityRecovery::RetryLater,
        ),
        _ => (
            CapabilityFailureKind::RemoteRejected,
            CapabilityRecovery::Stop,
        ),
    };
    CapabilityFailure { kind, recovery }
}

struct CurrentPlan {
    definition: CompiledAdapterDefinition,
    connection: AdapterConnectionV4,
    operation: CompiledOperation,
    auth_mode: AuthenticationMode,
    credential: Option<AdapterCredentialGenerationV2>,
    grant: Option<AuthorizationGrantV1>,
    oauth_token: Option<OauthGrantTokenV1>,
    application: Option<OauthApplicationV1>,
    application_credential: Option<OauthApplicationCredentialV1>,
    profile: Option<OauthProfileV1>,
}

impl CurrentPlan {
    fn execution_decision(&self) -> Result<CapabilityExecutionDecision, CapabilityError> {
        let policy = self.connection.policy.ok_or(CapabilityError::Denied)?;
        let behavior = effective_behavior(&self.connection, &self.operation)
            .map_err(|_| CapabilityError::UnknownOperation)?;
        Ok(resolve_capability_execution_decision(policy, behavior))
    }
}

fn authority_matches(
    authority: &AdapterOperationAuthorityV1,
    definition: &CompiledAdapterDefinition,
    descriptor: &AdapterConnectionV4,
    operation: &CompiledOperation,
    grant: Option<&AuthorizationGrantV1>,
) -> bool {
    canonical_name(
        &definition.adapter_id,
        &descriptor.connection_slug,
        &operation.operation_id,
    )
    .is_ok_and(|name| name.as_str() == authority.canonical_name)
        && descriptor.connection_id == authority.connection_id
        && descriptor.connection_slug == authority.connection_slug
        && descriptor.semantic_digest == authority.semantic_digest
        && descriptor.policy_revision == authority.policy_revision
        && descriptor
            .policy
            .is_some_and(|policy| policy.revision == authority.policy_revision)
        && descriptor.connection_revision == authority.connection_revision
        && authentication_authority_matches(authority, descriptor, grant)
        && descriptor
            .allowed_operations
            .contains(&authority.operation_id)
        && operation.operation_digest.as_str() == authority.operation_digest
        && operation.token.as_str() == authority.definition_token
}

fn authentication_authority_matches(
    authority: &AdapterOperationAuthorityV1,
    descriptor: &AdapterConnectionV4,
    grant: Option<&AuthorizationGrantV1>,
) -> bool {
    match (&descriptor.authentication, grant) {
        (AdapterConnectionAuthenticationV1::Pending, None) => false,
        (AdapterConnectionAuthenticationV1::None, None) => {
            authority.credential_revision.is_none() && authority.grant_id.is_none()
        }
        (AdapterConnectionAuthenticationV1::Credential { revision, .. }, None) => {
            authority.credential_revision == Some(*revision) && authority.grant_id.is_none()
        }
        (AdapterConnectionAuthenticationV1::OauthGrant { grant_id }, Some(grant)) => {
            authority.credential_revision.is_none()
                && authority.grant_id.as_deref() == Some(grant_id)
                && authority.grant_authority_revision == Some(grant.authority_revision)
                && authority.account_id == grant.account_id
        }
        _ => false,
    }
}

fn token_needs_refresh(token: Option<&OauthGrantTokenV1>, now_epoch_seconds: u64) -> bool {
    let Some(OauthGrantTokenV1 {
        expires_at_epoch_seconds: Some(expires_at),
        ..
    }) = token
    else {
        return false;
    };
    now_epoch_seconds.saturating_add(TOKEN_REFRESH_SKEW_SECONDS) >= *expires_at
}

fn map_http_result(
    result: Result<AdapterHttpResponse, AdapterHttpError>,
    read_only: bool,
) -> Result<AdapterHttpResponse, CapabilityError> {
    result.map_err(|error| match error {
        AdapterHttpError::Unavailable => CapabilityError::Unavailable,
        AdapterHttpError::OutcomeUncertain if read_only => CapabilityError::Unavailable,
        AdapterHttpError::OutcomeUncertain => CapabilityError::OutcomeUncertain,
        AdapterHttpError::InvalidResponse if read_only => CapabilityError::Failed,
        AdapterHttpError::InvalidResponse => CapabilityError::OutcomeUncertain,
        AdapterHttpError::ResponseTransformFailed(_) if read_only => CapabilityError::Failed,
        AdapterHttpError::ResponseTransformFailed(_) => CapabilityError::OutcomeUncertain,
    })
}

fn authentication_required(
    authority: &AdapterOperationAuthorityV1,
    auth_mode: AuthenticationMode,
) -> CapabilityError {
    let challenge_kind = match auth_mode {
        AuthenticationMode::Credential => CapabilityAuthenticationChallengeKind::ReplaceCredential,
        AuthenticationMode::Oauth2AuthorizationCodePkce | AuthenticationMode::None => {
            CapabilityAuthenticationChallengeKind::Reauthenticate
        }
    };
    let challenge = if let Some(grant_id) = &authority.grant_id {
        CapabilityAuthenticationChallenge::new_for_destination(
            challenge_kind,
            CapabilityAuthenticationAuthorityKind::AdapterGrant,
            grant_id.clone(),
            authority.connection_id.clone(),
            authority.destination_revision(),
        )
    } else {
        CapabilityAuthenticationChallenge::new(
            challenge_kind,
            CapabilityAuthenticationAuthorityKind::AdapterConnection,
            authority.connection_id.clone(),
            authority.destination_revision(),
        )
    };
    CapabilityError::AuthenticationRequired {
        challenge: challenge.expect("validated adapter authority is bounded"),
    }
}
