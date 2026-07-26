//! One route-aware projection for capability results.

use noema_capabilities::{
    CapabilityBinding, CapabilityError, CapabilityId, CapabilityModelPayloadPolicy,
    CapabilityModelRoutePolicy, CapabilityProviderRetentionPolicy, DataFlowClass,
    PersistedCapabilityPayload,
};
use noema_providers::{
    ProviderAccountStatus, ProviderRouteLease, ProviderSelectionSnapshot,
    capabilities_for_provider_account,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::turn::SuccessfulProviderTurn;

/// Exact provider behavior relevant to delivering a capability result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CapabilityResultRoute {
    selection: ProviderSelectionSnapshot,
    provider_generation: u64,
    data_flow: DataFlowClass,
    response_storage: bool,
    previous_response_state: bool,
    prompt_cache: bool,
}

impl CapabilityResultRoute {
    fn for_turn(turn: &SuccessfulProviderTurn) -> Result<Self, CapabilityError> {
        Self::for_route(&turn.provider_route)
    }

    pub(super) fn for_route(route: &ProviderRouteLease) -> Result<Self, CapabilityError> {
        let mut selection = route.selection().clone();
        // Selection provenance explains how the route was chosen, but it does
        // not change where bytes flow. Keep it out of the semantic fence so a
        // rediscovery through another equivalent source does not invalidate an
        // otherwise identical route.
        selection.selection_source = None;
        let data_flow = capabilities_for_provider_account(
            &selection.provider_kind,
            &selection.provider_account_id,
            ProviderAccountStatus::Authenticated,
        )
        .into_iter()
        .find(|capability| capability.capability_id == CapabilityId::ModelGenerate)
        .map(|capability| capability.data_flow_class)
        .ok_or(CapabilityError::Denied)?;
        let model = selection.model_profile.as_deref();
        let operations = route.operations();
        let continuation = operations.response_continuation(model);
        let tool_capabilities = operations.tool_capabilities(model);
        let prompt_cache = tool_capabilities.prompt_cache_retention
            || tool_capabilities.prompt_cache_key
            || tool_capabilities.prompt_cache_options
            || tool_capabilities.prompt_cache_breakpoints;
        Ok(Self {
            selection,
            provider_generation: route.generation(),
            data_flow,
            response_storage: continuation.store_response(),
            previous_response_state: continuation.supports_previous_response_id(),
            prompt_cache,
        })
    }

    pub(super) fn matches_fence(route: &ProviderRouteLease, fence: &Value) -> bool {
        Self::for_route(route).is_ok_and(|current| current.matches_serialized_fence(fence))
    }

    pub(super) fn digest_for_route(route: &ProviderRouteLease) -> Result<String, CapabilityError> {
        Self::for_route(route).and_then(|current| current.digest())
    }

    pub(super) fn matches_digest(route: &ProviderRouteLease, expected: &str) -> bool {
        Self::digest_for_route(route).is_ok_and(|digest| digest == expected)
    }

    fn digest(&self) -> Result<String, CapabilityError> {
        let encoded = serde_json::to_vec(self).map_err(|_| CapabilityError::Denied)?;
        Ok(ring::digest::digest(&ring::digest::SHA256, &encoded)
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn matches_serialized_fence(&self, fence: &Value) -> bool {
        let Ok(expected) = serde_json::from_value::<Self>(fence.clone()) else {
            return false;
        };
        self == &expected
    }

    #[cfg(test)]
    fn synthetic(
        data_flow: DataFlowClass,
        response_storage: bool,
        previous_response_state: bool,
        prompt_cache: bool,
    ) -> Self {
        Self {
            selection: ProviderSelectionSnapshot::explicit(
                if data_flow == DataFlowClass::LocalInference {
                    "local_models"
                } else {
                    "openai"
                },
                "provider_account:test:synthetic",
                "synthetic",
                None,
                Some("test".to_string()),
            ),
            provider_generation: 1,
            data_flow,
            response_storage,
            previous_response_state,
            prompt_cache,
        }
    }
}

/// Admitted result projection fixed before capability invocation.
pub(super) struct CapabilityResultProjection<'a> {
    binding: &'a CapabilityBinding,
    route: CapabilityResultRoute,
}

/// Result views after the raw invoker payload has crossed the projection once.
pub(super) struct ProjectedCapabilityResult {
    pub(super) execution_payload: Value,
    pub(super) model_payload: Value,
    pub(super) persisted: PersistedCapabilityPayload,
    pub(super) continue_model: bool,
}

impl<'a> CapabilityResultProjection<'a> {
    pub(super) fn for_turn(
        binding: &'a CapabilityBinding,
        turn: &SuccessfulProviderTurn,
    ) -> Result<Self, CapabilityError> {
        Self::admit(binding, CapabilityResultRoute::for_turn(turn)?)
    }

    fn admit(
        binding: &'a CapabilityBinding,
        route: CapabilityResultRoute,
    ) -> Result<Self, CapabilityError> {
        let policy = binding.result_policy();
        if policy.model_route == CapabilityModelRoutePolicy::LocalOnly
            && route.data_flow != DataFlowClass::LocalInference
        {
            return Err(CapabilityError::Denied);
        }
        if policy.provider_retention == CapabilityProviderRetentionPolicy::Deny
            && (route.response_storage || route.previous_response_state || route.prompt_cache)
        {
            return Err(CapabilityError::Denied);
        }
        Ok(Self { binding, route })
    }

