//! Human resolution and exact replay of durable governed actions.

use std::sync::Arc;

use noema_capabilities::{
    CapabilityError, CapabilityInvoker, CapabilityRegistryRouter, CapabilityRouter,
    GovernedCapabilityAdmission,
};
use noema_store::{
    GovernedActionDecision, GovernedActionEffect, GovernedActionRecord, GovernedActionState,
    GovernedExecutionOutcome, WorkCommandService,
};

use super::{action_gateway::capability_failure_code, actor::RuntimeActor};
use crate::daemon::protocol::RuntimeError;

impl RuntimeActor {
    pub(super) async fn resolve_governed_action(
        &self,
        action_id: &str,
        revision: u64,
        human_id: &str,
        decision: GovernedActionDecision,
    ) -> Result<GovernedActionRecord, RuntimeError> {
        let current = self
            .store
            .get_governed_action(action_id, revision)
            .await?
            .ok_or_else(|| RuntimeError::Protocol("governed action is unavailable".to_string()))?;
        if current.owner_human_id != human_id {
            return Err(RuntimeError::Protocol(
                "governed action is unavailable".to_string(),
            ));
        }
        let action = match (current.state, decision) {
            (GovernedActionState::AwaitingApproval, _) => {
                self.store
                    .decide_governed_action(action_id, revision, human_id, decision)
                    .await?
            }
            (GovernedActionState::Executable, GovernedActionDecision::Approve) => current,
            (
                GovernedActionState::Succeeded
                | GovernedActionState::Failed
                | GovernedActionState::OutcomeUncertain
                | GovernedActionState::Declined
                | GovernedActionState::Superseded
                | GovernedActionState::Cancelled,
                _,
            ) => {
                self.resume_action_task(&current, human_id).await?;
                return Ok(current);
            }
            _ => {
                return Err(RuntimeError::Protocol(
                    "governed action decision is stale".to_string(),
                ));
            }
        };
        if decision == GovernedActionDecision::Decline {
            self.resume_action_task(&action, human_id).await?;
            return Ok(action);
        }

        let catalog = self
            .capability_bindings
            .catalog()
            .await
            .map_err(|_| RuntimeError::Protocol("capability catalog is unavailable".to_string()))?
            .snapshot;
        let Some(binding) = catalog.resolve(&action.capability_name) else {
            return self
                .supersede_and_resume(action, human_id, "capability_removed")
                .await;
        };
        let current_effect = match binding.access().effect {
            noema_capabilities::CapabilityEffect::ExternalWrite => {
                Some(GovernedActionEffect::Write)
            }
            noema_capabilities::CapabilityEffect::ExternalExport => {
                Some(GovernedActionEffect::Export)
            }
            noema_capabilities::CapabilityEffect::ExternalWriteAndExport => {
                Some(GovernedActionEffect::WriteAndExport)
            }
            _ => None,
        };
        if binding.target().operation_token().as_str() != action.operation_token
            || current_effect != Some(action.effect)
            || binding.spec().input_schema.as_value() != &action.input_schema
        {
            return self
                .supersede_and_resume(action, human_id, "capability_changed")
                .await;
        }

        let claimed = self
            .store
            .claim_governed_action_execution(action_id, revision)
            .await?;
        let router =
            CapabilityRegistryRouter::new(self.capability_invokers.iter().map(|registration| {
                (
                    registration.key().clone(),
                    registration.invoker().clone() as Arc<dyn CapabilityInvoker + '_>,
                )
            }))
            .expect("runtime capability invoker keys are unique");
        let dispatch = router
            .dispatch_governed(
                catalog,
                claimed.capability_name.clone(),
                claimed.arguments.clone(),
                GovernedCapabilityAdmission {
                    action_id: claimed.action_id.clone(),
                    revision: claimed.revision,
                    arguments_sha256: claimed.arguments_sha256.clone(),
                },
            )
            .await;
        let finished = match dispatch {
            Ok(dispatch) => {
                let outcome = if dispatch.output.success {
                    GovernedExecutionOutcome::Succeeded
                } else {
                    GovernedExecutionOutcome::Failed
                };
                self.store
                    .finish_governed_action_execution(
                        action_id,
                        revision,
                        outcome,
                        Some(&dispatch.output.payload),
                        (!dispatch.output.success).then_some("tool_declared_failure"),
                    )
                    .await?
            }
            Err(failure) => {
                let outcome = if failure.error == CapabilityError::OutcomeUncertain {
                    GovernedExecutionOutcome::OutcomeUncertain
                } else {
                    GovernedExecutionOutcome::Failed
                };
                self.store
                    .finish_governed_action_execution(
                        action_id,
                        revision,
                        outcome,
                        failure.persisted.output.as_ref(),
                        Some(capability_failure_code(&failure.error)),
                    )
                    .await?
            }
        };
        self.resume_action_task(&finished, human_id).await?;
        Ok(finished)
    }

    async fn supersede_and_resume(
        &self,
        action: GovernedActionRecord,
        human_id: &str,
        reason: &str,
    ) -> Result<GovernedActionRecord, RuntimeError> {
        let action = self
            .store
            .supersede_governed_action(&action.action_id, action.revision, reason)
            .await?;
        self.resume_action_task(&action, human_id).await?;
        Ok(action)
    }

    async fn resume_action_task(
        &self,
        action: &GovernedActionRecord,
        human_id: &str,
    ) -> Result<(), RuntimeError> {
        let actor_id = format!("actor:{human_id}");
        WorkCommandService::new(self.store.clone(), self.provider_registry.clone())
            .resume_after_governed_action(&action.action_id, action.revision, &actor_id)
            .await?;
        Ok(())
    }
}
