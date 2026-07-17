//! Exact provider selection-to-instance route resolution.

use std::{fmt, future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    ProviderInstanceKey, ProviderInstanceLease, ProviderOperations, ProviderRegistryError,
    ProviderRegistryHandle, ProviderSelectionSnapshot,
};

const MAX_SELECTION_RETRIES: usize = 4;

/// Boxed future returned by object-safe provider routing contracts.
pub type ProviderRouteFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderRouteError>> + Send + 'a>>;

/// Async source for one canonical provider selection snapshot.
pub trait ProviderSelectionLoader: Send + Sync {
    /// Load the current canonical selection.
    fn load_selection(&self) -> ProviderRouteFuture<'_, ProviderSelectionSnapshot>;
}

/// Clonable provider selection loader handle.
pub type ProviderSelectionLoaderHandle = Arc<dyn ProviderSelectionLoader>;

/// Build a selection loader from an owned boxed-future closure.
#[must_use]
pub fn provider_selection_loader<F>(loader: F) -> ProviderSelectionLoaderHandle
where
    F: Fn() -> ProviderRouteFuture<'static, ProviderSelectionSnapshot> + Send + Sync + 'static,
{
    Arc::new(ClosureSelectionLoader(loader))
}

struct ClosureSelectionLoader<F>(F);

impl<F> ProviderSelectionLoader for ClosureSelectionLoader<F>
where
    F: Fn() -> ProviderRouteFuture<'static, ProviderSelectionSnapshot> + Send + Sync + 'static,
{
    fn load_selection(&self) -> ProviderRouteFuture<'_, ProviderSelectionSnapshot> {
        (self.0)()
    }
}

impl<F> fmt::Debug for ClosureSelectionLoader<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSelectionLoader")
            .finish_non_exhaustive()
    }
}

/// Object-safe route resolver bound to one canonical selection source.
pub trait ProviderRouteResolver: Send + Sync {
    /// Load the current selection and lease its exact ready provider instance.
    fn resolve_route(&self) -> ProviderRouteFuture<'_, ProviderRouteLease>;
}

/// Clonable provider route resolver handle.
pub type ProviderRouteResolverHandle = Arc<dyn ProviderRouteResolver>;

/// Strict resolver backed by a canonical snapshot loader and provider registry.
pub struct RegistryProviderRouteResolver {
    loader: ProviderSelectionLoaderHandle,
    registry: ProviderRegistryHandle,
}

impl RegistryProviderRouteResolver {
    /// Bind one canonical selection loader to the shared provider registry.
    #[must_use]
    pub fn new(loader: ProviderSelectionLoaderHandle, registry: ProviderRegistryHandle) -> Self {
        Self { loader, registry }
    }
}

impl fmt::Debug for RegistryProviderRouteResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryProviderRouteResolver")
            .finish_non_exhaustive()
    }
}

impl ProviderRouteResolver for RegistryProviderRouteResolver {
    fn resolve_route(&self) -> ProviderRouteFuture<'_, ProviderRouteLease> {
        Box::pin(async move {
            let mut selection = load_normalized_selection(self.loader.as_ref()).await?;
            for _ in 0..MAX_SELECTION_RETRIES {
                let key = selection
                    .provider_instance_key
                    .as_ref()
                    .ok_or(ProviderRouteError::MissingInstanceKey)?;
                match self.registry.lease(key) {
                    Ok(instance) => {
                        let next = load_normalized_selection(self.loader.as_ref()).await?;
                        if next != selection {
                            drop(instance);
                            selection = next;
                            continue;
                        }
                        return ProviderRouteLease::try_new(next, instance);
                    }
                    Err(error @ ProviderRegistryError::Missing { .. })
                    | Err(error @ ProviderRegistryError::Retiring { .. })
                    | Err(error @ ProviderRegistryError::Unready { .. }) => {
                        let next = load_normalized_selection(self.loader.as_ref()).await?;
                        if next != selection {
                            selection = next;
                            continue;
                        }
                        return Err(route_availability_error(error));
                    }
                    Err(_) => {
                        return Err(ProviderRouteError::Registry {
                            operation: "lease_provider_instance",
                        });
                    }
                }
            }
            Err(ProviderRouteError::SelectionConflict)
        })
    }
}

async fn load_normalized_selection(
    loader: &dyn ProviderSelectionLoader,
) -> Result<ProviderSelectionSnapshot, ProviderRouteError> {
    loader
        .load_selection()
        .await?
        .normalized()
        .map_err(|error| ProviderRouteError::InvalidSelection {
            message: error.to_string(),
        })
}

