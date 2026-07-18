use noema_capabilities::{CapabilityBindingSource, CapabilityError};

use crate::{McpOperationError, McpOperations};

use super::test_support::TestHarness;

#[tokio::test]
async fn shutdown_cancels_admitted_transport_work_and_rejects_new_work() {
    let harness = TestHarness::new();
    let invoking = harness.start_blocked_invocation().await;

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
