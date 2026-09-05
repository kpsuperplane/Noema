package provider

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestFoundationAvailabilityAndTokenCount(t *testing.T) {
	t.Setenv("NOEMA_HOME", "private-home")
	t.Setenv("NOEMA_OPENAI__API_KEY", "private-key")
	t.Setenv("NOEMA_TEST_ORDINARY", "ordinary-value")
	generator, record := foundationTestGenerator(t)
	if err := generator.CheckAvailability(context.Background()); err != nil {
		t.Fatal(err)
	}
	instructions := "Use local context."
	tokens, err := generator.CountTokens(context.Background(), &instructions, "Count this.")
	if err != nil || tokens != 17 {
		t.Fatalf("token count = %d, %v", tokens, err)
	}
	persistence := generator.accounts.persistence.(*memoryAccountPersistence)
	account := persistence.accounts[foundationAccountID]
	account.Status = StatusUnavailable
	persistence.accounts[foundationAccountID] = account
	account, err = generator.RefreshAccount(context.Background(), time.Now())
	if err != nil || account.Status != StatusAuthenticated {
		t.Fatalf("refreshed Foundation Models account = %#v, %v", account, err)
	}
	requests, err := os.ReadFile(record)
	if err != nil || !strings.Contains(string(requests), `"type":"count_tokens"`) ||
		!strings.Contains(string(requests), `"instructions":"Use local context."`) {
		t.Fatalf("bridge requests = %q, %v", requests, err)
	}
	if !strings.Contains(string(requests), "environment:::ordinary-value") {
		t.Fatalf("bridge environment was not filtered: %q", requests)
	}
	unsupported := newFoundationGenerator(generator.accounts, generator.bridgePath, false)
	if err := unsupported.CheckAvailability(context.Background()); FoundationErrorCode(err) != "unsupported_platform" {
		t.Fatalf("unsupported availability = %v", err)
	}
	packagePath := t.TempDir()
	expectedBridge := filepath.Join(packagePath, ".build", "debug", "noema-foundation-bridge")
	if err := os.MkdirAll(filepath.Dir(expectedBridge), 0o700); err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	swift := filepath.Join(bin, "swift")
	buildRecord := filepath.Join(packagePath, "swift-build")
	buildScript := "#!/bin/sh\nprintf build > " + buildRecord + "\ncp " + generator.bridgePath + " " + expectedBridge + "\n"
	if err := os.WriteFile(swift, []byte(buildScript), 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	development := newFoundationGenerator(generator.accounts, expectedBridge, true)
	development.developmentPackage = packagePath
	if err := development.CheckAvailability(context.Background()); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(buildRecord); err != nil {
		t.Fatalf("development bridge was not built: %v", err)
	}
}

func TestFoundationSessionReplaysToolsAndContinues(t *testing.T) {
	generator, record := foundationTestGenerator(t)
	session := generator.OpenGenerationSession()
	defer session.Close()
	maxTokens := uint32(128)
	request := GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:one",
		Messages: []GenerationMessage{
			{Role: "developer", Content: "Trusted context."},
			{Role: "user", Content: "Search memory."},
		},
		MaxOutputTokens: &maxTokens, ToolTransport: ToolTransportNative,
		Tools: []GenerationTool{{
			Name: "memory.search", Description: "Search memory.",
			InputSchema: []byte(`{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}`),
		}},
	}
	var deltas []string
	first, err := session.Generate(context.Background(), request, func(event StreamEvent) {
		deltas = append(deltas, event.Delta)
	})
	if err != nil {
		t.Fatal(err)
	}
	if first.Text != "Checking." || strings.Join(deltas, "") != "Checking." || len(first.ToolCalls) != 1 ||
		first.ToolCalls[0].Name != "memory.search" || first.ToolCalls[0].ProviderName != "search" {
		t.Fatalf("first generation = %#v, deltas = %#v", first, deltas)
	}
	result := &ReplayToolResult{
		ProviderCallID: "call-1", Name: "memory.search", ProviderName: "search",
		Success: true, Payload: []byte(`{"matches":["one"]}`),
	}
	second, err := session.Generate(context.Background(), GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:one",
		Messages: []GenerationMessage{{Role: "tool", ToolResult: result}},
		ReplayMessages: append(request.Messages,
			GenerationMessage{Role: "assistant", ToolCalls: []ReplayToolCall{{
				ProviderCallID: "call-1", Name: "memory.search", ProviderName: "search",
				Arguments: []byte(`{"query":"one"}`),
			}}},
			GenerationMessage{Role: "tool", ToolResult: result},
		),
		Tools: request.Tools, ToolTransport: ToolTransportNative,
	}, nil)
	if err != nil || second.Text != "Found one." || second.FinishReason != "stop" {
		t.Fatalf("continued generation = %#v, %v", second, err)
	}
	if err := session.Close(); err != nil {
		t.Fatal(err)
	}
	fullReplay := append(append([]GenerationMessage{}, request.Messages...),
		GenerationMessage{Role: "assistant", ToolCalls: []ReplayToolCall{{
			ProviderCallID: "call-1", Name: "memory.search", ProviderName: "search",
			Arguments: []byte(`{"query":"one"}`),
		}}},
		GenerationMessage{Role: "tool", ToolResult: result},
	)
	restarted := generator.OpenGenerationSession()
	_, err = restarted.Generate(context.Background(), GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:one",
		Messages: fullReplay, Tools: request.Tools, ToolTransport: ToolTransportNative,
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	_ = restarted.Close()
	requests, err := os.ReadFile(record)
	if err != nil {
		t.Fatal(err)
	}
	wire := string(requests)
	for _, required := range []string{
		`"type":"create_session"`, `"name":"search"`,
		`"role":"application_context"`, `"input":"Search memory."`,
		`"type":"tool_result"`, `\"success\":true`, `"tool_call":{`,
		`"input":""`, `"max_output_tokens":512`, `"type":"close_session"`,
	} {
		if !strings.Contains(wire, required) {
			t.Fatalf("bridge wire lacks %q: %s", required, wire)
		}
	}
}

