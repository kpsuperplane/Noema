//! Adapter invocation with live filesystem authority revalidation.

use crate::{
    AdapterCapabilityService, AdapterConnectionStatus, AdapterConnectionV3,
    AdapterCredentialGenerationV2, AdapterCredentialMaterial, AuthenticationMode,
    CompiledAdapterDefinition, CompiledOperation, CursorBinding, PaginationPolicy,
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
    CapabilityFuture, CapabilityInvocation, CapabilityInvoker, CapabilityOutput, PayloadSanitizer,
    RedactingPayloadSanitizer, resolve_capability_execution_decision,
};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

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
        if invocation.operation.as_str() != authority.canonical_name {
            return Err(CapabilityError::UnknownOperation);
        }

        let preliminary = {
            let service = self.clone();
            let authority = authority.clone();
            tokio::task::spawn_blocking(move || service.current_plan(&authority))
                .await
                .map_err(|_| CapabilityError::Unavailable)??
        };
        let (model_arguments, continuation_reference) =
            split_continuation_arguments(&invocation.arguments, &preliminary.operation.pagination)?;
        let arguments_sha256 = crate::digest::canonical_value_sha256(&model_arguments)
            .map_err(|_| CapabilityError::InvalidArguments)?;
        if preliminary.execution_decision()?.requires_review() {
            let Some(authorization) = invocation.reviewed_authorization.as_ref() else {
                return Err(CapabilityError::Denied);
            };
            if authorization.action_id == "observed_url"
                || !authorization.matches_arguments(&invocation.arguments)
            {
                return Err(CapabilityError::Denied);
            }
        }
        let mut request = encode_request(
            &preliminary.definition,
            &preliminary.operation,
            &model_arguments,
        )
        .map_err(|_| CapabilityError::InvalidArguments)?;

        self.refresh_oauth_if_needed(&authority, None).await?;
        let lock = self
            .connection_lock(&authority.connection_id)
            .map_err(|_| CapabilityError::Unavailable)?;
        let mut _guard = Some(lock.read().await);
        let mut current = {
            let service = self.clone();
            let authority = authority.clone();
            tokio::task::spawn_blocking(move || service.current_plan_with_credential(&authority))
                .await
                .map_err(|_| CapabilityError::Unavailable)??
        };
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
        match current.connection.status {
            AdapterConnectionStatus::Suspended => return Err(CapabilityError::Denied),
            AdapterConnectionStatus::AuthenticationRequired => {
                return Err(authentication_required(&authority, current.auth_mode));
            }
            AdapterConnectionStatus::Active => {}
        }
        let now_epoch_seconds = current_epoch_seconds()?;
        let cursor_binding = CursorBinding {
            connection_id: current.connection.connection_id.clone(),
            semantic_digest: current.definition.semantic_digest.to_string(),
            operation_id: current.operation.operation_id.clone(),
            account_kind: current.connection.account_kind.clone(),
            grant_revision: current.connection.revisions.grant,
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
                Some(self.oauth_bearer(&current, &authority)?)
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
            let used_generation = current.connection.credential_generation.clone();
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
            let bearer = self.oauth_bearer(&current, &authority)?;
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
        if response.status == 429 {
            return Err(CapabilityError::Unavailable);
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
        let mut payload = crate::response::success(&response, &current.operation.response)
            .await
            .map_err(|_| {
                if behavior.read_only {
                    CapabilityError::Failed
                } else {
                    CapabilityError::OutcomeUncertain
                }
            })?;
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
            RedactingPayloadSanitizer
                .persist_output(&payload)
                .unwrap_or_else(|| json!({"error": "response_redacted"})),
        ))
    }

    fn current_plan(
        &self,
        authority: &AdapterOperationAuthorityV1,
    ) -> Result<CurrentPlan, CapabilityError> {
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| CapabilityError::Unavailable)?;
        let definition = definitions
            .definitions
            .into_iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == authority.semantic_digest
            })
            .ok_or(CapabilityError::UnknownOperation)?;
        let connections = self
            .inner
            .connections
            .scan(std::slice::from_ref(&definition))
            .map_err(|_| CapabilityError::Unavailable)?;
        let connection = connections
            .connections
            .into_iter()
            .find(|connection| connection.descriptor.connection_id == authority.connection_id)
            .ok_or(CapabilityError::UnknownOperation)?;
        let operation = definition
            .compiled
            .operations
            .iter()
            .find(|operation| operation.operation_id == authority.operation_id)
            .cloned()
            .ok_or(CapabilityError::UnknownOperation)?;
        if !authority_matches(
            authority,
            &definition.compiled,
            &connection.descriptor,
            &operation,
        ) {
            return Err(CapabilityError::UnknownOperation);
        }
        Ok(CurrentPlan {
            auth_mode: definition.compiled.authentication.mode(),
            definition: definition.compiled,
            connection: connection.descriptor,
            operation,
            credential: None,
        })
    }

    fn current_plan_with_credential(
        &self,
        authority: &AdapterOperationAuthorityV1,
    ) -> Result<CurrentPlan, CapabilityError> {
        let definitions = self
            .inner
            .definitions
            .scan()
            .map_err(|_| CapabilityError::Unavailable)?;
        let definition = definitions
            .definitions
            .into_iter()
            .find(|definition| {
                definition.compiled.semantic_digest.as_str() == authority.semantic_digest
            })
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
        if !authority_matches(authority, &definition.compiled, &descriptor, &operation) {
            return Err(CapabilityError::UnknownOperation);
        }
        Ok(CurrentPlan {
            auth_mode: definition.compiled.authentication.mode(),
            definition: definition.compiled,
            connection: descriptor,
            operation,
            credential,
        })
    }

    async fn refresh_oauth_if_needed(
        &self,
        authority: &AdapterOperationAuthorityV1,
        force_generation: Option<&str>,
    ) -> Result<(), CapabilityError> {
        let lock = self
            .connection_lock(&authority.connection_id)
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
            current.connection.credential_generation.as_deref() == Some(generation)
        });
        let now_epoch_seconds = current_epoch_seconds()?;
        if force_generation.is_some() && !generation_matches
            || force_generation.is_none()
                && !credential_needs_refresh(current.credential.as_ref(), now_epoch_seconds)
        {
            return Ok(());
        }
        let Some(AdapterCredentialGenerationV2 {
            material:
                AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                    callback_mode,
                    client_id,
                    client_secret,
                    refresh_token: Some(refresh_token),
                    ..
                },
            ..
        }) = current.credential.as_ref()
        else {
            return Err(authentication_required(authority, current.auth_mode));
        };
        let config = current
            .definition
            .authentication
            .oauth2()
            .ok_or(CapabilityError::Failed)?;
        let token = self
            .inner
            .http
            .exchange_oauth_token(AdapterOAuthTokenRequest {
                token_endpoint: url::Url::parse(&config.token_endpoint)
                    .map_err(|_| CapabilityError::Failed)?,
                client_authentication: config.client_authentication,
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                grant: AdapterOAuthTokenGrant::RefreshToken {
                    refresh_token: refresh_token.clone(),
                },
                expected_scopes: current.definition.authentication.scopes().to_vec(),
                now_epoch_seconds,
            })
            .await
            .map_err(|error| match error {
                AdapterOAuthTokenError::Rejected => {
                    authentication_required(authority, current.auth_mode)
                }
                AdapterOAuthTokenError::Unavailable => CapabilityError::Unavailable,
                AdapterOAuthTokenError::InvalidRequest
                | AdapterOAuthTokenError::InvalidResponse => CapabilityError::Failed,
            })?;
        let generation_id =
            crate::private_fs::random_hex(16).map_err(|_| CapabilityError::Unavailable)?;
        let credential = AdapterCredentialGenerationV2 {
            schema_version: 2,
            generation_id: generation_id.clone(),
            material: AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                callback_mode: *callback_mode,
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                access_token: token.access_token,
                refresh_token: token.refresh_token.or_else(|| Some(refresh_token.clone())),
                expires_at_epoch_seconds: token.expires_at_epoch_seconds,
            },
        };
        let mut replacement = current.connection.clone();
        replacement.revisions.connection = replacement
            .revisions
            .connection
            .checked_add(1)
            .ok_or(CapabilityError::Unavailable)?;
        replacement.revisions.credential = replacement
            .revisions
            .credential
            .checked_add(1)
            .ok_or(CapabilityError::Unavailable)?;
        replacement.credential_generation = Some(generation_id);
        self.inner
            .connections
            .refresh_oauth_credential(
                &current.connection,
                &replacement,
                &credential,
                &current.definition,
            )
            .map_err(|_| CapabilityError::Unavailable)?;
        Ok(())
    }

    fn oauth_bearer(
        &self,
        current: &CurrentPlan,
        authority: &AdapterOperationAuthorityV1,
    ) -> Result<AdapterBearerCredential, CapabilityError> {
        let serving_mode = self
            .inner
            .oauth_callback_mode
            .lock()
            .ok()
            .and_then(|mode| *mode);
        current
            .credential
            .as_ref()
            .and_then(|credential| bearer_credential(credential, serving_mode))
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

fn remote_failure(status: u16, payload: Option<&serde_json::Value>) -> CapabilityOutput {
    let mut failure = json!({"error": "remote_request_failed", "status": status});
    if let Some(payload) =
        payload.and_then(|payload| RedactingPayloadSanitizer.persist_output(payload))
    {
        failure["response"] = payload;
    }
    if serde_json::to_vec(&failure).map_or(true, |bytes| bytes.len() > 4 * 1024) {
        failure = json!({
            "error": "remote_request_failed",
            "status": status,
            "response_omitted": "too_large"
        });
    }
    CapabilityOutput::failed(failure)
}

struct CurrentPlan {
    definition: CompiledAdapterDefinition,
    connection: AdapterConnectionV3,
    operation: CompiledOperation,
    auth_mode: AuthenticationMode,
    credential: Option<AdapterCredentialGenerationV2>,
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
    descriptor: &AdapterConnectionV3,
    operation: &CompiledOperation,
) -> bool {
    canonical_name(
        &definition.adapter_id,
        &descriptor.connection_slug,
        &operation.operation_id,
    )
    .is_ok_and(|name| name.as_str() == authority.canonical_name)
        && descriptor.connection_id == authority.connection_id
        && descriptor.connection_slug == authority.connection_slug
        && descriptor.account_id == authority.account_id
        && descriptor.account_kind == authority.account_kind
        && descriptor.semantic_digest == authority.semantic_digest
        && descriptor.revisions.grant == authority.grant_revision
        && descriptor.revisions.policy == authority.policy_revision
        && descriptor
            .policy
            .is_some_and(|policy| policy.revision == authority.policy_revision)
        && (descriptor.revisions.connection == authority.connection_revision
            && descriptor.revisions.credential == authority.credential_revision
            && descriptor.credential_generation == authority.credential_generation
            || authority_matches_refresh_drift(authority, descriptor))
        && descriptor
            .allowed_operations
            .contains(&authority.operation_id)
        && operation.operation_digest.as_str() == authority.operation_digest
        && operation.token.as_str() == authority.definition_token
}

fn authority_matches_refresh_drift(
    authority: &AdapterOperationAuthorityV1,
    descriptor: &AdapterConnectionV3,
) -> bool {
    let Some(connection_delta) = descriptor
        .revisions
        .connection
        .checked_sub(authority.connection_revision)
    else {
        return false;
    };
    let Some(credential_delta) = descriptor
        .revisions
        .credential
        .checked_sub(authority.credential_revision)
    else {
        return false;
    };
    descriptor.status == AdapterConnectionStatus::Active
        && connection_delta > 0
        && connection_delta == credential_delta
        && descriptor.credential_generation.is_some()
        && authority.credential_generation.is_some()
        && descriptor.credential_generation != authority.credential_generation
}

fn credential_needs_refresh(
    credential: Option<&AdapterCredentialGenerationV2>,
    now_epoch_seconds: u64,
) -> bool {
    let Some(AdapterCredentialGenerationV2 {
        material:
            AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
                expires_at_epoch_seconds: Some(expires_at),
                ..
            },
        ..
    }) = credential
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
    })
}

fn bearer_credential(
    credential: &AdapterCredentialGenerationV2,
    serving_mode: Option<crate::Oauth2CallbackMode>,
) -> Option<AdapterBearerCredential> {
    let AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
        callback_mode,
        access_token,
        ..
    } = &credential.material
    else {
        return None;
    };
    (serving_mode == Some(*callback_mode))
        .then(|| AdapterBearerCredential::new(access_token.clone()))
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
    CapabilityError::AuthenticationRequired {
        challenge: CapabilityAuthenticationChallenge::new(
            challenge_kind,
            CapabilityAuthenticationAuthorityKind::AdapterConnection,
            authority.connection_id.clone(),
            authority.destination_revision(),
        )
        .expect("validated adapter authority is bounded"),
    }
}

#[cfg(test)]
mod tests;
