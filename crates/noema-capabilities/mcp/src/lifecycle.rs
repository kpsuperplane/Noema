//! Shared shutdown admission and cancellation for the local MCP service.

use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicUsize, Ordering},
};
use std::time::Duration;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::McpOperationError;

const RUNNING: u8 = 0;
const SHUTTING_DOWN: u8 = 1;
const STOPPED: u8 = 2;

#[derive(Debug)]
pub(crate) struct McpServiceLifecycle {
    state: AtomicU8,
    active: AtomicUsize,
    cancellation: CancellationToken,
    drained: Notify,
}

impl Default for McpServiceLifecycle {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(RUNNING),
            active: AtomicUsize::new(0),
            cancellation: CancellationToken::new(),
            drained: Notify::new(),
        }
    }
}

impl McpServiceLifecycle {
    pub(crate) fn enter(self: &Arc<Self>) -> Result<McpServiceAdmission, McpOperationError> {
        if self.state.load(Ordering::Acquire) != RUNNING {
            return Err(McpOperationError::ShuttingDown);
        }
        self.active.fetch_add(1, Ordering::AcqRel);
        if self.state.load(Ordering::Acquire) != RUNNING {
            self.release();
            return Err(McpOperationError::ShuttingDown);
        }
        Ok(McpServiceAdmission {
            lifecycle: self.clone(),
        })
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub(crate) fn begin_shutdown(&self) {
        if self
            .state
            .compare_exchange(RUNNING, SHUTTING_DOWN, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.cancellation.cancel();
            if self.active.load(Ordering::Acquire) == 0 {
                self.drained.notify_one();
            }
        }
    }

    pub(crate) async fn drain(&self, timeout: Duration) -> bool {
        self.begin_shutdown();
        let drained = async {
            while self.active.load(Ordering::Acquire) != 0 {
                self.drained.notified().await;
            }
        };
        let completed = tokio::time::timeout(timeout, drained).await.is_ok();
        if completed {
            self.state.store(STOPPED, Ordering::Release);
        }
        completed
    }

    pub(crate) async fn run_admitted<T>(
        self: &Arc<Self>,
        future: impl Future<Output = T>,
    ) -> Result<T, McpOperationError> {
        let _admission = self.enter()?;
        tokio::select! {
            biased;
            () = self.cancellation.cancelled() => Err(McpOperationError::Cancelled),
            result = future => Ok(result),
        }
    }

    fn release(&self) {
        if self.active.fetch_sub(1, Ordering::AcqRel) == 1
            && self.state.load(Ordering::Acquire) != RUNNING
        {
            self.drained.notify_one();
        }
    }
}

#[derive(Debug)]
pub(crate) struct McpServiceAdmission {
    lifecycle: Arc<McpServiceLifecycle>,
}

impl Drop for McpServiceAdmission {
    fn drop(&mut self) {
        self.lifecycle.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_rejects_new_work_and_drains_existing_admission() {
        let lifecycle = Arc::new(McpServiceLifecycle::default());
        let admission = lifecycle.enter().expect("admission");
        lifecycle.begin_shutdown();
        assert_eq!(
            lifecycle.enter().expect_err("rejected"),
            McpOperationError::ShuttingDown
        );
        assert!(lifecycle.cancellation_token().is_cancelled());

        let draining = {
            let lifecycle = lifecycle.clone();
            tokio::spawn(async move { lifecycle.drain(Duration::from_secs(1)).await })
        };
        tokio::task::yield_now().await;
        assert!(!draining.is_finished());
        drop(admission);
        assert!(draining.await.expect("drain task"));
    }

    #[tokio::test]
    async fn shutdown_timeout_keeps_the_lifecycle_shutting_down() {
        let lifecycle = Arc::new(McpServiceLifecycle::default());
        let _admission = lifecycle.enter().expect("admission");
        assert!(!lifecycle.drain(Duration::from_millis(1)).await);
        assert_eq!(
            lifecycle.enter().expect_err("stopped"),
            McpOperationError::ShuttingDown
        );
    }
}
