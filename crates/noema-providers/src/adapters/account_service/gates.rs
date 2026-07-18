//! Shared serialization gates for provider-account credential operations.

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex as StdMutex, Weak},
};

use tokio::sync::Mutex;

/// Registry of weakly held per-account operation gates.
///
/// Every account service and runtime credential reader must share the same
/// registry instance. Gates disappear from the registry after their last
/// strong handle is dropped, so accounts do not accumulate permanent lock
/// entries.
#[derive(Clone, Default)]
pub struct AccountGateRegistry {
    gates: Arc<StdMutex<HashMap<String, Weak<Mutex<()>>>>>,
}

impl fmt::Debug for AccountGateRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gate_count = self
            .gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        formatter
            .debug_struct("AccountGateRegistry")
            .field("gate_count", &gate_count)
            .finish()
    }
}

impl AccountGateRegistry {
    /// Build an empty gate registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the shared serialization gate for one provider account.
    #[must_use]
    pub fn gate(&self, provider_account_id: &str) -> Arc<Mutex<()>> {
        let mut gates = self
            .gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(provider_account_id).and_then(Weak::upgrade) {
            return gate;
        }

        let gate = Arc::new(Mutex::new(()));
        gates.insert(provider_account_id.to_string(), Arc::downgrade(&gate));
        gate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn live_gate_serializes_account_operations() {
        let registry = AccountGateRegistry::new();
        let first = registry.gate("provider_account:exa:first");
        let second = registry.gate("provider_account:exa:first");
        let other = registry.gate("provider_account:exa:second");
        assert!(
            !Arc::ptr_eq(&first, &other),
            "different accounts must receive different gates"
        );
        let guard = first.lock().await;

        assert!(second.try_lock().is_err());
        drop(guard);
        assert!(second.try_lock().is_ok());

        let released = registry.gate("provider_account:exa:released");
        let released_weak = Arc::downgrade(&released);
        drop(released);
        let replacement = registry.gate("provider_account:exa:released");
        assert!(
            released_weak.upgrade().is_none(),
            "released gate must not retain the account key"
        );
        assert_eq!(Arc::strong_count(&replacement), 1);
    }
}
