//! Process-local exact-instance routing for the transitional Phase 10B schema.

use std::{fmt, sync::Arc};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use crate::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, ProviderInstanceKey, ProviderRegistryError,
    ProviderRegistryHandle, ProviderRouteError, ProviderRouteLease, ProviderSelectionMode,
    ProviderSelectionSnapshot,
};

const LOCAL_MODELS_PROVIDER_KIND: &str = "local_models";
const UNAVAILABLE_LOCAL_INSTANCE_KEY: &str = "local-model:v1:unavailable";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveLocalModelRoute {
    pub(super) key: ProviderInstanceKey,
    pub(super) model_id: String,
}

/// Shared exact-instance route selected by the local-model manager.
///
/// Phase 10B keeps durable selections on their legacy kind/profile shape. This
/// handle supplies the exact process key only after reading the current
/// manager-owned publication state, so distinct installations never collapse
/// onto the legacy provider-kind key.
#[derive(Clone)]
pub struct LocalModelRouteHandle {
    registry: ProviderRegistryHandle,
    active: Arc<RwLock<Option<ActiveLocalModelRoute>>>,
}

impl LocalModelRouteHandle {
    pub(super) fn new(registry: ProviderRegistryHandle) -> Self {
        Self {
            registry,
            active: Arc::new(RwLock::new(None)),
        }
    }

    /// Acquire a stable active-route view and lease its exact provider process.
    ///
    /// # Errors
    ///
    /// Returns a typed route error when the selection is invalid, no active
    /// local installation is published, or its exact instance is unavailable.
    pub async fn resolve_snapshot(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, ProviderRouteError> {
        self.read().await.resolve_snapshot(selection)
    }

    /// Return the active exact instance key, when one is published.
    #[must_use]
    pub async fn active_key(&self) -> Option<ProviderInstanceKey> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| active.key.clone())
    }

    /// Acquire a stable active-route view before reading its durable selection.
    ///
    /// This transitional guard lets a caller span the canonical selection read
    /// and the exact-instance lease with one shared publication lock.
    #[doc(hidden)]
    pub async fn read(&self) -> LocalModelRouteReadGuard {
        LocalModelRouteReadGuard {
            registry: Arc::clone(&self.registry),
            active: Arc::clone(&self.active).read_owned().await,
        }
    }

    pub(super) async fn begin_publication(&self) -> LocalModelRoutePublicationGuard {
        LocalModelRoutePublicationGuard {
            active: Arc::clone(&self.active).write_owned().await,
        }
    }
}

impl fmt::Debug for LocalModelRouteHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalModelRouteHandle")
            .finish_non_exhaustive()
    }
}

/// Stable local-model publication view used by the transitional route bridge.
#[doc(hidden)]
pub struct LocalModelRouteReadGuard {
    registry: ProviderRegistryHandle,
    active: OwnedRwLockReadGuard<Option<ActiveLocalModelRoute>>,
}

impl LocalModelRouteReadGuard {
    /// Lease the exact local-model instance matching a snapshot read under this guard.
    ///
    /// # Errors
    ///
    /// Returns a typed route error when the selection is invalid, no active
    /// local installation is published, or its exact instance is unavailable.
    pub fn resolve_snapshot(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, ProviderRouteError> {
        let mut selection =
            selection
                .normalized()
                .map_err(|error| ProviderRouteError::InvalidSelection {
                    message: error.to_string(),
                })?;
        if selection.provider_kind != LOCAL_MODELS_PROVIDER_KIND
            || selection.provider_account_id != LOCAL_MODELS_PROVIDER_ACCOUNT_ID
        {
            return Err(ProviderRouteError::InvalidSelection {
                message: "local-model route requires the built-in local provider account"
                    .to_string(),
            });
        }
        let active = self.active.as_ref().ok_or_else(unavailable_route_error)?;
        if selection.selection_mode == ProviderSelectionMode::ExplicitProfile
            && selection.model_profile.as_deref() != Some(active.model_id.as_str())
        {
            return Err(ProviderRouteError::InvalidSelection {
                message: "selected local model is not the active installation".to_string(),
            });
        }
        let instance = self
            .registry
            .lease(&active.key)
            .map_err(route_registry_error)?;
        selection.provider_instance_key = Some(active.key.clone());
        ProviderRouteLease::try_new(selection, instance)
    }
}

pub(super) struct LocalModelRoutePublicationGuard {
    active: OwnedRwLockWriteGuard<Option<ActiveLocalModelRoute>>,
}

impl LocalModelRoutePublicationGuard {
    pub(super) fn publish(&mut self, route: ActiveLocalModelRoute) {
        *self.active = Some(route);
    }

    pub(super) fn clear(&mut self) {
        *self.active = None;
    }
}

fn unavailable_route_error() -> ProviderRouteError {
    ProviderRouteError::InstanceMissing {
        key: ProviderInstanceKey::new(UNAVAILABLE_LOCAL_INSTANCE_KEY)
            .expect("static local-model instance key is valid"),
    }
}

fn route_registry_error(error: ProviderRegistryError) -> ProviderRouteError {
    match error {
        ProviderRegistryError::Missing { key } => ProviderRouteError::InstanceMissing { key },
        ProviderRegistryError::Retiring { key, .. } => {
            ProviderRouteError::RetiringSelectionInvariant { key }
        }
        _ => ProviderRouteError::Registry {
            operation: "lease_local_model_instance",
        },
    }
}
