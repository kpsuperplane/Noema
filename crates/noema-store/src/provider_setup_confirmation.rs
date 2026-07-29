//! Atomic publication of first-run model assignments.

use std::{collections::HashSet, str::FromStr};

use noema_providers::{ProviderKind, ProviderReadySelection, ProviderSelectionSnapshot};
use rusqlite::Transaction;

use crate::{
    NoemaStore, StoreError,
    provider_selection_initialization::ensure_builtin_agents,
    provider_selections::{
        CanonicalPreferenceOwner, PreferenceOrigin, SelectionEligibility,
        validate_provider_selection_tx, validate_ready_selection_proof, write_preference_tx,
        write_task_pool_preference_tx,
    },
};

/// Human-meaningful model assignment configured during first-run setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderSetupRole {
    /// Foreground Noema conversations.
    Noema,
    /// Simple task execution.
    SimpleTasks,
    /// Medium task execution.
    MediumTasks,
    /// Difficult task execution.
    DifficultTasks,
    /// Task review.
    TaskReviewer,
    /// Web-page summarization.
    WebFetchSummarizer,
    /// Tool-continuation progress checks.
    ToolProgressAudit,
    /// Governed-action review.
    ActionReviewer,
    /// Native-memory consolidation.
    MemoryConsolidation,
}

/// One ready first-run model assignment and whether it differs from Noema's proposal.
pub struct ReadyProviderSetupSelection {
    /// Semantic workload being configured.
    pub role: ProviderSetupRole,
    /// Exact canonical selection.
    pub selection: ProviderSelectionSnapshot,
    /// Registry proof held through the writer transaction.
    pub ready: ProviderReadySelection,
    /// Whether the human changed Noema's proposed value.
    pub is_override: bool,
}

impl NoemaStore {
    /// Atomically publish a complete first-run model configuration.
    ///
    /// A concurrent winner makes later calls idempotent and never allows them
    /// to replace the already committed configuration.
    ///
    /// # Errors
    ///
    /// Returns a typed store error when the assignments are incomplete,
    /// inconsistent, unavailable, or when any durable write fails.
    pub async fn confirm_provider_setup_selections(
        &self,
        assignments: &[ReadyProviderSetupSelection],
    ) -> Result<bool, StoreError> {
        let normalized = normalize_setup_assignments(assignments)?;
        self.with_immediate_transaction_retry(|transaction| {
            let initialized = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_runtime_preferences WHERE agent_id = 'agent:primary')",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if initialized {
                return Ok(false);
            }
            let partial = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM default_model_preference UNION ALL SELECT 1 FROM agent_runtime_preferences UNION ALL SELECT 1 FROM task_model_pool_entries UNION ALL SELECT 1 FROM auxiliary_model_preferences)",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if partial {
                return Err(StoreError::InvariantViolation {
                    message: "first-run model selections are partially initialized".to_string(),
                });
            }
            for (assignment, selection) in assignments.iter().zip(&normalized) {
                validate_ready_selection_proof(selection, &assignment.ready)?;
                validate_provider_selection_tx(
                    transaction,
                    selection,
                    SelectionEligibility::Canonical,
                )?;
            }
            ensure_builtin_agents(transaction)?;
            for (assignment, selection) in assignments.iter().zip(&normalized) {
                write_setup_assignment(transaction, assignment, selection)?;
            }
            Ok(true)
        })
        .await
    }
}

fn normalize_setup_assignments(
    assignments: &[ReadyProviderSetupSelection],
) -> Result<Vec<ProviderSelectionSnapshot>, StoreError> {
    let first = assignments
        .first()
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "first-run model selections are empty".to_string(),
        })?;
    let provider_kind =
        ProviderKind::from_str(first.selection.provider_kind.trim()).map_err(|_| {
            StoreError::InvariantViolation {
                message: "first-run model provider is unsupported".to_string(),
            }
        })?;
    let first_selection = first
        .selection
        .normalized_for_persistence()
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    let expected: HashSet<_> = [
        ProviderSetupRole::Noema,
        ProviderSetupRole::SimpleTasks,
        ProviderSetupRole::MediumTasks,
        ProviderSetupRole::DifficultTasks,
        ProviderSetupRole::TaskReviewer,
        ProviderSetupRole::WebFetchSummarizer,
        ProviderSetupRole::ToolProgressAudit,
        ProviderSetupRole::MemoryConsolidation,
    ]
    .into_iter()
    .chain(
        (provider_kind != ProviderKind::LocalModels).then_some(ProviderSetupRole::ActionReviewer),
    )
    .collect();
    let actual: HashSet<_> = assignments
        .iter()
        .map(|assignment| assignment.role)
        .collect();
    if actual != expected || actual.len() != assignments.len() {
        return Err(StoreError::InvariantViolation {
            message: "first-run model selections are incomplete or duplicated".to_string(),
        });
    }
    let mut normalized = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        let selection = assignment
            .selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
        if selection.provider_kind != first_selection.provider_kind
            || selection.provider_account_id != first_selection.provider_account_id
            || selection.provider_instance_key != first_selection.provider_instance_key
        {
            return Err(StoreError::InvariantViolation {
                message: "first-run model selections must use one provider account".to_string(),
            });
        }
        normalized.push(selection);
    }
    Ok(normalized)
}

fn write_setup_assignment(
    transaction: &Transaction<'_>,
    assignment: &ReadyProviderSetupSelection,
    selection: &ProviderSelectionSnapshot,
) -> Result<(), StoreError> {
    let origin = if assignment.is_override {
        PreferenceOrigin::Override
    } else {
        PreferenceOrigin::Default
    };
    match assignment.role {
        ProviderSetupRole::Noema => {
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Default,
                selection,
                origin,
                false,
            )?;
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Agent("agent:primary"),
                selection,
                origin,
                false,
            )
        }
        ProviderSetupRole::SimpleTasks => write_task_pool_preference_tx(
            transaction,
            "task_pool:setting:simple",
            "simple",
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::MediumTasks => {
            write_task_pool_preference_tx(
                transaction,
                "task_pool:setting:medium",
                "medium",
                selection,
                origin,
                false,
            )?;
            write_preference_tx(
                transaction,
                CanonicalPreferenceOwner::Agent("agent:task-executor"),
                selection,
                origin,
                false,
            )
        }
        ProviderSetupRole::DifficultTasks => write_task_pool_preference_tx(
            transaction,
            "task_pool:setting:difficult",
            "difficult",
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::TaskReviewer => write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Agent("agent:task-reviewer"),
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::WebFetchSummarizer => write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary("web_fetch_summarizer"),
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::ToolProgressAudit => write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary("tool_progress_audit"),
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::ActionReviewer => write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary("action_reviewer"),
            selection,
            origin,
            false,
        ),
        ProviderSetupRole::MemoryConsolidation => write_preference_tx(
            transaction,
            CanonicalPreferenceOwner::Auxiliary("memory_extraction"),
            selection,
            origin,
            false,
        ),
    }
}
