//! Generation-safe registry for ready provider instances.
//!
//! The registry contains provider instances only. Durable workload selections
//! remain in their owning repositories and are resolved before a lease is
//! requested.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{
        Arc, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use thiserror::Error;

use crate::{
    ProviderHandle, ProviderInstanceKey, ProviderOperations, ProviderSelectionError,
    ProviderSelectionSnapshot,
};

/// Shared provider registry handle used by runtime and auxiliary model callers.
pub type ProviderRegistryHandle = Arc<ProviderRegistry>;

/// One ready provider-instance registry.
pub struct ProviderRegistry {
    inner: Arc<RegistryInner>,
}

impl ProviderRegistry {
    /// Create an empty provider registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RegistryInner::default()),
        }
    }

    /// Register one ready provider instance under an immutable key.
    ///
    /// Registering the same key again atomically installs a new generation.
    /// The replaced generation rejects new leases and remains alive until its
    /// existing leases drain.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderRegistryError::GenerationExhausted`] if the registry
    /// can no longer allocate a distinct generation token.
    pub fn register(
        &self,
        key: ProviderInstanceKey,
        provider: ProviderHandle,
    ) -> Result<ProviderRegistration, ProviderRegistryError> {
        let mut state = lock_state(&self.inner);
        if let Some(generation) = state.blocked.get(&key).copied() {
            return Err(ProviderRegistryError::Retiring { key, generation });
        }
        let generation = state
            .next_generation
            .checked_add(1)
            .ok_or(ProviderRegistryError::GenerationExhausted)?;
        state.next_generation = generation;
        state.unready.remove(&key);

        let entry = Arc::new(RegistryEntry {
            key: key.clone(),
            generation,
            provider,
            registry: Arc::downgrade(&self.inner),
            retiring: AtomicBool::new(false),
            leases: AtomicUsize::new(0),
        });
        state.retired.remove(&key);
        let replaced = state.entries.insert(key.clone(), Arc::clone(&entry));
        if let Some(replaced) = &replaced {
            replaced.retiring.store(true, Ordering::Release);
        }
        drop(state);
        drop(replaced);

        Ok(ProviderRegistration {
            key,
            generation,
            registry: Arc::downgrade(&self.inner),
            entry: Arc::downgrade(&entry),
        })
    }

    /// Lease the current ready generation for an exact immutable key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderRegistryError::Missing`] when no generation is
    /// registered and [`ProviderRegistryError::Retiring`] after retirement has
    /// begun.
    pub fn lease(
        &self,
        key: &ProviderInstanceKey,
    ) -> Result<ProviderInstanceLease, ProviderRegistryError> {
        let state = lock_state(&self.inner);
        if let Some(generation) = state.blocked.get(key).copied() {
            return Err(ProviderRegistryError::Retiring {
                key: key.clone(),
                generation,
            });
        }
        let Some(entry) = state.entries.get(key).cloned() else {
            if state.unready.contains(key) {
                return Err(ProviderRegistryError::Unready { key: key.clone() });
            }
            return match state.retired.get(key).copied() {
                Some(generation) => Err(ProviderRegistryError::Retiring {
                    key: key.clone(),
                    generation,
                }),
                None => Err(ProviderRegistryError::Missing { key: key.clone() }),
            };
        };
        if entry.retiring.load(Ordering::Acquire) {
            return Err(ProviderRegistryError::Retiring {
                key: key.clone(),
                generation: entry.generation,
            });
        }
        entry
            .leases
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |leases| {
                leases.checked_add(1)
            })
            .map_err(|_| ProviderRegistryError::LeaseCountExhausted {
                key: key.clone(),
                generation: entry.generation,
            })?;
        drop(state);

        Ok(ProviderInstanceLease { entry })
    }

    /// Prove that one exact selection names a ready registered instance.
    ///
    /// The returned token retains the registry lease, so a persistence boundary
    /// can borrow it through commit without racing instance retirement.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderReadySelectionError::InvalidSelection`] when the
    /// selection is not an exact durable snapshot, or wraps the registry error
    /// when its exact instance is not ready for a new lease.
    pub fn prove_ready_selection(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderReadySelection, ProviderReadySelectionError> {
        let selection = selection
            .normalized_for_persistence()
            .map_err(ProviderReadySelectionError::InvalidSelection)?;
        let key = selection
            .provider_instance_key
            .as_ref()
            .expect("normalized durable selections contain an instance key");
        let lease = self.lease(key)?;
        Ok(ProviderReadySelection { selection, lease })
    }

    /// Begin retirement of the exact generation represented by a registration.
    ///
    /// The opaque registration token prevents a delayed retirement request from
    /// targeting a replacement generation under the same stable key.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderRegistryError::ForeignRegistration`] when the
    /// registration belongs to a different registry.
    pub fn begin_retirement(
        &self,
        registration: &ProviderRegistration,
    ) -> Result<ProviderRetirementGuard, ProviderRegistryError> {
        let registry = Arc::downgrade(&self.inner);
        if !Weak::ptr_eq(&registry, &registration.registry) {
            return Err(ProviderRegistryError::ForeignRegistration);
        }

        let entry = {
            let _state = lock_state(&self.inner);
            let entry = registration.entry.upgrade();
            if let Some(entry) = &entry {
                entry.retiring.store(true, Ordering::Release);
            }
            entry
        };
        if let Some(entry) = entry {
            finalize_if_drained(&entry);
        }

        Ok(ProviderRetirementGuard {
            key: registration.key.clone(),
            generation: registration.generation,
            registry: registration.registry.clone(),
            entry: registration.entry.clone(),
        })
    }

    /// Permanently block an exact key after its durable retirement claim commits.
    ///
    /// When a ready generation exists, this also rejects new leases and returns
    /// a guard that can be used to await its existing leases. A blocked key can
    /// never be registered again in this registry.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderRegistryError::GenerationExhausted`] if an absent key
    /// cannot be assigned a diagnostic retirement generation.
    pub fn block_for_retirement(
        &self,
        key: ProviderInstanceKey,
    ) -> Result<Option<ProviderRetirementGuard>, ProviderRegistryError> {
        let mut state = lock_state(&self.inner);
        if state.blocked.contains_key(&key) {
            return Ok(None);
        }
        state.unready.remove(&key);
        let entry = state.entries.get(&key).cloned();
        let generation = if let Some(entry) = &entry {
            entry.retiring.store(true, Ordering::Release);
            entry.generation
        } else {
            let generation = state
                .next_generation
                .checked_add(1)
                .ok_or(ProviderRegistryError::GenerationExhausted)?;
            state.next_generation = generation;
            generation
        };
        state.blocked.insert(key.clone(), generation);
        drop(state);

        let Some(entry) = entry else {
            return Ok(None);
        };
        finalize_if_drained(&entry);
        Ok(Some(ProviderRetirementGuard {
            key,
            generation,
            registry: Arc::downgrade(&self.inner),
            entry: Arc::downgrade(&entry),
        }))
    }

    /// Release a durable-retirement tombstone after the owning installation row
    /// has been deleted successfully.
    ///
    /// Until completion, the block prevents a failed cleanup from resurrecting
    /// the exact key. Once persistence no longer contains the claimed row, a
    /// deterministic reinstall may safely register a new generation.
    #[cfg(feature = "local-models")]
    pub(crate) fn unblock_after_completed_retirement(&self, key: &ProviderInstanceKey) {
        let mut state = lock_state(&self.inner);
        debug_assert!(
            !state.entries.contains_key(key),
            "a completed retirement must not retain a registered generation"
        );
        state.blocked.remove(key);
        state.retired.remove(key);
    }

    /// Record a structurally valid exact instance that is temporarily unavailable.
    ///
    /// Successful registration clears this marker. Durable retirement takes
    /// precedence and prevents a claimed key from becoming unready or ready.
    #[cfg(feature = "local-models")]
    pub(crate) fn mark_unready(&self, key: ProviderInstanceKey) {
        let mut state = lock_state(&self.inner);
        if state.blocked.contains_key(&key) || state.entries.contains_key(&key) {
            return;
        }
        state.unready.insert(key);
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProviderRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock_state(&self.inner);
        formatter
            .debug_struct("ProviderRegistry")
            .field("registered_instances", &state.entries.len())
            .finish()
    }
}

