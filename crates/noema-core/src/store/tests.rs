use tempfile::TempDir;

use super::{NoemaStore, StoreConfig};

#[tokio::test]
async fn opens_embedded_store_under_noema_db_dir() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("open store");

    assert!(paths.db_dir().exists());
    assert_eq!(store.schema_version().await.expect("schema version"), 1);
}

#[tokio::test]
async fn reopens_existing_embedded_store() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let first = NoemaStore::open(&config).await.expect("first open");
    assert_eq!(first.schema_version().await.expect("first version"), 1);
    drop(first);
    tokio::task::yield_now().await;

    let second = NoemaStore::open(&config).await.expect("second open");
    assert_eq!(second.schema_version().await.expect("second version"), 1);
}
