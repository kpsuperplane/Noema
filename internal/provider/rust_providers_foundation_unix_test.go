//go:build aix || darwin || dragonfly || freebsd || illumos || ios || linux || netbsd || openbsd || solaris

package provider

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_start_reports_foundation_unavailable_health (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeStartReportsFoundationUnavailableHealth(t *testing.T) {
	path := parityFoundationBridge(t, "", false)
	_, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	var bridgeErr foundationBridgeError
	if !errors.As(err, &bridgeErr) || bridgeErr.Code != "foundation_unavailable" {
		t.Fatalf("unavailable bridge error = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_generate_returns_session_output_and_deltas (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeGenerateReturnsSessionOutputAndDeltas(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"bridge "}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	session, err := process.createSession("conversation:test", "default", "be concise", nil, "empty")
	if err != nil {
		t.Fatal(err)
	}
	var deltas []string
	generation, err := process.generateInSession(session, "hello", func(delta string) { deltas = append(deltas, delta) })
	if err != nil || generation.Text != "bridge answer" || !reflect.DeepEqual(deltas, []string{"bridge "}) {
		t.Fatalf("bridge generation = %#v/%v, deltas=%#v", generation, err, deltas)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_native_tool_call_round_trip_continues_same_session (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeNativeToolCallRoundTripContinuesSameSession(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-1","tool_name":"search_memory","arguments":"{\"query\":\"trains\"}"}}' ;;
*'"type":"tool_result"'*) printf '%s\n' '{"id":"tool_result:call-1","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"continued answer"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	session, err := process.createSession("conversation:test", "default", "", []foundationToolDefinition{{Name: "search_memory", Description: "Search memory.", Parameters: `{"type":"object"}`}}, "catalog-1")
	if err != nil {
		t.Fatal(err)
	}
	first, err := process.generateInSession(session, "search", nil)
	if err != nil || len(first.ToolCalls) != 1 || first.ToolCalls[0].CallID != "call-1" {
		t.Fatalf("first tool call = %#v/%v", first, err)
	}
	continued, err := process.continueGeneration(session, []foundationToolResult{{CallID: "call-1", Output: `{"matches":[]}`}}, nil)
	if err != nil || continued.Text != "continued answer" || len(continued.ToolCalls) != 0 {
		t.Fatalf("continued tool call = %#v/%v", continued, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_rejects_unknown_missing_and_stale_tool_results_immediately (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeRejectsUnknownMissingAndStaleToolResultsImmediately(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-1","tool_name":"search_memory","arguments":"{}"}}' ;;
*'"type":"tool_result"'*) printf '%s\n' '{"id":"tool_result:call-1","payload":{"type":"tool_result_accepted"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"continued answer"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	session, err := process.createSession("conversation:test", "default", "", nil, "empty")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := process.generateInSession(session, "search", nil); err != nil {
		t.Fatal(err)
	}
	for _, testCase := range []struct {
		results []foundationToolResult
		want    string
	}{{[]foundationToolResult{{CallID: "stale-call"}}, "unknown Foundation tool result"}, {nil, "no Foundation tool results"}} {
		if _, err := process.continueGeneration(session, testCase.results, nil); err == nil || !strings.Contains(err.Error(), testCase.want) {
			t.Fatalf("tool result %v = %v", testCase.results, err)
		}
	}
	if _, err := process.continueGeneration(session, []foundationToolResult{{CallID: "call-1", Output: `{}`}}, nil); err != nil {
		t.Fatal(err)
	}
	if _, err := process.continueGeneration(session, []foundationToolResult{{CallID: "call-1", Output: `{}`}}, nil); err == nil || !strings.Contains(err.Error(), "without a pending generation") {
		t.Fatalf("stale tool result = %v", err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_generate_waits_longer_than_control_timeout (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeGenerateWaitsLongerThanControlTimeout(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
*'"id":"generate"'*) sleep 6; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"slow bridge answer"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	session, err := process.createSession("conversation:test", "default", "", nil, "empty")
	if err != nil {
		t.Fatal(err)
	}
	generation, err := process.generateInSession(session, "hello", nil)
	if err != nil || generation.Text != "slow bridge answer" {
		t.Fatalf("slow generation = %#v/%v", generation, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_count_tokens_waits_longer_than_control_timeout (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeCountTokensWaitsLongerThanControlTimeout(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"count_tokens"'*) sleep 6; printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	if tokens, err := process.countTokens("instructions", "hello"); err != nil || tokens != 42 {
		t.Fatalf("slow token count = %d/%v", tokens, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_ignores_stale_response_ids_before_matching_response (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeIgnoresStaleResponseIdsBeforeMatchingResponse(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"create_session"'*) printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":42}}'; printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	session, err := process.createSession("conversation:test", "default", "be concise", nil, "empty")
	if err != nil || session != "session-1" {
		t.Fatalf("stale response session = %q/%v", session, err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_replay_turns_sends_replay_request (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeReplayTurnsSendsReplayRequest(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"replay_turns"'*'"role":"user"'*'"text":"hello"'*) printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	if err := process.replayTurns("session-1", []foundationReplayTurn{{Role: "user", Text: "hello"}}); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::bridge_cancel_request_accepts_cancel_complete (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_BridgeCancelRequestAcceptsCancelComplete(t *testing.T) {
	path := parityFoundationBridge(t, `*'"id":"cancel"'*'"request_id":"generate"'*) printf '%s\n' '{"id":"cancel","payload":{"type":"cancel_complete"}}' ;;`, true)
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: path})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(process.close)
	if err := process.cancelRequest("generate"); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::missing_bridge_can_be_materialized_before_launch (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_MissingBridgeCanBeMaterializedBeforeLaunch(t *testing.T) {
	packageDir := t.TempDir()
	swift := filepath.Join(packageDir, "swift")
	bridgePath := filepath.Join(packageDir, ".build/debug/noema-foundation-bridge")
	swiftScript := `#!/bin/sh
mkdir -p .build/debug
cat > .build/debug/noema-foundation-bridge <<'BRIDGE'
#!/bin/sh
while IFS= read -r line; do
case "$line" in
*'"id":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
*'"id":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true,"profiles":[{"id":"default"}]}}' ;;
esac
done
BRIDGE
chmod +x .build/debug/noema-foundation-bridge
`
	if err := os.WriteFile(swift, []byte(swiftScript), 0o755); err != nil {
		t.Fatal(err)
	}
	process, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: bridgePath, Build: &foundationBridgeBuild{PackagePath: packageDir, SwiftExecutable: swift}})
	if err != nil {
		t.Fatal(err)
	}
	process.close()
}

// Rust source: crates/noema-providers/src/adapters/foundation/bridge/tests.rs::failed_materialization_reports_build_error (baseline a007a4fa984f0d2eaeb2c101337dbbe7881d9379).
func TestRustProviders_FailedMaterializationReportsBuildError(t *testing.T) {
	packageDir := t.TempDir()
	swift := filepath.Join(packageDir, "swift")
	if err := os.WriteFile(swift, []byte("#!/bin/sh\necho 'missing BuildServerProtocol.framework' >&2\nexit 42\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	_, err := providerStartFoundationBridge(t.Context(), foundationBridgeConfig{BridgePath: filepath.Join(packageDir, ".build/debug/noema-foundation-bridge"), Build: &foundationBridgeBuild{PackagePath: packageDir, SwiftExecutable: swift}})
	var bridgeErr foundationBridgeError
	if !errors.As(err, &bridgeErr) || bridgeErr.Code != "bridge_build_failed" || !strings.Contains(bridgeErr.Detail, "BuildServerProtocol.framework") {
		t.Fatalf("failed materialization = %v", err)
	}
}
