use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;
use crate::{GenerateRequest, GenerateResponse, ProviderOperationFuture, ProviderOperations};

#[derive(Debug)]
struct TrackedProvider {
    label: &'static str,
    drops: Arc<AtomicUsize>,
}

impl Drop for TrackedProvider {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl ProviderOperations for TrackedProvider {
    fn generate_streaming<'a>(
        &'a self,
        _request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(crate::GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(async move {
            Ok(GenerateResponse::final_text(
                self.label,
                "registry-test",
                "registry-test-model",
            ))
        })
    }
}

fn key(value: &str) -> ProviderInstanceKey {
    ProviderInstanceKey::new(value).expect("valid provider instance key")
}

fn provider(label: &'static str, drops: Arc<AtomicUsize>) -> ProviderHandle {
    Arc::new(TrackedProvider { label, drops })
}

#[test]
fn retirement_rejects_new_leases_and_cleans_up_after_the_last_lease() {
    let registry = ProviderRegistry::new();
    let instance_key = key("provider_account:codex:default");
    let drops = Arc::new(AtomicUsize::new(0));
    let registration = registry
        .register(instance_key.clone(), provider("old", Arc::clone(&drops)))
        .expect("register provider");
    let lease = registry.lease(&instance_key).expect("lease provider");

    let retirement = registry
        .begin_retirement(&registration)
        .expect("begin retirement");
    assert_eq!(
        registry.lease(&instance_key).expect_err("retiring"),
        ProviderRegistryError::Retiring {
            key: instance_key.clone(),
            generation: registration.generation(),
        }
    );
    assert!(!retirement.is_drained());
    assert_eq!(drops.load(Ordering::SeqCst), 0);

    drop(lease);

    assert!(retirement.is_drained());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        registry.lease(&instance_key).expect_err("retired"),
        ProviderRegistryError::Retiring {
            key: instance_key,
            generation: registration.generation()
        }
    );
}

#[test]
fn replacement_clears_a_drained_retirement_tombstone() {
    let registry = ProviderRegistry::new();
    let instance_key = key("provider_account:codex:retired-replacement");
    let registration = registry
        .register(
            instance_key.clone(),
            provider("old", Arc::new(AtomicUsize::new(0))),
        )
        .expect("register old provider");
    registry
        .begin_retirement(&registration)
        .expect("retire old provider");
    assert!(matches!(
        registry.lease(&instance_key),
        Err(ProviderRegistryError::Retiring { .. })
    ));

    let replacement = registry
        .register(
            instance_key.clone(),
            provider("new", Arc::new(AtomicUsize::new(0))),
        )
        .expect("register replacement");

    assert_eq!(
        registry
            .lease(&instance_key)
            .expect("replacement lease")
            .generation(),
        replacement.generation()
    );
}

#[tokio::test]
async fn replacement_installs_a_new_generation_while_old_leases_remain_valid() {
    let registry = ProviderRegistry::new();
    let instance_key = key("provider_account:openai:default");
    let old_drops = Arc::new(AtomicUsize::new(0));
    let new_drops = Arc::new(AtomicUsize::new(0));
    let old_registration = registry
        .register(
            instance_key.clone(),
            provider("old", Arc::clone(&old_drops)),
        )
        .expect("register old provider");
    let old_lease = registry.lease(&instance_key).expect("lease old provider");

    let new_registration = registry
        .register(
            instance_key.clone(),
            provider("new", Arc::clone(&new_drops)),
        )
        .expect("replace provider");
    let new_lease = registry.lease(&instance_key).expect("lease new provider");

    assert_ne!(old_registration.generation(), new_registration.generation());
    assert_eq!(old_lease.generation(), old_registration.generation());
    assert_eq!(new_lease.generation(), new_registration.generation());
    assert_eq!(old_drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        old_lease
            .operations()
            .generate(GenerateRequest::text("still usable"))
            .await
            .expect("old leased provider remains usable")
            .assistant_text(),
        "old"
    );
    assert_eq!(
        new_lease
            .operations()
            .generate(GenerateRequest::text("replacement"))
            .await
            .expect("replacement provider")
            .assistant_text(),
        "new"
    );

    drop(old_lease);

    assert_eq!(old_drops.load(Ordering::SeqCst), 1);
    assert_eq!(new_drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        registry
            .lease(&instance_key)
            .expect("replacement remains")
            .generation(),
        new_registration.generation()
    );
}

#[test]
fn paused_old_generation_retirement_cannot_remove_a_replacement() {
    let registry = ProviderRegistry::new();
    let instance_key = key("provider_account:local_models:installation:gemma");
    let old_drops = Arc::new(AtomicUsize::new(0));
    let new_drops = Arc::new(AtomicUsize::new(0));
    let old_registration = registry
        .register(
            instance_key.clone(),
            provider("old", Arc::clone(&old_drops)),
        )
        .expect("register old provider");
    let old_lease = registry.lease(&instance_key).expect("lease old provider");
    let old_retirement = registry
        .begin_retirement(&old_registration)
        .expect("pause retirement behind lease");

    let new_registration = registry
        .register(
            instance_key.clone(),
            provider("new", Arc::clone(&new_drops)),
        )
        .expect("register replacement");
    old_retirement.try_finalize();
    assert_eq!(
        registry
            .lease(&instance_key)
            .expect("replacement lease")
            .generation(),
        new_registration.generation()
    );

    drop(old_lease);
    old_retirement.try_finalize();

    assert!(old_retirement.is_drained());
    assert_eq!(old_drops.load(Ordering::SeqCst), 1);
    assert_eq!(new_drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        registry
            .lease(&instance_key)
            .expect("old guard preserved replacement")
            .generation(),
        new_registration.generation()
    );
}

#[test]
fn registration_tokens_cannot_retire_another_registry() {
    let first = ProviderRegistry::new();
    let second = ProviderRegistry::new();
    let instance_key = key("provider_account:codex:foreign");
    let registration = first
        .register(
            instance_key,
            provider("first", Arc::new(AtomicUsize::new(0))),
        )
        .expect("register provider");

    assert_eq!(
        second
            .begin_retirement(&registration)
            .expect_err("foreign registration"),
        ProviderRegistryError::ForeignRegistration
    );
}
