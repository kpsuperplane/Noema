use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::oneshot;

use super::*;
use crate::{
    GenerateRequest, GenerateResponse, ProviderHandle, ProviderOperationFuture, ProviderOperations,
    ProviderRegistry, ProviderSelectionSnapshot,
};

#[derive(Debug)]
struct StaticProvider(&'static str);

impl ProviderOperations for StaticProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(crate::GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(async move {
            Ok(GenerateResponse::final_text(
                self.0,
                "route-test",
                "route-test-model",
            ))
        })
    }
}

#[derive(Debug)]
struct SequenceLoader {
    snapshots: Mutex<VecDeque<ProviderSelectionSnapshot>>,
    last: Mutex<Option<ProviderSelectionSnapshot>>,
}

struct PausedSelectionLoader {
    current: Arc<Mutex<ProviderSelectionSnapshot>>,
    first_read: AtomicBool,
    read_tx: Mutex<Option<oneshot::Sender<()>>>,
    resume_rx: Mutex<Option<oneshot::Receiver<()>>>,
}

impl ProviderSelectionLoader for PausedSelectionLoader {
    fn load_selection(&self) -> ProviderRouteFuture<'_, ProviderSelectionSnapshot> {
        let snapshot = self.current.lock().expect("current selection").clone();
        if !self.first_read.swap(true, Ordering::AcqRel) {
            let read_tx = self.read_tx.lock().expect("read signal").take();
            let resume_rx = self.resume_rx.lock().expect("resume signal").take();
            return Box::pin(async move {
                read_tx
                    .expect("first read sender")
                    .send(())
                    .expect("signal first read");
                resume_rx
                    .expect("first read receiver")
                    .await
                    .expect("resume first read");
                Ok(snapshot)
            });
        }
        Box::pin(async move { Ok(snapshot) })
    }
}

impl SequenceLoader {
    fn new(snapshots: impl IntoIterator<Item = ProviderSelectionSnapshot>) -> Self {
        Self {
            snapshots: Mutex::new(snapshots.into_iter().collect()),
            last: Mutex::new(None),
        }
    }
}

impl ProviderSelectionLoader for SequenceLoader {
    fn load_selection(&self) -> ProviderRouteFuture<'_, ProviderSelectionSnapshot> {
        let next = self
            .snapshots
            .lock()
            .expect("snapshot queue")
            .pop_front()
            .or_else(|| self.last.lock().expect("last snapshot").clone());
        if let Some(snapshot) = &next {
            *self.last.lock().expect("last snapshot") = Some(snapshot.clone());
        }
        Box::pin(async move {
            next.ok_or(ProviderRouteError::SelectionLoad {
                operation: "test_selection",
            })
        })
    }
}

fn key(value: &str) -> ProviderInstanceKey {
    ProviderInstanceKey::new(value).expect("valid instance key")
}

fn selection(instance_key: ProviderInstanceKey) -> ProviderSelectionSnapshot {
    let mut selection = ProviderSelectionSnapshot::explicit(
        "openai",
        "provider_account:openai:default",
        "gpt-test",
        None,
        None,
    );
    selection.provider_instance_key = Some(instance_key);
    selection
}

fn provider(label: &'static str) -> ProviderHandle {
    Arc::new(StaticProvider(label))
}

fn resolver(
    loader: impl ProviderSelectionLoader + 'static,
    registry: Arc<ProviderRegistry>,
) -> RegistryProviderRouteResolver {
    RegistryProviderRouteResolver::new(Arc::new(loader), registry)
}

