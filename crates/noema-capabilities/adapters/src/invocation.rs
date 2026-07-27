//! Adapter invocation with live filesystem authority revalidation.

use crate::{
    AdapterCapabilityService, AdapterConnectionStatus, AdapterConnectionV1,
    AdapterCredentialGenerationV1, AdapterCredentialMaterial, AuthenticationMode,
    CompiledAdapterDefinition, CompiledOperation,
    catalog::{AdapterOperationAuthorityV1, canonical_name},
    network::{AdapterBearerCredential, AdapterHttpError, AdapterHttpOutcome},
    request::encode_request,
};
use noema_capabilities::{
    CapabilityAuthenticationAuthorityKind, CapabilityAuthenticationChallenge,
    CapabilityAuthenticationChallengeKind, CapabilityError, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput,
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
        if preliminary.operation.effect.requires_governed_admission() {
            let Some(admission) = invocation.governed_admission.as_ref() else {
                return Err(CapabilityError::Denied);
            };
            if admission.action_id == "observed_url"
                || !admission.matches_arguments(&invocation.arguments)
            {
                return Err(CapabilityError::Denied);
            }
        }
        let request = encode_request(
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
        if current.operation.effect.requires_governed_admission() {
            let Some(admission) = invocation.governed_admission.as_ref() else {
                return Err(CapabilityError::Denied);
            };
            if !current.operation.admission.requires_governed_admission()
                || admission.action_id == "observed_url"
                || !admission.matches_arguments(&invocation.arguments)
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
        let bearer = current.credential.and_then(bearer_credential);
        if current.auth_mode != AuthenticationMode::None && bearer.is_none() {
            return Err(authentication_required(&authority, current.auth_mode));
        }

        match self
            .inner
            .http
            .execute(
                current.operation.method,
                current.operation.retry,
                request,
                bearer,
            )
            .await
        {
            Ok(AdapterHttpOutcome::Success(payload)) => Ok(CapabilityOutput::success(payload)),
            Ok(AdapterHttpOutcome::Rejected(status)) if status >= 500 => {
                if current.operation.effect.requires_governed_admission() {
                    Err(CapabilityError::OutcomeUncertain)
                } else {
                    Ok(CapabilityOutput::failed(json!({
                        "error": "remote_request_failed",
                        "status": status,
                    })))
                }
            }
            Ok(AdapterHttpOutcome::Rejected(status)) => Ok(CapabilityOutput::failed(json!({
                "error": "remote_request_failed",
                "status": status,
            }))),
            Ok(AdapterHttpOutcome::AuthenticationRequired) => {
                if current.auth_mode == AuthenticationMode::None {
                    Err(CapabilityError::Failed)
                } else {
                    Err(authentication_required(&authority, current.auth_mode))
                }
            }
            Ok(AdapterHttpOutcome::Denied) => Err(CapabilityError::Denied),
            Ok(AdapterHttpOutcome::RateLimited) => Err(CapabilityError::Unavailable),
            Err(AdapterHttpError::Unavailable) => {
                if current.operation.effect.requires_governed_admission() {
                    Err(CapabilityError::OutcomeUncertain)
                } else {
                    Err(CapabilityError::Unavailable)
                }
            }
            Err(AdapterHttpError::OutcomeUncertain) => {
                if current.operation.effect.requires_governed_admission() {
                    Err(CapabilityError::OutcomeUncertain)
                } else {
                    Err(CapabilityError::Unavailable)
                }
            }
            Err(AdapterHttpError::InvalidResponse) => {
                if current.operation.effect.requires_governed_admission() {
                    Err(CapabilityError::OutcomeUncertain)
                } else {
                    Err(CapabilityError::Failed)
                }
            }
        }
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
            auth_mode: definition.compiled.authentication.mode,
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
            auth_mode: definition.compiled.authentication.mode,
            definition: definition.compiled,
            connection: descriptor,
            operation,
            credential,
        })
    }
}

struct CurrentPlan {
    definition: CompiledAdapterDefinition,
    connection: AdapterConnectionV1,
    operation: CompiledOperation,
    auth_mode: AuthenticationMode,
    credential: Option<AdapterCredentialGenerationV1>,
}

fn authority_matches(
    authority: &AdapterOperationAuthorityV1,
    definition: &CompiledAdapterDefinition,
    descriptor: &AdapterConnectionV1,
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
        && descriptor.credential_generation == authority.credential_generation
        && descriptor
            .allowed_operations
            .contains(&authority.operation_id)
        && operation.operation_digest.as_str() == authority.operation_digest
        && operation.token.as_str() == authority.definition_token
}

fn credential_expired(credential: Option<&AdapterCredentialGenerationV1>) -> bool {
    let Some(AdapterCredentialGenerationV1 {
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

fn bearer_credential(credential: AdapterCredentialGenerationV1) -> Option<AdapterBearerCredential> {
    let token = match credential.material {
        AdapterCredentialMaterial::StaticBearer { token }
        | AdapterCredentialMaterial::Oauth2AuthorizationCodePkce {
            access_token: token,
            ..
        } => Some(token),
        AdapterCredentialMaterial::Oauth2ClientMetadata { .. } => None,
    };
    token.map(AdapterBearerCredential::new)
}

fn authentication_required(
    authority: &AdapterOperationAuthorityV1,
    auth_mode: AuthenticationMode,
) -> CapabilityError {
    let challenge_kind = match auth_mode {
        AuthenticationMode::StaticBearer => {
            CapabilityAuthenticationChallengeKind::ReplaceCredential
        }
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
