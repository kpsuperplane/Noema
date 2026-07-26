use super::*;
use serde_json::json;

fn store() -> (tempfile::TempDir, CapabilityAuthArgumentStore) {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    std::fs::create_dir_all(paths.root()).expect("root");
    (home, CapabilityAuthArgumentStore::new(paths))
}

#[test]
fn exact_arguments_round_trip_behind_an_opaque_reference() {
    let (_home, store) = store();
    let arguments = json!({"private": "marker-do-not-store-in-sqlite"});
    let protected = store.persist(&arguments).expect("persist");

    assert_eq!(
        store
            .load(&protected.reference, &protected.sha256)
            .expect("load"),
        arguments
    );
    assert!(!protected.reference.contains("marker"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let directory_mode = std::fs::metadata(store.paths.capability_auth_arguments_dir())
            .expect("directory metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = std::fs::metadata(
            store
                .paths
                .capability_auth_arguments_dir()
                .join(format!("{}.json", protected.reference)),
        )
        .expect("file metadata")
        .permissions()
        .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }
    store.remove(&protected.reference).expect("remove");
    assert!(store.load(&protected.reference, &protected.sha256).is_err());
}

#[test]
fn digest_mismatch_and_unreferenced_cleanup_fail_closed() {
    let (_home, store) = store();
    let retained = store.persist(&json!({"id": 1})).expect("retained");
    let removed = store.persist(&json!({"id": 2})).expect("removed");

    assert!(store.load(&retained.reference, &"0".repeat(64)).is_err());
    store
        .remove_unreferenced(&HashSet::from([retained.reference.clone()]))
        .expect("cleanup");
    assert!(store.load(&removed.reference, &removed.sha256).is_err());
    assert!(store.load(&retained.reference, &retained.sha256).is_ok());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let home = tempfile::tempdir().expect("symlink home");
        let outside = tempfile::tempdir().expect("outside");
        let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
        std::fs::create_dir_all(paths.root()).expect("root");
        symlink(outside.path(), paths.root().join("run")).expect("run symlink");
        let redirected = CapabilityAuthArgumentStore::new(paths);
        assert!(redirected.persist(&json!({"private": true})).is_err());
        assert_eq!(
            std::fs::read_dir(outside.path())
                .expect("outside entries")
                .count(),
            0
        );
    }
}
