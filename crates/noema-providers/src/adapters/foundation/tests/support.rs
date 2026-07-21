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

pub(super) fn healthy_bridge_script(extra_cases: &str) -> String {
    format!(
        r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":2}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{{"id":"health","payload":{{"type":"health","available":true,"profiles":[{{"id":"default","label":"Default"}}],"unavailable_reason":null}}}}' ;;
    {extra_cases}
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
    )
}
