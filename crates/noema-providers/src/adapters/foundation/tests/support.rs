use std::path::PathBuf;

pub(super) fn bridge_script(contents: &str) -> (tempfile::TempDir, PathBuf) {
    use std::{fs, os::unix::fs::PermissionsExt};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bridge");
    fs::write(&path, contents).expect("script write");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    (dir, path)
}