#[tokio::test]
async fn changed_snapshot_retries_onto_the_new_ready_instance() {
    let registry = Arc::new(ProviderRegistry::new());
    let old_key = key("openai:default:old");
    let new_key = key("openai:default:new");
    let old_registration = registry
        .register(old_key.clone(), provider("old"))
        .expect("register old");
    let current = Arc::new(Mutex::new(selection(old_key)));
    let (read_tx, read_rx) = oneshot::channel();
    let (resume_tx, resume_rx) = oneshot::channel();
    let resolver = resolver(
        PausedSelectionLoader {
            current: Arc::clone(&current),
            first_read: AtomicBool::new(false),
            read_tx: Mutex::new(Some(read_tx)),
            resume_rx: Mutex::new(Some(resume_rx)),
        },
        Arc::clone(&registry),
    );
    let resolution = tokio::spawn(async move { resolver.resolve_route().await });

    read_rx.await.expect("selection read paused");
    registry
        .begin_retirement(&old_registration)
        .expect("retire old");
    let new_registration = registry
        .register(new_key.clone(), provider("new"))
        .expect("register new");
    *current.lock().expect("current selection") = selection(new_key.clone());
    resume_tx.send(()).expect("resume stale read");

    let route = resolution
        .await
        .expect("resolver task")
        .expect("resolved route");

    assert_eq!(route.key(), &new_key);
    assert_eq!(route.generation(), new_registration.generation());
}

#[tokio::test]
async fn unchanged_retiring_selection_is_an_invariant() {
    let registry = Arc::new(ProviderRegistry::new());
    let instance_key = key("openai:default:retiring");
    let registration = registry
        .register(instance_key.clone(), provider("old"))
        .expect("register");
    registry.begin_retirement(&registration).expect("retire");
    let resolver = resolver(
        SequenceLoader::new([selection(instance_key.clone())]),
        registry,
    );

    assert_eq!(
        resolver.resolve_route().await.expect_err("invariant"),
        ProviderRouteError::RetiringSelectionInvariant { key: instance_key }
    );
}

#[tokio::test]
async fn unchanged_missing_selection_is_typed_unavailability() {
    let registry = Arc::new(ProviderRegistry::new());
    let instance_key = key("openai:default:missing");
    let resolver = resolver(
        SequenceLoader::new([selection(instance_key.clone())]),
        registry,
    );

    assert_eq!(
        resolver.resolve_route().await.expect_err("missing"),
        ProviderRouteError::InstanceMissing { key: instance_key }
    );
}

#[tokio::test]
async fn continuous_selection_churn_is_bounded() {
    let registry = Arc::new(ProviderRegistry::new());
    let snapshots = (0..=MAX_SELECTION_RETRIES)
        .map(|index| selection(key(&format!("openai:default:missing-{index}"))))
        .collect::<Vec<_>>();
    let resolver = resolver(SequenceLoader::new(snapshots), registry);

    assert_eq!(
        resolver
            .resolve_route()
            .await
            .expect_err("bounded conflict"),
        ProviderRouteError::SelectionConflict
    );
}

#[tokio::test]
async fn strict_resolver_never_guesses_a_missing_instance_key() {
    let registry = Arc::new(ProviderRegistry::new());
    let unresolved = ProviderSelectionSnapshot::explicit(
        "openai",
        "provider_account:openai:default",
        "gpt-test",
        None,
        None,
    );
    let resolver = resolver(SequenceLoader::new([unresolved]), registry);

    assert_eq!(
        resolver.resolve_route().await.expect_err("missing key"),
        ProviderRouteError::MissingInstanceKey
    );
}

#[test]
fn route_lease_rejects_missing_or_mismatched_provenance() {
    let registry = ProviderRegistry::new();
    let leased_key = key("openai:default:leased");
    registry
        .register(leased_key.clone(), provider("leased"))
        .expect("register");

    let unresolved = ProviderSelectionSnapshot::explicit(
        "openai",
        "provider_account:openai:default",
        "gpt-test",
        None,
        None,
    );
    assert_eq!(
        ProviderRouteLease::try_new(
            unresolved,
            registry.lease(&leased_key).expect("lease unresolved test")
        )
        .expect_err("missing key"),
        ProviderRouteError::MissingInstanceKey
    );

    let selected_key = key("openai:default:selected");
    assert_eq!(
        ProviderRouteLease::try_new(
            selection(selected_key.clone()),
            registry.lease(&leased_key).expect("lease mismatch test")
        )
        .expect_err("mismatched key"),
        ProviderRouteError::InstanceKeyMismatch {
            selected: selected_key,
            leased: leased_key
        }
    );
}