/// Opaque registration token for one exact provider generation.
#[derive(Clone)]
pub struct ProviderRegistration {
    key: ProviderInstanceKey,
    generation: u64,
    registry: Weak<RegistryInner>,
    entry: Weak<RegistryEntry>,
}

impl ProviderRegistration {
    /// Return the immutable provider instance key.
    #[must_use]
    pub fn key(&self) -> &ProviderInstanceKey {
        &self.key
    }

    /// Return the registry-local generation used for diagnostics and tests.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl fmt::Debug for ProviderRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRegistration")
            .field("key", &self.key)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// Lease retaining one exact provider generation through an execution chain.
pub struct ProviderInstanceLease {
    entry: Arc<RegistryEntry>,
}

impl ProviderInstanceLease {
    /// Return the immutable provider instance key.
    #[must_use]
    pub fn key(&self) -> &ProviderInstanceKey {
        &self.entry.key
    }

    /// Return the exact leased registry generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.entry.generation
    }

    /// Borrow provider operations without exposing a clonable handle that could
    /// escape the lease's retirement guard.
    #[must_use]
    pub fn operations(&self) -> &dyn ProviderOperations {
        self.entry.provider.as_ref()
    }
}

impl fmt::Debug for ProviderInstanceLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderInstanceLease")
            .field("key", &self.entry.key)
            .field("generation", &self.entry.generation)
            .finish_non_exhaustive()
    }
}

