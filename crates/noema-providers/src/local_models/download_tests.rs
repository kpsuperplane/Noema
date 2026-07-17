mod fake;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::{LocalModelBackend, LocalModelEventKind, LocalModelInstallationStatus};

use super::{LocalFileModelImport, LocalModelInstallError, LocalModelInstaller};
use fake::FakeInstallationPersistence;

fn installer(
    paths: &noema_home::NoemaPaths,
) -> (LocalModelInstaller, Arc<FakeInstallationPersistence>) {
    let persistence = Arc::new(FakeInstallationPersistence::default());
    let installer =
        LocalModelInstaller::new(persistence.clone(), paths.clone()).expect("installer");
    (installer, persistence)
}

fn local_import(name: &str, path: std::path::PathBuf) -> LocalFileModelImport {
    LocalFileModelImport {
        name: name.to_string(),
        model_id: name.to_ascii_lowercase().replace(' ', "-"),
        path,
        license: None,
        backend: LocalModelBackend::Cpu,
    }
}

#[tokio::test]
async fn local_file_import_is_verified_content_addressed_and_removable() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let source = home.path().join("user-model.gguf");
    tokio::fs::write(&source, b"GGUF small deterministic test payload")
        .await
        .expect("source model");
    let (installer, persistence) = installer(&paths);

    let installed = installer
        .import_local_file(local_import("User model", source), CancellationToken::new())
        .await
        .expect("import");

    assert_eq!(installed.status, LocalModelInstallationStatus::Installed);
    let digest = installed.sha256.as_deref().expect("verified digest");
    let blob = paths.local_model_blob_path(digest).expect("blob path");
    assert_eq!(
        tokio::fs::read(&blob).await.expect("installed blob"),
        b"GGUF small deterministic test payload"
    );

    let removed = installer
        .remove(&installed.installation_id)
        .await
        .expect("remove");
    assert!(removed.unreferenced_blob_relative_path.is_some());
    assert!(!blob.exists());
    assert_eq!(
        persistence.events().last().expect("removal event").kind,
        LocalModelEventKind::Removed
    );
}

#[tokio::test]
async fn verified_import_preserves_a_corrupt_shared_blob_and_fails_closed() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let (installer, _) = installer(&paths);
    let payload = b"GGUF same verified payload";
    let first_source = home.path().join("first.gguf");
    let second_source = home.path().join("second.gguf");
    tokio::fs::write(&first_source, payload)
        .await
        .expect("first source");
    tokio::fs::write(&second_source, payload)
        .await
        .expect("second source");

    let first = installer
        .import_local_file(
            local_import("First", first_source),
            CancellationToken::new(),
        )
        .await
        .expect("first import");
    let blob = paths
        .local_model_blob_path(first.sha256.as_deref().expect("digest"))
        .expect("blob");
    tokio::fs::write(&blob, b"corrupt")
        .await
        .expect("corrupt blob");

    let error = installer
        .import_local_file(
            local_import("Second", second_source),
            CancellationToken::new(),
        )
        .await
        .expect_err("occupied corrupt blob must fail closed");

    assert!(matches!(
        error,
        LocalModelInstallError::BlobDigestConflict { .. }
    ));
    assert_eq!(
        tokio::fs::read(blob).await.expect("preserved corrupt blob"),
        b"corrupt"
    );
}

#[tokio::test]
async fn verified_import_repairs_an_unreferenced_corrupt_blob() {
    use ring::digest::{SHA256, digest};

    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    tokio::fs::create_dir_all(paths.local_model_blobs_dir())
        .await
        .expect("blob directory");
    let (installer, _) = installer(&paths);
    let payload = b"GGUF orphan recovery payload";
    let source = home.path().join("recovery.gguf");
    tokio::fs::write(&source, payload)
        .await
        .expect("source model");
    let sha256 = digest(&SHA256, payload)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let blob = paths.local_model_blob_path(&sha256).expect("blob path");
    tokio::fs::write(&blob, b"orphaned corrupt bytes")
        .await
        .expect("corrupt blob");

    let installed = installer
        .import_local_file(local_import("Recovered", source), CancellationToken::new())
        .await
        .expect("unreferenced blob can be repaired");

    assert_eq!(installed.status, LocalModelInstallationStatus::Installed);
    assert_eq!(tokio::fs::read(blob).await.expect("repaired blob"), payload);
}

#[tokio::test]
async fn local_file_import_rejects_a_non_gguf_payload() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let source = home.path().join("not-a-model.gguf");
    tokio::fs::write(&source, b"this is not a model")
        .await
        .expect("source");
    let (installer, _) = installer(&paths);

    let error = installer
        .import_local_file(local_import("Invalid", source), CancellationToken::new())
        .await
        .expect_err("non-GGUF payload must be rejected");

    assert!(error.to_string().contains("does not have a GGUF header"));
}

#[tokio::test]
async fn cancelled_local_import_persists_terminal_state_and_can_retry() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let source = home.path().join("retry.gguf");
    tokio::fs::write(&source, b"GGUF retry payload")
        .await
        .expect("source");
    let (installer, persistence) = installer(&paths);
    let input = local_import("Retry", source);
    let installation_id = LocalModelInstaller::local_file_installation_id(&input)
        .await
        .expect("installation id");
    let cancelled = CancellationToken::new();
    cancelled.cancel();

    let error = installer
        .import_local_file(input.clone(), cancelled)
        .await
        .expect_err("cancelled import");
    assert!(matches!(error, LocalModelInstallError::Cancelled));
    assert_eq!(
        persistence
            .installation(&installation_id)
            .expect("cancelled row")
            .status,
        LocalModelInstallationStatus::Cancelled
    );

    let installed = installer
        .import_local_file(input, CancellationToken::new())
        .await
        .expect("retry import");
    assert_eq!(installed.status, LocalModelInstallationStatus::Installed);
    assert_eq!(installed.error_code, None);
    assert_eq!(installed.error_message, None);
}

#[tokio::test]
async fn large_local_import_persists_incremental_progress_before_verification() {
    const PAYLOAD_BYTES: usize = 9 * 1024 * 1024;

    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let source = home.path().join("large.gguf");
    let mut payload = vec![0_u8; PAYLOAD_BYTES];
    payload[..4].copy_from_slice(b"GGUF");
    tokio::fs::write(&source, payload)
        .await
        .expect("large source");
    let (installer, persistence) = installer(&paths);

    let installed = installer
        .import_local_file(local_import("Large", source), CancellationToken::new())
        .await
        .expect("large import");
    let events = persistence.events();

    assert!(events.iter().any(|event| {
        event.kind == LocalModelEventKind::Progress
            && event
                .downloaded_bytes
                .is_some_and(|bytes| bytes >= 8 * 1024 * 1024)
    }));
    assert!(events.iter().any(|event| {
        event.kind == LocalModelEventKind::Verifying
            && event.downloaded_bytes == installed.expected_bytes
    }));
}
