//! Temporary process-local provider routing for legacy core consumers.
//!
//! Durable selections do not contain exact instance identity until Phase 10C.
//! This bridge resolves the current provider for a selected kind, attaches its
//! process-local key only to the returned in-memory snapshot, and leases the
//! exact registry generation used by the execution chain.

use std::{fmt, str::FromStr, sync::Arc};

use noema_providers::{
    LocalModelRouteHandle, LocalModelRouteReadGuard, ProviderHandle, ProviderInstanceKey,
    ProviderKind, ProviderRegistration, ProviderRegistry, ProviderRegistryError,
    ProviderRouteError, ProviderRouteFuture, ProviderRouteLease, ProviderRouteResolver,
    ProviderRouteResolverHandle, ProviderSelectionError, ProviderSelectionLoaderHandle,
    ProviderSelectionSnapshot,
};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

const LEGACY_INSTANCE_KEY_PREFIX: &str = "legacy-process:";
const SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID: &str = "provider_account:codex:default";

/// Core-private bridge from legacy kind-based selection to exact provider leases.
#[derive(Clone)]
pub(crate) struct LegacyProviderRoutes {
    registry: Arc<ProviderRegistry>,
    publication_gate: Arc<RwLock<()>>,
    local_models: Option<LocalModelRouteHandle>,
}

impl LegacyProviderRoutes {
    /// Register the initial process-local providers.
    pub(crate) fn new<I, K>(providers: I) -> Result<Self, ProviderRouteError>
    where
        I: IntoIterator<Item = (K, ProviderHandle)>,
        K: AsRef<str>,
    {
        let routes = Self {
            registry: Arc::new(ProviderRegistry::new()),
            publication_gate: Arc::new(RwLock::new(())),
            local_models: None,
        };
        for (provider_kind, provider) in providers {
            routes.register(provider_kind.as_ref(), provider)?;
        }
        Ok(routes)
    }

    /// Attach the provider-owned exact-instance route for local models.
    #[must_use]
    pub(crate) fn with_local_models_route(mut self, route: LocalModelRouteHandle) -> Self {
        self.local_models = Some(route);
        self
    }

