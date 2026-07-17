//! Exact owner-repository selection loaders for provider routing.

use noema_providers::{
    ProviderRouteError, ProviderSelectionLoaderHandle, ProviderSelectionSnapshot, ReasoningEffort,
    provider_selection_loader,
};

use crate::{NoemaStore, StoreError};

impl NoemaStore {
    /// Bind a fresh-per-resolution loader to the canonical global preference.
    #[must_use]
    pub fn default_provider_selection_loader(&self) -> ProviderSelectionLoaderHandle {
        let store = self.clone();
        provider_selection_loader(move || {
            let store = store.clone();
            Box::pin(async move { store.default_provider_selection().await.map_err(load_error) })
        })
    }

    /// Bind a fresh-per-resolution loader to one agent preference.
    #[must_use]
    pub fn agent_provider_selection_loader(
        &self,
        agent_id: impl Into<String>,
    ) -> ProviderSelectionLoaderHandle {
        let store = self.clone();
        let agent_id = agent_id.into();
        provider_selection_loader(move || {
            let store = store.clone();
            let agent_id = agent_id.clone();
            Box::pin(async move {
                store
                    .agent_provider_selection(&agent_id)
                    .await
                    .map_err(load_error)
            })
        })
    }

    /// Bind a fresh-per-resolution loader to one auxiliary preference.
    #[must_use]
    pub fn auxiliary_provider_selection_loader(
        &self,
        task_id: impl Into<String>,
    ) -> ProviderSelectionLoaderHandle {
        let store = self.clone();
        let task_id = task_id.into();
        provider_selection_loader(move || {
            let store = store.clone();
            let task_id = task_id.clone();
            Box::pin(async move {
                store
                    .auxiliary_provider_selection(&task_id)
                    .await
                    .map_err(load_error)
            })
        })
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
        let record = self
            .get_default_model_preference()
            .await?
            .ok_or_else(missing_initialized_selection)?;
        let reasoning_effort = parse_reasoning(record.reasoning_effort.as_deref())?;
        let mut selection = ProviderSelectionSnapshot::explicit(
            record.provider_kind,
            record.provider_account_id,
            record.model_profile,
            reasoning_effort,
            Some("default_model_preference".to_string()),
        );
        selection.provider_instance_key = Some(record.provider_instance_key);
        selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })
    }

    /// Load one agent's canonical exact provider snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent preference is missing or malformed.
    pub async fn agent_provider_selection(
        &self,
        agent_id: &str,
    ) -> Result<ProviderSelectionSnapshot, StoreError> {
        let record = self
            .get_agent_runtime_preference(agent_id)
            .await?
            .ok_or_else(missing_initialized_selection)?;
        let mut selection = ProviderSelectionSnapshot::explicit(
            record.provider_kind,
            record.provider_account_id,
            record.model_profile,
            record.reasoning_effort,
            Some(format!("agent_runtime_preference:{agent_id}")),
        );
        selection.provider_instance_key = Some(record.provider_instance_key);
        selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })
    }

    /// Load one auxiliary task's canonical exact provider snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the auxiliary preference is missing or malformed.
    pub async fn auxiliary_provider_selection(
        &self,
        task_id: &str,
    ) -> Result<ProviderSelectionSnapshot, StoreError> {
        let record = self
            .get_auxiliary_model_preference(task_id)
            .await?
            .ok_or_else(missing_initialized_selection)?;
        let mut selection = ProviderSelectionSnapshot::explicit(
            record.provider_kind,
            record.provider_account_id,
            record.model_profile,
            record.reasoning_effort,
            Some(format!("auxiliary_model_preference:{task_id}")),
        );
        selection.provider_instance_key = Some(record.provider_instance_key);
        selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })
    }
}

fn parse_reasoning(value: Option<&str>) -> Result<Option<ReasoningEffort>, StoreError> {
    value
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| StoreError::InvalidEnum {
                kind: "reasoning_effort",
                value: value.to_string(),
            })
        })
        .transpose()
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
