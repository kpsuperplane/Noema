//! Local-model persistence boundaries.

use std::sync::Arc;

use crate::{
    LocalModelEventRecord, LocalModelInstallationRecord, LocalModelInstallationUpdate,
    NewLocalModelInstallation, ProviderInstanceKey, ProviderPersistenceFuture,
    ProviderReadySelection, RemovedLocalModelInstallation,
};

/// Durable source capable of creating a future lease for one local instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalModelInstanceReferenceSource {
    /// The installation is the active local-model bootstrap target.
    ActiveInstallation,
    /// System default model selection.
    DefaultModelPreference,
    /// One agent's runtime preference.
    AgentRuntimePreference {
        /// Agent whose runtime preference owns the reference.
        agent_id: String,
    },
    /// One auxiliary workload preference.
    AuxiliaryModelPreference {
        /// Auxiliary workload identity.
        workload: String,
    },
    /// One enabled task-model pool entry.
    TaskModelPool {
        /// Referencing pool-entry identity.
        pool_entry_id: String,
    },
    /// A nonterminal task snapshot that may create another run.
    TaskSnapshot {
        /// Referencing task identity.
        task_id: String,
    },
    /// A nonterminal or resumable agent-run snapshot.
    AgentRunSnapshot {
        /// Referencing run identity.
        run_id: String,
    },
}

/// One exact local-model reference discovered during lifecycle reconciliation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModelInstanceReference {
    /// Immutable provider instance identity.
    pub provider_instance_key: ProviderInstanceKey,
    /// Durable source that can create or resume a lease.
    pub source: LocalModelInstanceReferenceSource,
}

/// Transactionally consistent input for manager startup and reaping.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalModelReconstructionSnapshot {
    /// Every persisted installation, including claimed cleanup work.
    pub installations: Vec<LocalModelInstallationRecord>,
    /// Every canonical or future-lease-eligible exact reference.
    pub references: Vec<LocalModelInstanceReference>,
}

/// Durable cleanup plan retained until artifact and row deletion completes.
#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedLocalModelInstallation {
    /// Claimed installation metadata.
    pub installation: LocalModelInstallationRecord,
}

/// Result of the atomic zero-reference retirement claim.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalModelRetirementClaimResult {
    /// This call atomically established the durable claim.
    Claimed(ClaimedLocalModelInstallation),
    /// A previous call or process already established the durable claim.
    AlreadyClaimed(ClaimedLocalModelInstallation),
    /// Durable future references still pin the exact instance.
    Referenced {
        /// Installation that could not be claimed.
        installation: LocalModelInstallationRecord,
        /// Sources that prevent retirement.
        references: Vec<LocalModelInstanceReferenceSource>,
    },
    /// No installation owns the exact key.
    Missing,
}

/// Result of atomically establishing reversible runtime-retirement intent.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalModelRuntimeRetirementResult {
    /// This call atomically established the durable stop intent.
    Retired(LocalModelInstallationRecord),
    /// A previous reaper already established the durable stop intent.
    AlreadyRetired(LocalModelInstallationRecord),
    /// Durable future references still require the exact runtime.
    Referenced {
        /// Installation whose runtime must remain available.
        installation: LocalModelInstallationRecord,
        /// Sources that prevent runtime retirement.
        references: Vec<LocalModelInstanceReferenceSource>,
    },
    /// No installation owns the exact key.
    Missing,
}

/// Local-model installation lifecycle persistence.
pub trait LocalModelInstallationPersistence: Send + Sync {
    /// Create or refresh queued installation provenance.
    fn upsert_local_model_installation(
        &self,
        input: NewLocalModelInstallation,
    ) -> ProviderPersistenceFuture<'_, LocalModelInstallationRecord>;

    /// Return one installation by id.
    fn local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, Option<LocalModelInstallationRecord>>;

    /// Return every installation in stable presentation order.
    fn local_model_installations(
        &self,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelInstallationRecord>>;

    /// Persist one valid lifecycle transition and its event atomically.
    fn update_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
        update: LocalModelInstallationUpdate,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord>;

    /// Cancel one unfinished installation.
    fn cancel_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord>;

    /// Remove one terminal, non-installed installation projection.
    ///
    /// Implementations must reject installed, active, or in-progress rows. An
    /// installed artifact can be deleted only through the lifecycle claim port.
    fn remove_terminal_local_model_installation<'a>(
        &'a self,
        installation_id: &'a str,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation>;

    /// Return lifecycle events strictly after an optional cursor.
    fn local_model_events(
        &self,
        after_cursor: Option<u64>,
        limit: u32,
    ) -> ProviderPersistenceFuture<'_, Vec<LocalModelEventRecord>>;
}

/// Clonable local-model installation persistence handle.
pub type LocalModelInstallationPersistenceHandle = Arc<dyn LocalModelInstallationPersistence>;

/// Coarse system-wide local-model activation persistence.
pub trait LocalModelActivationPersistence: Send + Sync {
    /// Atomically publish an installed model, optionally assigning every
    /// current workload, and return its committed installation projection.
    fn publish_local_model<'a>(
        &'a self,
        installation_id: &'a str,
        ready_selection: &'a ProviderReadySelection,
        assign_workloads: bool,
    ) -> ProviderPersistenceFuture<'a, LocalModelInstallationRecord>;
}

/// Clonable local-model activation persistence handle.
pub type LocalModelActivationPersistenceHandle = Arc<dyn LocalModelActivationPersistence>;

/// Durable local-model reconstruction and retirement coordination.
///
/// Claim implementations must prove zero references and persist the claim in
/// one writer transaction. A claim is monotonic until durable row deletion
/// completes; only after that completion may a deterministic reinstall create
/// the same exact identity again.
pub trait LocalModelLifecyclePersistence: Send + Sync {
    /// Read every installation and future lease reference from one consistent snapshot.
    fn local_model_reconstruction_snapshot(
        &self,
    ) -> ProviderPersistenceFuture<'_, LocalModelReconstructionSnapshot>;

    /// Atomically establish reversible stop intent after proving zero references.
    fn retire_unreferenced_instance_runtime<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRuntimeRetirementResult>;

    /// Atomically claim one inactive, unreferenced exact instance for removal.
    fn claim_unreferenced_instance_for_retirement<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, LocalModelRetirementClaimResult>;

    /// Delete one claimed row after its runtime cleanup completes.
    ///
    /// Verified blobs are retained for future digest-aware garbage collection;
    /// callers may clean up only installation-scoped partial artifacts.
    fn complete_claimed_local_model_removal<'a>(
        &'a self,
        provider_instance_key: &'a ProviderInstanceKey,
    ) -> ProviderPersistenceFuture<'a, RemovedLocalModelInstallation>;
}

/// Clonable local-model lifecycle persistence handle.
pub type LocalModelLifecyclePersistenceHandle = Arc<dyn LocalModelLifecyclePersistence>;
