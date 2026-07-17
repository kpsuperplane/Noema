use std::future::Future;

use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[derive(Debug, Default)]
pub(super) struct RuntimeTaskGroup {
    cancellation: CancellationToken,
    tasks: TaskTracker,
}

impl RuntimeTaskGroup {
    pub(super) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub(super) fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let cancellation = self.cancellation.clone();
        drop(self.tasks.spawn(async move {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => {}
                () = future => {}
            }
        }));
    }

    pub(super) async fn shutdown(&self) {
        self.cancellation.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }
}