    /// Resolve through the provider-owned local route when configured.
    pub(crate) async fn resolve_snapshot_async(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, ProviderRouteError> {
        self.read().await.resolve_snapshot(selection)
    }

    /// Register or generation-safely replace the current provider for a kind.
    pub(crate) fn register(
        &self,
        provider_kind: &str,
        provider: ProviderHandle,
    ) -> Result<ProviderRegistration, ProviderRouteError> {
        self.registry
            .register(Self::key_for_kind(provider_kind)?, provider)
            .map_err(|_| ProviderRouteError::Registry {
                operation: "register_legacy_provider_instance",
            })
    }

    /// Return the stable process-local registry key for a provider kind.
    pub(crate) fn key_for_kind(
        provider_kind: &str,
    ) -> Result<ProviderInstanceKey, ProviderRouteError> {
        let provider_kind = normalized_provider_kind(provider_kind)?;
        ProviderInstanceKey::new(format!("{LEGACY_INSTANCE_KEY_PREFIX}{provider_kind}")).map_err(
            |error| ProviderRouteError::InvalidSelection {
                message: error.to_string(),
            },
        )
    }

    /// Resolve a full selection snapshot onto the current process-local provider.
    ///
    /// The input snapshot is normalized and retained in full. Its exact instance
    /// key is overwritten only in the returned route, so legacy durable records
    /// never claim process-local identity.
    pub(crate) fn resolve_snapshot(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, ProviderRouteError> {
        let mut selection =
            selection
                .normalized()
                .map_err(|error| ProviderRouteError::InvalidSelection {
                    message: error.to_string(),
                })?;
        validate_supported_legacy_account(&selection)?;
        let key = Self::key_for_kind(&selection.provider_kind)?;
        let instance = self.registry.lease(&key).map_err(route_registry_error)?;
        selection.provider_instance_key = Some(key);
        ProviderRouteLease::try_new(selection, instance)
    }

    /// Acquire shared publication access before reading a durable selection.
    ///
    /// The guard must cover the canonical repository read and
    /// [`LegacyProviderRouteReadGuard::resolve_snapshot`] call. This prevents an
    /// activation from committing durable state and publishing its replacement
    /// registry generation between those two operations.
    pub(crate) async fn read(&self) -> LegacyProviderRouteReadGuard {
        let gate = Arc::clone(&self.publication_gate).read_owned().await;
        let local_models = match &self.local_models {
            Some(route) => Some(route.read().await),
            None => None,
        };
        LegacyProviderRouteReadGuard {
            routes: self.clone(),
            local_models,
            _gate: gate,
        }
    }

    /// Acquire exclusive publication access for a provider activation.
    ///
    /// Hold this owned guard across the durable selection commit and the
    /// following [`LegacyProviderPublicationGuard::register`] call. Readers
    /// cannot observe the state between those two publication steps.
    pub(crate) async fn begin_publication(&self) -> LegacyProviderPublicationGuard {
        let gate = Arc::clone(&self.publication_gate).write_owned().await;
        LegacyProviderPublicationGuard {
            routes: self.clone(),
            _gate: gate,
        }
    }

    /// Bind an async canonical snapshot loader to this temporary route bridge.
    pub(crate) fn bind(
        &self,
        loader: ProviderSelectionLoaderHandle,
    ) -> ProviderRouteResolverHandle {
        Arc::new(LegacyProviderRouteResolver {
            routes: self.clone(),
            loader,
        })
    }
}

/// Owned shared guard spanning one canonical selection read and route lease.
pub(crate) struct LegacyProviderRouteReadGuard {
    routes: LegacyProviderRoutes,
    local_models: Option<LocalModelRouteReadGuard>,
    _gate: OwnedRwLockReadGuard<()>,
}

impl LegacyProviderRouteReadGuard {
    /// Lease the process-local provider matching a snapshot read under this guard.
    pub(crate) fn resolve_snapshot(
        &self,
        selection: ProviderSelectionSnapshot,
    ) -> Result<ProviderRouteLease, ProviderRouteError> {
        if selection
            .provider_kind
            .trim()
            .eq_ignore_ascii_case(ProviderKind::LocalModels.as_str())
            && let Some(route) = &self.local_models
        {
            return route.resolve_snapshot(selection);
        }
        self.routes.resolve_snapshot(selection)
    }
}

impl fmt::Debug for LegacyProviderRouteReadGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacyProviderRouteReadGuard")
            .finish_non_exhaustive()
    }
}

/// Owned exclusive guard spanning durable activation and registry publication.
pub(crate) struct LegacyProviderPublicationGuard {
    routes: LegacyProviderRoutes,
    _gate: OwnedRwLockWriteGuard<()>,
}

impl LegacyProviderPublicationGuard {
    /// Publish the provider generation after the matching durable commit succeeds.
    pub(crate) fn register(
        &self,
        provider_kind: &str,
        provider: ProviderHandle,
    ) -> Result<ProviderRegistration, ProviderRouteError> {
        self.routes.register(provider_kind, provider)
    }
}

impl fmt::Debug for LegacyProviderPublicationGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacyProviderPublicationGuard")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for LegacyProviderRoutes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegacyProviderRoutes")
            .finish_non_exhaustive()
    }
}

struct LegacyProviderRouteResolver {
    routes: LegacyProviderRoutes,
    loader: ProviderSelectionLoaderHandle,
}

impl ProviderRouteResolver for LegacyProviderRouteResolver {
    fn resolve_route(&self) -> ProviderRouteFuture<'_, ProviderRouteLease> {
        Box::pin(async move {
            let routes = self.routes.read().await;
            let selection = self.loader.load_selection().await?;
            routes.resolve_snapshot(selection)
        })
    }
}

fn normalized_provider_kind(provider_kind: &str) -> Result<&'static str, ProviderRouteError> {
    ProviderKind::from_str(provider_kind)
        .map(|kind| kind.as_str())
        .map_err(|provider_kind| ProviderRouteError::InvalidSelection {
            message: ProviderSelectionError::UnsupportedProvider { provider_kind }.to_string(),
        })
}

