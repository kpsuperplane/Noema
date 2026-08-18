//! Exact owner-repository selection loaders for provider routing.

use noema_providers::{
    ModelPreferenceSelection, NoemaModelUseCase, ProviderKind, ProviderRouteError,
    ProviderSelectionLoaderHandle, ProviderSelectionSnapshot, provider_selection_loader,
};

use crate::{AuxiliaryModelTask, NoemaStore, StoreError};

impl NoemaStore {
    /// Bind a fresh-per-resolution loader to the canonical global preference.
    #[must_use]
    pub fn default_provider_selection_loader(&self) -> ProviderSelectionLoaderHandle {
        self.selection_loader(SelectionOwner::Default)
    }

    /// Bind a fresh-per-resolution loader to one agent preference.
    #[must_use]
    pub fn agent_provider_selection_loader(
        &self,
        agent_id: impl Into<String>,
    ) -> ProviderSelectionLoaderHandle {
        self.selection_loader(SelectionOwner::Agent(agent_id.into()))
    }

    /// Bind a fresh-per-resolution loader to one auxiliary preference.
    #[must_use]
    pub fn auxiliary_provider_selection_loader(
        &self,
        task: AuxiliaryModelTask,
    ) -> ProviderSelectionLoaderHandle {
        self.selection_loader(SelectionOwner::Auxiliary(task))
    }

    fn selection_loader(&self, owner: SelectionOwner) -> ProviderSelectionLoaderHandle {
        let store = self.clone();
        provider_selection_loader(move || {
            let store = store.clone();
            let owner = owner.clone();
            Box::pin(async move { store.provider_selection(owner).await.map_err(load_error) })
        })
    }

    async fn provider_selection(
        &self,
        owner: SelectionOwner,
    ) -> Result<ProviderSelectionSnapshot, StoreError> {
        match owner {
            SelectionOwner::Default => {
                let record = self
                    .get_default_model_preference()
                    .await?
                    .ok_or_else(missing_initialized_selection)?;
                exact_selection(
                    record.provider_kind,
                    record.provider_account_id,
                    record.provider_instance_key,
                    record.selection,
                    record.fast_mode,
                    NoemaModelUseCase::Primary,
                    "default_model_preference".to_string(),
                )
            }
            SelectionOwner::Agent(agent_id) => {
                let record = self
                    .get_agent_runtime_preference(&agent_id)
                    .await?
                    .ok_or_else(missing_initialized_selection)?;
                exact_selection(
                    record.provider_kind,
                    record.provider_account_id,
                    record.provider_instance_key,
                    record.selection,
                    record.fast_mode,
                    agent_use_case(&agent_id)?,
                    format!("agent_runtime_preference:{agent_id}"),
                )
            }
            SelectionOwner::Auxiliary(task) => {
                let record = self
                    .get_auxiliary_model_preference(task)
                    .await?
                    .ok_or_else(missing_initialized_selection)?;
                exact_selection(
                    record.provider_kind,
                    record.provider_account_id,
                    record.provider_instance_key,
                    record.selection,
                    record.fast_mode,
                    auxiliary_use_case(task),
                    format!("auxiliary_model_preference:{task}"),
                )
            }
        }
    }

    /// Load the canonical global exact provider snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when initialization has not persisted a valid
    /// exact default selection.
    pub async fn default_provider_selection(
        &self,
    ) -> Result<ProviderSelectionSnapshot, StoreError> {
        self.provider_selection(SelectionOwner::Default).await
    }

    /// Load one agent's current effective provider selection.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the preference is missing or cannot resolve.
    pub async fn effective_agent_provider_selection(
        &self,
        agent_id: &str,
    ) -> Result<ProviderSelectionSnapshot, StoreError> {
        self.provider_selection(SelectionOwner::Agent(agent_id.to_string()))
            .await
    }
}

#[derive(Clone)]
enum SelectionOwner {
    Default,
    Agent(String),
    Auxiliary(AuxiliaryModelTask),
}

fn exact_selection(
    provider_kind: String,
    provider_account_id: String,
    provider_instance_key: noema_providers::ProviderInstanceKey,
    preference: ModelPreferenceSelection,
    fast_mode: bool,
    use_case: NoemaModelUseCase,
    source: String,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let provider = provider_kind
        .parse::<ProviderKind>()
        .map_err(|_| StoreError::InvalidEnum {
            kind: "model_provider",
            value: provider_kind.clone(),
        })?;
    let (model_profile, reasoning_effort) =
        preference
            .resolve(provider, use_case)
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "provider has no Noema recommendation for this use case".to_string(),
            })?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort,
        Some(source),
    );
    selection.provider_instance_key = Some(provider_instance_key);
    selection.fast_mode = fast_mode;
    selection
        .normalized_for_persistence()
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })
}

fn agent_use_case(agent_id: &str) -> Result<NoemaModelUseCase, StoreError> {
    match agent_id {
        "agent:primary" => Ok(NoemaModelUseCase::Primary),
        "agent:task-executor" => Ok(NoemaModelUseCase::TaskMedium),
        "agent:task-reviewer" => Ok(NoemaModelUseCase::TaskReviewer),
        _ => Err(StoreError::InvariantViolation {
            message: format!("agent has no model recommendation use case: {agent_id}"),
        }),
    }
}

const fn auxiliary_use_case(task: AuxiliaryModelTask) -> NoemaModelUseCase {
    match task {
        AuxiliaryModelTask::WebFetchSummarizer => NoemaModelUseCase::WebFetchSummarizer,
        AuxiliaryModelTask::ToolProgressAudit => NoemaModelUseCase::ToolProgressAudit,
        AuxiliaryModelTask::ActionReviewer => NoemaModelUseCase::ActionReviewer,
        AuxiliaryModelTask::MemoryConsolidation => NoemaModelUseCase::MemoryConsolidation,
    }
}

fn missing_initialized_selection() -> StoreError {
    StoreError::InvariantViolation {
        message: "initialized provider selection is missing".to_string(),
    }
}

fn load_error(_: StoreError) -> ProviderRouteError {
    ProviderRouteError::SelectionLoad {
        operation: "load_sqlite_provider_selection",
    }
}
