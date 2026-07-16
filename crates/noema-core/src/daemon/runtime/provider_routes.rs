//! Temporary process-local provider routing for legacy core consumers.
//!
//! Durable selections do not contain exact instance identity until Phase 10C.
//! This bridge resolves the current provider for a selected kind, attaches its
//! process-local key only to the returned in-memory snapshot, and leases the
//! exact registry generation used by the execution chain.

use std::{fmt, str::FromStr, sync::Arc};

use noema_providers::{
    ProviderHandle, ProviderInstanceKey, ProviderKind, ProviderRegistration, ProviderRegistry,
    ProviderRegistryError, ProviderRouteError, ProviderRouteFuture, ProviderRouteLease,
    ProviderRouteResolver, ProviderRouteResolverHandle, ProviderSelectionError,
    ProviderSelectionLoaderHandle, ProviderSelectionSnapshot,
};

const LEGACY_INSTANCE_KEY_PREFIX: &str = "legacy-process:";

/// Core-private bridge from legacy kind-based selection to exact provider leases.
#[derive(Clone)]
pub(crate) struct LegacyProviderRoutes {
    registry: Arc<ProviderRegistry>,
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
        };
        for (provider_kind, provider) in providers {
            routes.register(provider_kind.as_ref(), provider)?;
        }
        Ok(routes)
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
        let key = Self::key_for_kind(&selection.provider_kind)?;
        let instance = self.registry.lease(&key).map_err(route_registry_error)?;
        selection.provider_instance_key = Some(key);
        ProviderRouteLease::try_new(selection, instance)
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
            let selection = self.loader.load_selection().await?;
            self.routes.resolve_snapshot(selection)
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
    use noema_providers::{
        GenerateRequest, GenerateResponse, GenerateStreamEvent, ProviderOperationFuture,
        ProviderOperations, ReasoningEffort, provider_selection_loader,
    };

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
        ProviderSelectionSnapshot::explicit(
            provider_kind,
            format!("provider_account:{provider_kind}:account-7"),
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