impl Drop for ProviderInstanceLease {
    fn drop(&mut self) {
        let previous = self.entry.leases.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "provider lease count underflow");
        if previous == 1 && self.entry.retiring.load(Ordering::Acquire) {
            finalize_if_drained(&self.entry);
        }
    }
}

/// Opaque proof that one exact provider selection was ready when leased.
///
/// This token is intentionally non-clonable. Its registry lease stays alive
/// until the persistence operation using the proof has completed.
pub struct ProviderReadySelection {
    selection: ProviderSelectionSnapshot,
    lease: ProviderInstanceLease,
}

impl ProviderReadySelection {
    /// Borrow the normalized exact selection covered by this proof.
    #[must_use]
    pub fn selection(&self) -> &ProviderSelectionSnapshot {
        &self.selection
    }

    /// Return the exact ready instance key.
    #[must_use]
    pub fn key(&self) -> &ProviderInstanceKey {
        self.lease.key()
    }

    /// Return the leased registry generation for diagnostics and tests.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.lease.generation()
    }
}

impl fmt::Debug for ProviderReadySelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderReadySelection")
            .field("selection", &self.selection)
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

/// Failure to prove that an exact selection currently has a ready instance.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProviderReadySelectionError {
    /// The supplied selection was not a normalized exact durable selection.
    #[error("invalid ready provider selection: {0}")]
    InvalidSelection(ProviderSelectionError),
    /// The exact instance could not issue a new registry lease.
    #[error(transparent)]
    Registry(#[from] ProviderRegistryError),
}

/// Guard for retirement of one exact provider generation.
///
/// The guard does not retain the provider. Existing leases do; after the last
/// lease drains, the registry removes only the matching generation.
#[derive(Clone)]
pub struct ProviderRetirementGuard {
    key: ProviderInstanceKey,
    generation: u64,
    registry: Weak<RegistryInner>,
    entry: Weak<RegistryEntry>,
}

impl ProviderRetirementGuard {
    /// Return the retired provider instance key.
    #[must_use]
    pub fn key(&self) -> &ProviderInstanceKey {
        &self.key
    }