fn route_availability_error(error: ProviderRegistryError) -> ProviderRouteError {
    match error {
        ProviderRegistryError::Missing { key } => ProviderRouteError::InstanceMissing { key },
        ProviderRegistryError::Unready { key } => ProviderRouteError::InstanceUnready { key },
        ProviderRegistryError::Retiring { key, .. } => {
            ProviderRouteError::RetiringSelectionInvariant { key }
        }
        _ => ProviderRouteError::Registry {
            operation: "lease_provider_instance",
        },
    }
}

/// One repository selection pinned to an exact provider generation.
pub struct ProviderRouteLease {
    selection: ProviderSelectionSnapshot,
    instance: ProviderInstanceLease,
}

impl ProviderRouteLease {
    /// Combine one canonical selection with its exact leased provider instance.
    ///
    /// Resolver implementations own the read-to-lease consistency policy.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderRouteError::MissingInstanceKey`] or
    /// [`ProviderRouteError::InstanceKeyMismatch`] when the selection does not
    /// name the leased instance exactly.
    pub fn try_new(
        selection: ProviderSelectionSnapshot,
        instance: ProviderInstanceLease,
    ) -> Result<Self, ProviderRouteError> {
        let selection =
            selection
                .normalized()
                .map_err(|error| ProviderRouteError::InvalidSelection {
                    message: error.to_string(),
                })?;
        let selected_key = selection
            .provider_instance_key
            .as_ref()
            .ok_or(ProviderRouteError::MissingInstanceKey)?;
        if selected_key != instance.key() {
            return Err(ProviderRouteError::InstanceKeyMismatch {
                selected: selected_key.clone(),
                leased: instance.key().clone(),
            });
        }
        Ok(Self {
            selection,
            instance,
        })
    }
    /// Return the canonical selection used for this route.
    #[must_use]
    pub fn selection(&self) -> &ProviderSelectionSnapshot {
        &self.selection
    }

    /// Return the exact immutable instance key.
    #[must_use]
    pub fn key(&self) -> &ProviderInstanceKey {
        self.instance.key()
    }

    /// Return the exact registry generation retained by this route.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.instance.generation()
    }

    /// Borrow provider operations while this route retains its retirement guard.
    #[must_use]
    pub fn operations(&self) -> &dyn ProviderOperations {
        self.instance.operations()
    }
}

impl fmt::Debug for ProviderRouteLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRouteLease")
            .field("selection", &self.selection)
            .field("instance_key", self.key())
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

/// Provider route selection, availability, or consistency failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProviderRouteError {
    /// The selection loader could not read its canonical repository state.
    #[error("provider selection could not be loaded during {operation}")]
    SelectionLoad {
        /// Stable owner-specific load operation.
        operation: &'static str,
    },
    /// The loaded selection violated provider selection invariants.
    #[error("invalid provider selection: {message}")]
    InvalidSelection {
        /// Sanitized validation message.
        message: String,
    },
    /// Strict registry resolution requires exact durable instance identity.
    #[error("provider selection does not contain an exact instance key")]
    MissingInstanceKey,
    /// A resolver paired a canonical selection with a different instance lease.
    #[error("provider selection key {selected} does not match leased instance {leased}")]
    InstanceKeyMismatch {
        /// Exact key named by the selection.
        selected: ProviderInstanceKey,
        /// Exact key retained by the lease.
        leased: ProviderInstanceKey,
    },
    /// No ready instance exists for an unchanged canonical selection.
    #[error("selected provider instance is missing: {key}")]
    InstanceMissing {
        /// Missing exact instance key.
        key: ProviderInstanceKey,
    },
    /// Construction state knows the selected instance but it is not ready.
    #[error("selected provider instance is not ready: {key}")]
    InstanceUnready {
        /// Unready exact instance key.
        key: ProviderInstanceKey,
    },
    /// Persistence still references an instance whose retirement already began.
    #[error("unchanged provider selection references a retiring instance: {key}")]
    RetiringSelectionInvariant {
        /// Incorrectly retiring exact instance key.
        key: ProviderInstanceKey,
    },
    /// Repeated selection mutation prevented a stable read-to-lease result.
    #[error("provider selection changed continuously while resolving a route")]
    SelectionConflict,
    /// Registry bookkeeping failed without exposing implementation details.
    #[error("provider registry failed during {operation}")]
    Registry {
        /// Stable registry operation.
        operation: &'static str,
    },
}

#[cfg(test)]
mod tests;
