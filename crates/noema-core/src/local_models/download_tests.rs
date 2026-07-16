use tokio_util::sync::CancellationToken;

use super::{LocalFileModelImport, LocalModelInstaller};
use noema_providers::{LocalModelBackend, LocalModelEventKind, LocalModelInstallationStatus};

#[tokio::test]
async fn local_file_import_is_verified_content_addressed_and_removable() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let source = home.path().join("user-model.gguf");
    tokio::fs::write(&source, b"GGUF small deterministic test payload")
        .await
        .expect("source model");
    let installer = LocalModelInstaller::new(store.clone(), paths.clone()).expect("installer");

    let installed = installer
        .import_local_file(
            LocalFileModelImport {
                name: "User model".to_string(),
                model_id: "user-model".to_string(),
                path: source,
                license: None,
                backend: LocalModelBackend::Cpu,
            },
            CancellationToken::new(),
        )
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
        store
            .list_local_model_events(None, 100)
            .await
            .expect("events")
            .last()
            .expect("removal event")
            .kind,
        LocalModelEventKind::Removed
    );
}

#[tokio::test]
async fn verified_import_replaces_corrupt_existing_content_addressed_blob() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let installer = LocalModelInstaller::new(store, paths.clone()).expect("installer");
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
            LocalFileModelImport {
                name: "First".to_string(),
                model_id: "first".to_string(),
                path: first_source,
                license: None,
                backend: LocalModelBackend::Cpu,
            },
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

    installer
        .import_local_file(
            LocalFileModelImport {
                name: "Second".to_string(),
                model_id: "second".to_string(),
                path: second_source,
                license: None,
                backend: LocalModelBackend::Cpu,
            },
            CancellationToken::new(),
        )
        .await
        .expect("second import");

    assert_eq!(tokio::fs::read(blob).await.expect("repaired blob"), payload);
}

#[tokio::test]
async fn local_file_import_rejects_a_non_gguf_payload() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let source = home.path().join("not-a-model.gguf");
    tokio::fs::write(&source, b"this is not a model")
        .await
        .expect("source");
    let installer = LocalModelInstaller::new(store, paths).expect("installer");

    let error = installer
        .import_local_file(
            LocalFileModelImport {
                name: "Invalid".to_string(),
                model_id: "invalid".to_string(),
                path: source,
                license: None,
                backend: LocalModelBackend::Cpu,
            },
            CancellationToken::new(),
        )
        .await
        .expect_err("non-GGUF payload must be rejected");

    assert!(error.to_string().contains("does not have a GGUF header"));
}
