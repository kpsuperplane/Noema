use tokio_util::sync::CancellationToken;

use super::{
    LocalFileModelImport, LocalModelBackend, LocalModelEventKind, LocalModelInstallationStatus,
    LocalModelInstaller,
};

#[tokio::test]
async fn local_file_import_is_verified_content_addressed_and_removable() {
    let home = tempfile::tempdir().expect("Noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    let source = home.path().join("user-model.gguf");
    tokio::fs::write(&source, b"small deterministic GGUF test payload")
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
        b"small deterministic GGUF test payload"
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
