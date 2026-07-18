use std::{fs, os::unix::fs::symlink};

use noema_home::NoemaPaths;

use super::{
    FilesystemMcpSecretStore, McpSecretMaterial, McpSecretStore, McpSecretStoreOperation,
    SECRET_FILE, STAGING_DIR,
};

const SERVER_ID: &str = "mcp:test";

fn material() -> McpSecretMaterial {
    crate::test_fixture::secret_material("inside-secret")
}

#[test]
fn filesystem_secret_store_symlink_and_root_identity_contracts() {
    // Case: symlinked_secret_root_components_are_rejected_before_write.
    for component in ["root", "mcp", "staging"] {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("create outside");
        let root = if component == "root" {
            let root = temp.path().join("noema");
            symlink(&outside, &root).expect("link root");
            root
        } else {
            let root = temp.path().to_path_buf();
            let mcp = root.join("mcp");
            if component == "mcp" {
                symlink(&outside, mcp).expect("link mcp directory");
            } else {
                fs::create_dir(&mcp).expect("create mcp");
                symlink(&outside, mcp.join(STAGING_DIR)).expect("link staging directory");
            }
            root
        };
        let store =
            FilesystemMcpSecretStore::new(NoemaPaths::from_noema_home(root).expect("paths"));

        let error = store.stage(&material()).expect_err(component);

        assert_eq!(error.operation, McpSecretStoreOperation::Stage);
        assert!(
            fs::read_dir(&outside)
                .expect("outside entries")
                .next()
                .is_none()
        );
    }
    // Case: symlinked_server_directory_is_rejected_without_reading_outside_secret.
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
    let outside = temp.path().join("outside");
    fs::create_dir_all(paths.mcp_dir()).expect("create mcp");
    fs::create_dir(&outside).expect("create outside");
    fs::write(outside.join(SECRET_FILE), b"outside-secret").expect("write outside");
    symlink(&outside, paths.mcp_server_home(SERVER_ID)).expect("link server directory");
    let store = FilesystemMcpSecretStore::new(paths);

    let error = store.load(SERVER_ID).expect_err("reject linked server dir");

    assert_eq!(error.operation, McpSecretStoreOperation::Read);
    assert_eq!(
        fs::read(outside.join(SECRET_FILE)).expect("read outside"),
        b"outside-secret"
    );
    // Case: symlinked_secret_file_is_rejected_without_replacing_outside_target.
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NoemaPaths::from_noema_home(temp.path()).expect("paths");
    let server = paths.mcp_server_home(SERVER_ID);
    let outside = temp.path().join("outside-secret");
    fs::create_dir_all(&server).expect("create server");
    fs::write(&outside, b"outside-secret").expect("write outside");
    symlink(&outside, server.join(SECRET_FILE)).expect("link secret file");
    let store = FilesystemMcpSecretStore::new(paths);
    let stage = store.stage(&material()).expect("stage inside root");

    let error = store
        .commit(stage, SERVER_ID)
        .expect_err("reject linked target");

    assert_eq!(error.operation, McpSecretStoreOperation::Commit);
    assert_eq!(fs::read(&outside).expect("read outside"), b"outside-secret");
    // Case: root_swap_after_staging_is_rejected_before_commit.
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("noema");
    let displaced = temp.path().join("displaced");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).expect("create root");
    fs::create_dir(&outside).expect("create outside");
    let paths = NoemaPaths::from_noema_home(&root).expect("paths");
    let store = FilesystemMcpSecretStore::new(paths);
    let stage = store.stage(&material()).expect("stage");
    fs::rename(&root, &displaced).expect("displace root");
    symlink(&outside, &root).expect("replace root with link");

    let error = store
        .commit(stage, SERVER_ID)
        .expect_err("reject swapped root");

    assert_eq!(error.operation, McpSecretStoreOperation::Commit);
    assert!(
        fs::read_dir(&outside)
            .expect("outside entries")
            .next()
            .is_none()
    );
}
