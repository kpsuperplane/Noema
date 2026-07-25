use std::{path::PathBuf, sync::Arc};

pub(crate) fn artifact_operations_for_environment(
    store: &noema_store::NoemaStore,
    environment: &super::TestEnvironment,
) -> Result<noema_artifacts::ArtifactOperationsHandle, String> {
    let metadata: noema_artifacts::ArtifactMetadataStoreHandle = Arc::new(store.clone());
    noema_artifacts::LocalArtifactService::new(environment.root(), metadata)
        .map(|service| Arc::new(service) as noema_artifacts::ArtifactOperationsHandle)
        .map_err(|error| error.to_string())
}

pub(crate) fn artifact_diagnostics_for_environment(
    environment: &super::TestEnvironment,
) -> noema_host::ArtifactDiagnosticHandle {
    Arc::new(TestArtifactDiagnostics {
        path: environment.errors_log_path(),
    })
}

#[derive(Debug)]
struct TestArtifactDiagnostics {
    path: PathBuf,
}

impl noema_host::ArtifactDiagnosticOperations for TestArtifactDiagnostics {
    fn record_download_failure(&self, operation: &'static str) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let event = serde_json::json!({
            "category": "artifact_download_failure",
            "message": "artifact download operation failed",
            "context": { "operation": operation },
        });
        let Ok(mut line) = serde_json::to_vec(&event) else {
            return;
        };
        line.push(b'\n');
        use std::io::Write as _;
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(&line));
    }
}
