//! Deterministic composition of provider-neutral capability binding sources.

use crate::{
    CapabilityBindingSource, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogBuilder, CapabilityCatalogResult, CapabilityFuture,
};

/// A request-local catalog source composed from configured child sources.
///
/// Sources are loaded sequentially in the order supplied to [`Self::new`].
/// Their snapshots and availability notices retain that same order, so the
/// resulting catalog is deterministic without introducing another ordering
/// authority. Any duplicate canonical name rejects the complete composition.
pub struct CompositeCapabilityBindingSource {
    sources: Vec<CapabilityBindingSourceHandle>,
}

impl CompositeCapabilityBindingSource {
    /// Construct a composite from its configured child sources.
    #[must_use]
    pub fn new(sources: impl IntoIterator<Item = CapabilityBindingSourceHandle>) -> Self {
        Self {
            sources: sources.into_iter().collect(),
        }
    }
}

impl CapabilityBindingSource for CompositeCapabilityBindingSource {
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>> {
        Box::pin(async move {
            let mut builder = CapabilityCatalogBuilder::new();
            let mut availability_notices = Vec::new();

            for source in &self.sources {
                let result = source.catalog().await?;
                for binding in result.snapshot.iter().cloned() {
                    builder
                        .add(binding)
                        .map_err(|_| CapabilityBindingSourceError::Invalid)?;
                }
                availability_notices.extend(result.availability_notices);
            }

            Ok(CapabilityCatalogResult {
                snapshot: builder.build(),
                availability_notices,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityAccess, CapabilityAvailabilityNotice, CapabilityAvailabilityStatus,
        CapabilityBinding, CapabilityEffect, CapabilityScope, CapabilityTarget, InvokerKey,
        OperationToken, RedactingPayloadSanitizer, ToolName, ToolSpec,
    };
    use serde_json::json;
    use std::{
        future::{self, Future},
        sync::Arc,
        task::{Context, Poll, Waker},
    };

    struct StaticSource {
        result: Result<CapabilityCatalogResult, CapabilityBindingSourceError>,
    }

    impl CapabilityBindingSource for StaticSource {
        fn catalog(
            &self,
        ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>>
        {
            Box::pin(future::ready(self.result.clone()))
        }
    }

    fn binding(name: &str) -> CapabilityBinding {
        CapabilityBinding::new(
            ToolSpec::new(name, "Test operation.", json!({"type":"object"})).expect("spec"),
            CapabilityTarget::new(InvokerKey::new("test"), OperationToken::new(name)),
            CapabilityAccess {
                effect: CapabilityEffect::ReadOnly,
                scope: CapabilityScope::Global,
            },
            Arc::new(RedactingPayloadSanitizer),
        )
    }

    fn result(
        names: &[&str],
        availability_notices: Vec<CapabilityAvailabilityNotice>,
    ) -> CapabilityCatalogResult {
        let mut builder = CapabilityCatalogBuilder::new();
        for name in names {
            builder.add(binding(name)).expect("unique test binding");
        }
        CapabilityCatalogResult {
            snapshot: builder.build(),
            availability_notices,
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(future);
        match Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }

    fn source(result: CapabilityCatalogResult) -> CapabilityBindingSourceHandle {
        Arc::new(StaticSource { result: Ok(result) })
    }

    #[test]
    fn merges_snapshots_and_notices_in_configured_order() {
        let first_notice = CapabilityAvailabilityNotice {
            capability: Some(ToolName::new("first.hidden").expect("name")),
            status: CapabilityAvailabilityStatus::Unavailable,
        };
        let second_notice = CapabilityAvailabilityNotice {
            capability: None,
            status: CapabilityAvailabilityStatus::AuthenticationRequired,
        };
        let composite = CompositeCapabilityBindingSource::new([
            source(result(&["first.one"], vec![first_notice.clone()])),
            source(result(&["second.one"], vec![second_notice.clone()])),
        ]);

        let catalog = block_on(composite.catalog()).expect("catalog");
        let names = catalog
            .snapshot
            .iter()
            .map(|binding| binding.spec().name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["first.one", "second.one"]);
        assert_eq!(catalog.availability_notices, [first_notice, second_notice]);
    }

    #[test]
    fn duplicate_canonical_name_fails_closed() {
        let composite = CompositeCapabilityBindingSource::new([
            source(result(&["same.name"], Vec::new())),
            source(result(&["same.name"], Vec::new())),
        ]);

        assert_eq!(
            block_on(composite.catalog()).expect_err("duplicate must fail"),
            CapabilityBindingSourceError::Invalid
        );
    }
}