fn validate_supported_legacy_account(
    selection: &ProviderSelectionSnapshot,
) -> Result<(), ProviderRouteError> {
    if selection.provider_kind == ProviderKind::Codex.as_str()
        && selection.provider_account_id != SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID
    {
        return Err(ProviderRouteError::InvalidSelection {
            message: format!(
                "legacy Codex routing supports only {SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID}"
            ),
        });
    }
    Ok(())
}

fn route_registry_error(error: ProviderRegistryError) -> ProviderRouteError {
    match error {
        ProviderRegistryError::Missing { key } => ProviderRouteError::InstanceMissing { key },
        ProviderRegistryError::Retiring { key, .. } => {
            ProviderRouteError::RetiringSelectionInvariant { key }
        }
        _ => ProviderRouteError::Registry {
            operation: "lease_legacy_provider_instance",
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use noema_providers::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderOperationFuture,
        ProviderOperations, ReasoningEffort, provider_selection_loader,
    };
    use tokio::sync::{Mutex, oneshot};

    use super::*;

    #[derive(Debug)]
    struct NamedProvider(&'static str);

    impl ProviderOperations for NamedProvider {
        fn generate_streaming<'a>(
            &'a self,
            _request: GenerateRequest,
            _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> ProviderOperationFuture<'a, GenerateResponse> {
            Box::pin(async move {
                Ok(GenerateResponse::final_text(
                    self.0,
                    "legacy-route-test",
                    "legacy-route-test-model",
                ))
            })
        }
    }

    fn provider(name: &'static str) -> ProviderHandle {
        Arc::new(NamedProvider(name))
    }

    fn full_selection(provider_kind: &str) -> ProviderSelectionSnapshot {
        let provider_account_id = if provider_kind.eq_ignore_ascii_case("codex") {
            SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID.to_string()
        } else {
            format!("provider_account:{provider_kind}:account-7")
        };
        ProviderSelectionSnapshot::explicit(
            provider_kind,
            provider_account_id,
            "model-profile-9",
            Some(ReasoningEffort::High),
            Some("task:task-11".to_string()),
        )
    }

    #[tokio::test]
    async fn replacement_keeps_the_key_stable_and_old_leases_alive() {
        let routes =
            LegacyProviderRoutes::new([("codex", provider("old"))]).expect("initial routes");
        let stable_key = LegacyProviderRoutes::key_for_kind("codex").expect("stable key");
        let old_route = routes
            .resolve_snapshot(full_selection("codex"))
            .expect("old route");

        let replacement = routes
            .register("CODEX", provider("new"))
            .expect("replace provider");
        let new_route = routes
            .resolve_snapshot(full_selection("codex"))
            .expect("new route");

        assert_eq!(old_route.key(), &stable_key);
        assert_eq!(new_route.key(), &stable_key);
        assert_ne!(old_route.generation(), new_route.generation());
        assert_eq!(new_route.generation(), replacement.generation());
        assert_eq!(
            old_route
                .operations()
                .generate(GenerateRequest::text("old"))
                .await
                .expect("old generation remains leased")
                .assistant_text(),
            "old"
        );
        assert_eq!(
            new_route
                .operations()
                .generate(GenerateRequest::text("new"))
                .await
                .expect("new generation")
                .assistant_text(),
            "new"
        );
    }

    #[test]
    fn resolution_preserves_full_selection_metadata_and_overwrites_only_the_route_key() {
        let routes =
            LegacyProviderRoutes::new([("local_models", provider("local"))]).expect("routes");
        let mut persisted = full_selection("local_models");
        persisted.provider_instance_key =
            Some(ProviderInstanceKey::new("stale-durable-key").expect("stale key"));
        let persisted_copy = persisted.clone();

        let route = routes.resolve_snapshot(persisted).expect("resolved route");
        let expected_key =
            LegacyProviderRoutes::key_for_kind("local_models").expect("legacy process key");

        assert_eq!(
            route.selection(),
            &ProviderSelectionSnapshot {
                provider_instance_key: Some(expected_key.clone()),
                ..persisted_copy.clone()
            }
        );
        assert_eq!(route.key(), &expected_key);
        assert_eq!(
            persisted_copy
                .provider_instance_key
                .as_ref()
                .map(ProviderInstanceKey::as_str),
            Some("stale-durable-key")
        );
    }

    #[test]
    fn non_default_codex_account_fails_closed() {
        let routes = LegacyProviderRoutes::new([("codex", provider("codex"))]).expect("routes");
        let selection = ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:secondary",
            "model-profile-9",
            Some(ReasoningEffort::High),
            Some("agent:primary".to_string()),
        );

        assert_eq!(
            routes
                .resolve_snapshot(selection)
                .expect_err("unsupported Codex account must not use default credentials"),
            ProviderRouteError::InvalidSelection {
                message: format!(
                    "legacy Codex routing supports only {SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID}"
                ),
            }
        );
    }

    #[tokio::test]
    async fn bound_resolver_loads_the_complete_snapshot_before_routing() {
        let routes = LegacyProviderRoutes::new([("openai", provider("openai"))]).expect("routes");
        let expected = full_selection("openai");
        let loader_snapshot = expected.clone();
        let resolver = routes.bind(provider_selection_loader(move || {
            let snapshot = loader_snapshot.clone();
            Box::pin(async move { Ok(snapshot) })
        }));

        let route = resolver.resolve_route().await.expect("resolved route");

        assert_eq!(route.selection().provider_kind, expected.provider_kind);
        assert_eq!(
            route.selection().provider_account_id,
            expected.provider_account_id
        );
        assert_eq!(route.selection().selection_mode, expected.selection_mode);
        assert_eq!(route.selection().model_profile, expected.model_profile);
        assert_eq!(
            route.selection().reasoning_effort,
            expected.reasoning_effort
        );
        assert_eq!(
            route.selection().selection_source,
            expected.selection_source
        );
        assert_eq!(
            route.selection().provider_instance_key.as_ref(),
            Some(route.key())
        );
    }

    #[tokio::test]
    async fn exclusive_publication_hides_the_durable_registry_middle_state() {
        let routes =
            LegacyProviderRoutes::new([("codex", provider("old"))]).expect("initial routes");
        let current_selection = Arc::new(Mutex::new(full_selection("codex")));
        let (loader_entered_tx, mut loader_entered_rx) = oneshot::channel();
        let loader_entered_tx = Arc::new(std::sync::Mutex::new(Some(loader_entered_tx)));
        let resolver = routes.bind(provider_selection_loader({
            let current_selection = Arc::clone(&current_selection);
            let loader_entered_tx = Arc::clone(&loader_entered_tx);
            move || {
                let current_selection = Arc::clone(&current_selection);
                let loader_entered_tx = Arc::clone(&loader_entered_tx);
                Box::pin(async move {
                    loader_entered_tx
                        .lock()
                        .expect("loader entered sender")
                        .take()
                        .expect("first loader call")
                        .send(())
                        .expect("signal loader entered");
                    Ok(current_selection.lock().await.clone())
                })
            }
        }));
        let publication = routes.begin_publication().await;
        *current_selection.lock().await = ProviderSelectionSnapshot::explicit(
            "codex",
            SUPPORTED_CODEX_PROVIDER_ACCOUNT_ID,
            "replacement-profile",
            Some(ReasoningEffort::Low),
            Some("replacement-commit".to_string()),
        );

        let resolution = tokio::spawn(async move { resolver.resolve_route().await });

        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut loader_entered_rx)
                .await
                .is_err()
        );
        assert!(!resolution.is_finished());

        let replacement = publication
            .register("codex", provider("new"))
            .expect("publish replacement");
        drop(publication);
        loader_entered_rx
            .await
            .expect("loader entered after publish");
        let route = resolution
            .await
            .expect("resolver task")
            .expect("resolved replacement");

        assert_eq!(route.generation(), replacement.generation());
        assert_eq!(
            route.selection().model_profile.as_deref(),
            Some("replacement-profile")
        );
        assert_eq!(
            route
                .operations()
                .generate(GenerateRequest::text("replacement"))
                .await
                .expect("replacement generation")
                .assistant_text(),
            "new"
        );
    }

    #[tokio::test]
    async fn publication_waits_for_an_in_flight_selection_read_and_lease() {
        let routes =
            LegacyProviderRoutes::new([("codex", provider("old"))]).expect("initial routes");
        let (loaded_tx, loaded_rx) = oneshot::channel();
        let loaded_tx = Arc::new(std::sync::Mutex::new(Some(loaded_tx)));
        let (resume_tx, resume_rx) = oneshot::channel();
        let resume_rx = Arc::new(Mutex::new(Some(resume_rx)));
        let resolver = routes.bind(provider_selection_loader({
            let loaded_tx = Arc::clone(&loaded_tx);
            let resume_rx = Arc::clone(&resume_rx);
            move || {
                let loaded_tx = Arc::clone(&loaded_tx);
                let resume_rx = Arc::clone(&resume_rx);
                Box::pin(async move {
                    loaded_tx
                        .lock()
                        .expect("loaded sender")
                        .take()
                        .expect("first loader call")
                        .send(())
                        .expect("signal loaded selection");
                    resume_rx
                        .lock()
                        .await
                        .take()
                        .expect("resume receiver")
                        .await
                        .expect("resume selection load");
                    Ok(full_selection("codex"))
                })
            }
        }));
        let resolution = tokio::spawn(async move { resolver.resolve_route().await });
        loaded_rx.await.expect("selection loader entered");

        let publication_routes = routes.clone();
        let (publication_attempted_tx, publication_attempted_rx) = oneshot::channel();
        let (publication_acquired_tx, mut publication_acquired_rx) = oneshot::channel();
        let publication = tokio::spawn(async move {
            publication_attempted_tx
                .send(())
                .expect("signal publication attempt");
            let publication = publication_routes.begin_publication().await;
            publication_acquired_tx
                .send(())
                .expect("signal publication acquired");
            let registration = publication
                .register("codex", provider("new"))
                .expect("publish replacement");
            (publication, registration)
        });
        publication_attempted_rx
            .await
            .expect("publication task attempted");
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut publication_acquired_rx)
                .await
                .is_err()
        );
        assert!(!publication.is_finished());

        resume_tx.send(()).expect("resume selection load");
        let old_route = resolution.await.expect("resolver task").expect("old route");
        publication_acquired_rx
            .await
            .expect("publication acquired after read");
        let (publication_guard, replacement) = publication.await.expect("publication task");
        drop(publication_guard);

        assert_eq!(
            old_route
                .operations()
                .generate(GenerateRequest::text("old"))
                .await
                .expect("old leased generation")
                .assistant_text(),
            "old"
        );
        let read = routes.read().await;
        let new_route = read
            .resolve_snapshot(full_selection("codex"))
            .expect("new route");
        assert_eq!(new_route.generation(), replacement.generation());
        assert_eq!(
            new_route
                .operations()
                .generate(GenerateRequest::text("new"))
                .await
                .expect("new generation")
                .assistant_text(),
            "new"
        );
    }

    #[test]
    fn missing_and_invalid_kinds_fail_without_fallback() {
        let routes = LegacyProviderRoutes::new(std::iter::empty::<(&str, ProviderHandle)>())
            .expect("empty routes");
        let missing_key = LegacyProviderRoutes::key_for_kind("codex").expect("codex key");

        assert_eq!(
            routes
                .resolve_snapshot(full_selection("codex"))
                .expect_err("missing provider"),
            ProviderRouteError::InstanceMissing { key: missing_key }
        );
        assert!(matches!(
            LegacyProviderRoutes::key_for_kind("unknown"),
            Err(ProviderRouteError::InvalidSelection { .. })
        ));
    }
}