func TestFoundationRejectsOversizedRequest(t *testing.T) {
	generator, _ := foundationTestGenerator(t)
	_, err := generator.Generate(context.Background(), GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:context",
		Messages: []GenerationMessage{{Role: "user", Content: "context-overflow"}},
	}, nil)
	if !errors.Is(err, ErrGenerationRequestTooLarge) {
		t.Fatalf("context overflow error = %v", err)
	}
	_, err = generator.Generate(context.Background(), GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:large",
		Messages: []GenerationMessage{{Role: "user", Content: strings.Repeat("x", foundationMessageLimit)}},
	}, nil)
	if !errors.Is(err, ErrGenerationRequestTooLarge) {
		t.Fatalf("oversized request error = %v", err)
	}
}

func TestFoundationCancellationStopsBridgeTree(t *testing.T) {
	generator, _ := foundationTestGenerator(t)
	blocking := `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"type":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
    *'"type":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true}}' ;;
    *'"type":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"type":"generate"'*) sleep 30 ;;
  esac
done
`
	if err := os.WriteFile(generator.bridgePath, []byte(blocking), 0o700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 25*time.Millisecond)
	defer cancel()
	started := time.Now()
	_, err := generator.Generate(ctx, GenerateRequest{
		AccountID: foundationAccountID, Model: "default", ConversationID: "conversation:cancel",
		Messages: []GenerationMessage{{Role: "user", Content: "Wait."}},
	}, nil)
	if !errors.Is(err, context.DeadlineExceeded) || time.Since(started) > 3*time.Second {
		t.Fatalf("cancelled generation = %v after %s", err, time.Since(started))
	}
}

func foundationTestGenerator(t *testing.T) (*FoundationGenerator, string) {
	t.Helper()
	directory := t.TempDir()
	record := filepath.Join(directory, "requests.jsonl")
	bridge := filepath.Join(directory, "bridge")
	script := `#!/bin/sh
record="` + record + `"
printf 'environment:%s:%s:%s\n' "$NOEMA_HOME" "$NOEMA_OPENAI__API_KEY" "$NOEMA_TEST_ORDINARY" >> "$record"
generated=0
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$record"
  case "$line" in
    *'"type":"handshake"'*) printf '%s\n' '{"id":"handshake","payload":{"type":"handshake_ok","protocol_version":3}}' ;;
    *'"type":"health"'*) printf '%s\n' '{"id":"health","payload":{"type":"health","available":true}}' ;;
    *'context-overflow'*) printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":3600}}' ;;
    *'"type":"count_tokens"'*) printf '%s\n' '{"id":"count_tokens","payload":{"type":"token_count","tokens":17}}' ;;
    *'"type":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"type":"replay_turns"'*) printf '%s\n' '{"id":"replay_turns","payload":{"type":"replay_complete"}}' ;;
    *'"type":"generate"'*)
      generated=1
      printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"Checking."}}'
      printf '%s\n' '{"id":"generate","payload":{"type":"tool_call","call_id":"call-1","tool_name":"search","arguments":"{\"query\":\"one\"}"}}'
      ;;
    *'"type":"tool_result"'*)
      printf '%s\n' '{"id":"tool_result:call-1","payload":{"type":"tool_result_accepted"}}'
      printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"Found one."}}'
      ;;
    *'"type":"close_session"'*) printf '%s\n' '{"id":"close_session","payload":{"type":"replay_complete"}}' ;;
  esac
done
`
	if err := os.WriteFile(bridge, []byte(script), 0o700); err != nil {
		t.Fatal(err)
	}
	persistence := &memoryAccountPersistence{accounts: map[string]Account{
		foundationAccountID: {
			ID: foundationAccountID, ProviderKind: "foundation_local", AccountKey: "default",
			DisplayName: "Apple Foundation Models", AuthMethod: AuthNone,
			IsActive: true, IsDefault: true, Status: StatusAuthenticated,
		},
	}}
	accounts, err := NewAccountService(directory, persistence)
	if err != nil {
		t.Fatal(err)
	}
	return newFoundationGenerator(accounts, bridge, true), record
}
