use std::path::PathBuf;

use super::*;

#[cfg(unix)]
#[tokio::test]
async fn bridge_start_reports_foundation_unavailable_health() {
    let (_dir, bridge_path) = bridge_script(&scripted_bridge("", false));

    let error = FoundationBridgeProcess::start(FoundationBridgeConfig {
        bridge_path,
        build: None,
    })
    .await
    .expect_err("unavailable bridge should fail health");

    assert_eq!(error.code(), "foundation_unavailable");
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_generate_returns_session_output_and_deltas() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"bridge "}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;
"#,
    )
    .await;
    let mut deltas = Vec::new();

    let session_id = process
        .create_session(
            "conversation:test".to_string(),
            "default".to_string(),
            Some("be concise".to_string()),
        )
        .await
        .expect("session should be created");
    let text = process
        .generate_in_session(session_id, "hello".to_string(), None, None, &mut |delta| {
            deltas.push(delta);
        })
        .await
        .expect("generate should complete");

    assert_eq!(text, "bridge answer");
    assert_eq!(deltas, vec!["bridge ".to_string()]);
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_generate_waits_longer_than_control_timeout() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) sleep 6; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"slow bridge answer"}}' ;;
"#,
    )
    .await;

    let session_id = process
        .create_session("conversation:test".to_string(), "default".to_string(), None)
        .await
        .expect("session should be created");
    let text = process
        .generate_in_session(session_id, "hello".to_string(), None, None, &mut |_| {})
        .await
        .expect("generate should wait beyond control timeout");

    assert_eq!(text, "slow bridge answer");
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_count_tokens_waits_longer_than_control_timeout() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"count_tokens"'*) sleep 6; printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}' ;;
"#,
    )
    .await;

    let tokens = process
        .count_tokens(Some("instructions".to_string()), "hello".to_string())
        .await
        .expect("count_tokens should wait beyond control timeout");

    assert_eq!(tokens, 42);
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_ignores_stale_response_ids_before_matching_response() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}'; printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
"#,
    )
    .await;

    let session_id = process
        .create_session(
            "conversation:test".to_string(),
            "default".to_string(),
            Some("be concise".to_string()),
        )
        .await
        .expect("create_session should skip stale responses");

    assert_eq!(session_id, "session-1");
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_replay_turns_sends_replay_request() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"replay_turns"'*'"role":"user"'*'"text":"hello"'*) printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;
"#,
    )
    .await;

    process
        .replay_turns(
            "session-1".to_string(),
            vec![BridgeReplayTurn {
                role: BridgeRole::User,
                text: "hello".to_string(),
            }],
        )
        .await
        .expect("replay should complete");
}

#[cfg(unix)]
#[tokio::test]
async fn bridge_cancel_request_accepts_cancel_complete() {
    let (_dir, mut process) = healthy_bridge(
        r#"
    *'"id":"cancel"'*'"request_id":"generate"'*) printf '%s\n' '{"id":"cancel","payload":{"type":"cancel_complete"}}' ;;
"#,
    )
    .await;

    process
        .cancel_request("generate".to_string())
        .await
        .expect("cancel should complete");
}

#[cfg(unix)]
#[tokio::test]
async fn missing_bridge_can_be_materialized_before_launch() {
    let package_dir = tempfile::tempdir().expect("package tempdir");
    let swift = fake_swift_builder(
        package_dir.path(),
        r#"#!/bin/sh
mkdir -p .build/debug
cat > .build/debug/noema-foundation-bridge <<'BRIDGE'
#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":2}}' ;;
    *'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}' ;;
    *) printf '%s\n' '{"id":"unknown","payload":{"type":"error","code":"unsupported_request","message":"Unsupported request."}}' ;;
  esac
done
BRIDGE
chmod +x .build/debug/noema-foundation-bridge
"#,
    );
    let bridge_path = package_dir
        .path()
        .join(".build/debug/noema-foundation-bridge");

    let process = FoundationBridgeProcess::start(FoundationBridgeConfig {
        bridge_path,
        build: Some(FoundationBridgeBuildConfig {
            package_path: package_dir.path().to_path_buf(),
            swift_executable: swift,
        }),
    })
    .await
    .expect("bridge should be materialized and launched");

    drop(process);
}

#[cfg(unix)]
#[tokio::test]
async fn failed_materialization_reports_build_error() {
    let package_dir = tempfile::tempdir().expect("package tempdir");
    let swift = fake_swift_builder(
        package_dir.path(),
        r#"#!/bin/sh
printf '%s\n' 'missing BuildServerProtocol.framework' >&2
exit 42
"#,
    );
    let bridge_path = package_dir
        .path()
        .join(".build/debug/noema-foundation-bridge");

    let error = FoundationBridgeProcess::start(FoundationBridgeConfig {
        bridge_path,
        build: Some(FoundationBridgeBuildConfig {
            package_path: package_dir.path().to_path_buf(),
            swift_executable: swift,
        }),
    })
    .await
    .expect_err("build failure should be reported");

    assert_eq!(error.code(), "bridge_build_failed");
    assert!(error.to_string().contains("BuildServerProtocol.framework"));
}

#[cfg(unix)]
fn bridge_script(contents: &str) -> (tempfile::TempDir, PathBuf) {
    use std::{fs, os::unix::fs::PermissionsExt};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bridge");
    fs::write(&path, contents).expect("script write");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    (dir, path)
}

#[cfg(unix)]
async fn healthy_bridge(extra_cases: &str) -> (tempfile::TempDir, FoundationBridgeProcess) {
    let (dir, bridge_path) = bridge_script(&scripted_bridge(extra_cases, true));
    let process = FoundationBridgeProcess::start(FoundationBridgeConfig {
        bridge_path,
        build: None,
    })
    .await
    .expect("bridge should start");
    (dir, process)
}

#[cfg(unix)]
fn scripted_bridge(extra_cases: &str, available: bool) -> String {
    let health = if available {
        r#"{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":null}}"#
    } else {
        r#"{"id":"health","payload":{"type":"health","available":false,"profiles":[{"id":"default","label":"Default"}],"unavailable_reason":"Foundation Models runtime is unavailable."}}"#
    };
    format!(
        r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":2}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{health}' ;;
    {extra_cases}
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
    )
}

#[cfg(unix)]
fn fake_swift_builder(package_dir: &std::path::Path, contents: &str) -> PathBuf {
    use std::{fs, os::unix::fs::PermissionsExt};

    let path = package_dir.join("swift");
    fs::write(&path, contents).expect("fake swift write");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    path
}