    pub(super) fn project(
        &self,
        arguments: &Value,
        success: bool,
        raw_payload: Value,
    ) -> ProjectedCapabilityResult {
        let policy = self.binding.result_policy();
        let persisted = PersistedCapabilityPayload {
            arguments: self.binding.persist_arguments(arguments),
            output: self.binding.persist_output(&raw_payload),
        };
        let execution_payload = raw_payload.clone();
        let (model_payload, continue_model) = match policy.model_payload {
            CapabilityModelPayloadPolicy::Full => (raw_payload, true),
            CapabilityModelPayloadPolicy::MetadataOnly => {
                (json!({"result": "metadata_only", "success": success}), true)
            }
            CapabilityModelPayloadPolicy::Omit => (json!({"result": "omitted_by_policy"}), false),
        };
        ProjectedCapabilityResult {
            execution_payload,
            model_payload,
            persisted,
            continue_model,
        }
    }

    pub(super) fn route_fence(&self) -> Value {
        serde_json::to_value(&self.route).expect("result route is serializable")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use noema_capabilities::{
        CapabilityAccess, CapabilityModelPayloadPolicy, CapabilityModelRoutePolicy,
        CapabilityProviderRetentionPolicy, CapabilityResultPolicy, CapabilityScope,
        CapabilityTarget, InvokerKey, OperationToken, RedactingPayloadSanitizer, ToolSpec,
    };

    use super::*;

    fn binding(policy: CapabilityResultPolicy) -> CapabilityBinding {
        CapabilityBinding::new(
            ToolSpec::new(
                "fixture.private",
                "Private fixture.",
                json!({"type":"object"}),
            )
            .expect("spec"),
            CapabilityTarget::new(InvokerKey::new("fixture"), OperationToken::new("one")),
            CapabilityAccess {
                effect: noema_capabilities::CapabilityEffect::ReadOnly,
                scope: CapabilityScope::Global,
            },
            Arc::new(RedactingPayloadSanitizer),
        )
        .with_result_policy(policy)
    }

    #[test]
    fn local_only_result_never_admits_a_remote_model_route() {
        let binding = binding(CapabilityResultPolicy {
            model_route: CapabilityModelRoutePolicy::LocalOnly,
            ..CapabilityResultPolicy::default()
        });
        assert!(
            CapabilityResultProjection::admit(
                &binding,
                CapabilityResultRoute::synthetic(
                    DataFlowClass::ModelProviderPrompt,
                    false,
                    false,
                    false,
                ),
            )
            .is_err()
        );
        assert!(
            CapabilityResultProjection::admit(
                &binding,
                CapabilityResultRoute::synthetic(
                    DataFlowClass::LocalInference,
                    false,
                    false,
                    false,
                ),
            )
            .is_ok()
        );
    }

    #[test]
    fn retention_denial_covers_storage_previous_response_and_cache() {
        let binding = binding(CapabilityResultPolicy {
            provider_retention: CapabilityProviderRetentionPolicy::Deny,
            ..CapabilityResultPolicy::default()
        });
        for route in [
            CapabilityResultRoute::synthetic(
                DataFlowClass::ModelProviderPrompt,
                true,
                false,
                false,
            ),
            CapabilityResultRoute::synthetic(
                DataFlowClass::ModelProviderPrompt,
                false,
                true,
                false,
            ),
            CapabilityResultRoute::synthetic(
                DataFlowClass::ModelProviderPrompt,
                false,
                false,
                true,
            ),
        ] {
            assert!(CapabilityResultProjection::admit(&binding, route).is_err());
        }
    }

    #[test]
    fn metadata_projection_separates_model_and_persisted_payloads() {
        let binding = binding(CapabilityResultPolicy {
            model_payload: CapabilityModelPayloadPolicy::MetadataOnly,
            ..CapabilityResultPolicy::default()
        });
        let projection = CapabilityResultProjection::admit(
            &binding,
            CapabilityResultRoute::synthetic(
                DataFlowClass::ModelProviderPrompt,
                false,
                false,
                false,
            ),
        )
        .expect("admitted");
        let projected = projection.project(
            &json!({"api_key":"private", "query":"safe"}),
            true,
            json!({"secret":"private-marker", "count":2}),
        );
        assert_eq!(
            projected.model_payload,
            json!({"result":"metadata_only", "success":true})
        );
        assert_eq!(
            projected.persisted.arguments,
            Some(json!({"api_key":"[REDACTED]", "query":"safe"}))
        );
        assert_eq!(
            projected.persisted.output,
            Some(json!({"secret":"[REDACTED]", "count":2}))
        );
        assert!(
            !projected
                .model_payload
                .to_string()
                .contains("private-marker")
        );
    }

    #[test]
    fn route_fence_detects_generation_and_retention_changes() {
        let current = CapabilityResultRoute::synthetic(
            DataFlowClass::ModelProviderPrompt,
            false,
            true,
            false,
        );
        let exact = serde_json::to_value(&current).expect("route fence");
        assert!(current.matches_serialized_fence(&exact));

        let mut changed = exact.clone();
        changed["provider_generation"] = json!(2);
        assert!(!current.matches_serialized_fence(&changed));
        changed = exact;
        changed["prompt_cache"] = json!(true);
        assert!(!current.matches_serialized_fence(&changed));
    }
}
