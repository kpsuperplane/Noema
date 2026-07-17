use noema_capabilities::{CapabilityBindingSource, CapabilityError, CapabilityInvoker};

use crate::{McpOperationError, McpOperations};

use super::test_support::{TestHarness, advertised_invocation};

#[tokio::test]
async fn shutdown_cancels_admitted_transport_work_and_rejects_new_work() {
    let harness = TestHarness::new();
    harness.sessions.block_calls();
    let invocation = advertised_invocation(&harness).await;
    let service = harness.service.clone();
    let invoking =
        tokio::spawn(async move { CapabilityInvoker::invoke(&service, invocation).await });
    harness.sessions.wait_for_call().await;

    assert!(harness.service.shutdown().await);
    assert_eq!(
        invoking.await.expect("invoke task"),
        Err(CapabilityError::Unavailable)
    );
    assert_eq!(
        McpOperations::list_servers(&harness.service).await,
        Err(McpOperationError::ShuttingDown)
    );
    assert!(
        CapabilityBindingSource::catalog(&harness.service)
            .await
            .is_err()
    );
}
