use std::{net::Ipv4Addr, net::SocketAddr};

use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

use super::{DaemonError, WebState, serve_daemon_web};

pub(crate) struct TestDaemonWebServer {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), DaemonError>>>,
}

impl TestDaemonWebServer {
    pub(crate) async fn start(
        graphql_state: crate::graphql::GraphqlState,
    ) -> Result<Self, DaemonError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let local_addr = listener.local_addr()?;
        let web_state = WebState::new(graphql_state);
        let (shutdown, shutdown_received) = oneshot::channel();
        let task = tokio::spawn(serve_daemon_web(listener, web_state, async move {
            shutdown_received.await.map_err(|_| {
                DaemonError::Protocol("test daemon web shutdown sender dropped".to_string())
            })
        }));

        Ok(Self {
            local_addr,
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://{}", self.local_addr)
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), DaemonError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .take()
            .expect("test daemon web task")
            .await
            .map_err(|source| DaemonError::Protocol(source.to_string()))?
    }
}

impl Drop for TestDaemonWebServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