    /// Return the exact retired generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Return whether all leases for this generation have drained.
    #[must_use]
    pub fn is_drained(&self) -> bool {
        self.entry
            .upgrade()
            .is_none_or(|entry| entry.leases.load(Ordering::Acquire) == 0)
    }

    /// Retry same-generation final removal after observing external progress.
    ///
    /// Lease drops already attempt this automatically. This method is useful
    /// for deterministic shutdown and tests.
    pub fn try_finalize(&self) {
        let Some(entry) = self.entry.upgrade() else {
            return;
        };
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let registry = Arc::downgrade(&registry);
        if !Weak::ptr_eq(&registry, &entry.registry) {
            return;
        }
        finalize_if_drained(&entry);
    }
}

impl fmt::Debug for ProviderRetirementGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRetirementGuard")
            .field("key", &self.key)
            .field("generation", &self.generation)
            .field("drained", &self.is_drained())
            .finish_non_exhaustive()
    }
}

/// Provider registry availability or token failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProviderRegistryError {
    /// No ready instance is registered for the selected immutable key.
    #[error("provider instance is not registered: {key}")]
    Missing {
        /// Missing immutable instance key.
        key: ProviderInstanceKey,
    },
    /// Structurally valid instance is known but its runtime is unavailable.
    #[error("provider instance is temporarily unavailable: {key}")]
    Unready {
        /// Unavailable immutable instance key.
        key: ProviderInstanceKey,
    },
    /// The selected generation is retiring and rejects new leases.
    #[error("provider instance is retiring: {key} generation {generation}")]
    Retiring {
        /// Retiring immutable instance key.
        key: ProviderInstanceKey,
        /// Retiring registry generation.
        generation: u64,
    },
    /// A retirement token came from another registry.
    #[error("provider registration belongs to a different registry")]
    ForeignRegistration,
    /// The registry exhausted its generation token space.
    #[error("provider registry generation space is exhausted")]
    GenerationExhausted,
    /// A generation accumulated more simultaneous leases than can be counted.
    #[error("provider instance lease count is exhausted: {key} generation {generation}")]
    LeaseCountExhausted {
        /// Immutable instance key.
        key: ProviderInstanceKey,
        /// Registry generation.
        generation: u64,
    },
}

#[derive(Default)]
struct RegistryInner {
    state: Mutex<RegistryState>,
}

#[derive(Default)]
struct RegistryState {
    entries: HashMap<ProviderInstanceKey, Arc<RegistryEntry>>,
    retired: HashMap<ProviderInstanceKey, u64>,
    blocked: HashMap<ProviderInstanceKey, u64>,
    unready: HashSet<ProviderInstanceKey>,
    next_generation: u64,
}

struct RegistryEntry {
    key: ProviderInstanceKey,
    generation: u64,
    provider: ProviderHandle,
    registry: Weak<RegistryInner>,
    retiring: AtomicBool,
    leases: AtomicUsize,
}

fn lock_state(registry: &RegistryInner) -> MutexGuard<'_, RegistryState> {
    registry
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn finalize_if_drained(entry: &Arc<RegistryEntry>) {
    if !entry.retiring.load(Ordering::Acquire) || entry.leases.load(Ordering::Acquire) != 0 {
        return;
    }
    let Some(registry) = entry.registry.upgrade() else {
        return;
    };
    let removed = {
        let mut state = lock_state(&registry);
        let matches_generation = state.entries.get(&entry.key).is_some_and(|current| {
            current.generation == entry.generation && Arc::ptr_eq(current, entry)
        });
        if matches_generation
            && entry.retiring.load(Ordering::Acquire)
            && entry.leases.load(Ordering::Acquire) == 0
        {
            let removed = state.entries.remove(&entry.key);
            state.retired.insert(entry.key.clone(), entry.generation);
            removed
        } else {
            None
        }
    };
    drop(removed);
}

#[cfg(test)]
mod tests;
