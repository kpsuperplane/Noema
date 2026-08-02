//! Adapter invocation with live filesystem authority revalidation.

use crate::{
    AdapterCapabilityService, AdapterConnectionStatus, AdapterConnectionV3,
    AdapterCredentialGenerationV2, AdapterCredentialMaterial, AuthenticationMode,
    CompiledAdapterDefinition, CompiledOperation,
    catalog::{AdapterOperationAuthorityV1, canonical_name, effective_behavior},
    network::{AdapterBearerCredential, AdapterHttpError},
    request::{apply_credential_auth, encode_request},
};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeKind, CapabilityError, CapabilityExecutionDecision,
    CapabilityFuture, CapabilityInvocation, CapabilityInvoker, CapabilityOutput, PayloadSanitizer,
    RedactingPayloadSanitizer, resolve_capability_execution_decision,
};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

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
            &invocation.arguments,
        )
        .map_err(|_| CapabilityError::InvalidArguments)?;

        let lock = self
            .connection_lock(&authority.connection_id)
            .map_err(|_| CapabilityError::Unavailable)?;
        let _guard = lock.read().await;
        let current = {
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
        if credential_expired(current.credential.as_ref()) {
            return Err(authentication_required(&authority, current.auth_mode));
        }
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
                let serving_mode = self
                    .inner
                    .oauth_callback_mode
                    .lock()
                    .ok()
                    .and_then(|mode| *mode);
                Some(
                    current
                        .credential
                        .as_ref()
                        .and_then(|credential| bearer_credential(credential, serving_mode))
                        .ok_or_else(|| authentication_required(&authority, current.auth_mode))?,
                )
            }
        };

        let behavior = effective_behavior(&current.connection, &current.operation)
            .map_err(|_| CapabilityError::UnknownOperation)?;
        let retry = if behavior.idempotent {
            current.operation.retry
        } else {
            crate::RetryPolicy::Never
        };
        let response = match self
            .inner
            .http
            .execute(current.operation.method, retry, request, bearer)
            .await
        {
            Ok(response) => response,
            Err(AdapterHttpError::Unavailable) => {
                if !behavior.read_only {
                    return Err(CapabilityError::OutcomeUncertain);
                } else {
                    return Err(CapabilityError::Unavailable);
                }
            }
            Err(AdapterHttpError::OutcomeUncertain) => {
                if !behavior.read_only {
                    return Err(CapabilityError::OutcomeUncertain);
                } else {
                    return Err(CapabilityError::Unavailable);
                }
            }
            Err(AdapterHttpError::InvalidResponse) => {
                if !behavior.read_only {
                    return Err(CapabilityError::OutcomeUncertain);
                } else {
                    return Err(CapabilityError::Failed);
                }
            }
        };
        if response.status == 401 {
            return if current.auth_mode == AuthenticationMode::None {
                Err(CapabilityError::Failed)
            } else {
                Err(authentication_required(&authority, current.auth_mode))
            };
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
        let payload = crate::response::success(&response, current.operation.response.as_ref())
            .await
            .map_err(|_| {
                if behavior.read_only {
                    CapabilityError::Failed
                } else {
                    CapabilityError::OutcomeUncertain
                }
            })?;
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
        && descriptor.revisions.connection == authority.connection_revision
        && descriptor.revisions.credential == authority.credential_revision
        && descriptor.revisions.grant == authority.grant_revision
        && descriptor.revisions.policy == authority.policy_revision
        && descriptor
            .policy
            .is_some_and(|policy| policy.revision == authority.policy_revision)
        && descriptor.credential_generation == authority.credential_generation
        && descriptor
            .allowed_operations
            .contains(&authority.operation_id)
        && operation.operation_digest.as_str() == authority.operation_digest
        && operation.token.as_str() == authority.definition_token
}

fn credential_expired(credential: Option<&AdapterCredentialGenerationV2>) -> bool {
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(true, |now| now.as_secs() >= *expires_at)
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
