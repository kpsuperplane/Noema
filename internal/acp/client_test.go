package acp

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"testing"
	"time"
)

func TestProbePreservesAdvertisedMetadata(t *testing.T) {
	command, pidFile := helperCommand(t, "probe")
	result, err := Probe(context.Background(), command)
	if err != nil {
		t.Fatal(err)
	}
	if result.ProtocolVersion != 1 || stringValue(result.ImplementationName) != "fake-acp" ||
		stringValue(result.ImplementationVersion) != "1.2.3" {
		t.Fatalf("unexpected implementation: %#v", result)
	}
	written, err := json.Marshal(result.Capabilities)
	if err != nil {
		t.Fatal(err)
	}
	if got := string(written); got != `{"agent":{"loadSession":true,"vendor":{"level":7}},"authMethods":[{"description":"Open a browser","id":"browser","name":"Browser login","vendor":{"mode":"external"}}]}` {
		t.Fatalf("capabilities = %s", got)
	}
	assertProcessStopped(t, pidFile)
}

func TestProbeDefaultsMalformedOptionalMetadataAndFiltersAuthMethods(t *testing.T) {
	tests := []struct {
		name        string
		initialized initializeResult
		want        string
	}{
		{
			name: "wrong optional shapes",
			initialized: initializeResult{
				AgentCapabilities: json.RawMessage(`[]`),
				AuthMethods:       json.RawMessage(`{}`),
			},
			want: `{"agent":{},"authMethods":[]}`,
		},
		{
			name: "invalid authentication entries",
			initialized: initializeResult{
				AgentCapabilities: json.RawMessage(`{"loadSession":true}`),
				AuthMethods: json.RawMessage(`[
					null,
					"browser",
					{"id":"missing-name"},
					{"id":" ","name":"Blank"},
					{"id":"browser","name":"Browser login","vendor":{"mode":"external"}}
				]`),
			},
			want: `{"agent":{"loadSession":true},"authMethods":[{"id":"browser","name":"Browser login","vendor":{"mode":"external"}}]}`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			capabilities, err := json.Marshal(probeCapabilities(test.initialized))
			if err != nil {
				t.Fatal(err)
			}
			if string(capabilities) != test.want {
				t.Fatalf("capabilities = %s", capabilities)
			}
		})
	}
}

func TestAuthenticateUsesNamedMethodAndCallerContext(t *testing.T) {
	t.Run("success", func(t *testing.T) {
		command, pidFile := helperCommand(t, "authenticate")
		if err := Authenticate(context.Background(), command, "browser"); err != nil {
			t.Fatal(err)
		}
		assertProcessStopped(t, pidFile)
	})

	t.Run("cancel", func(t *testing.T) {
		command, pidFile := helperCommand(t, "hang-authenticate")
		ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
		defer cancel()
		err := Authenticate(ctx, command, "browser")
		if !errors.Is(err, context.DeadlineExceeded) {
			t.Fatalf("authentication error = %v", err)
		}
		assertProcessStopped(t, pidFile)
	})
}

func TestProbeRejectsProtocolFailuresAndTimesOut(t *testing.T) {
	for _, test := range []struct {
		name string
		mode string
		want error
	}{
		{name: "JSON-RPC error", mode: "rpc-error", want: ErrInitializationFailed},
		{name: "wrong protocol", mode: "wrong-version", want: ErrInitializationFailed},
		{name: "oversized response", mode: "oversize", want: ErrInitializationFailed},
		{name: "timeout", mode: "hang-probe", want: ErrInitializationTimedOut},
	} {
		t.Run(test.name, func(t *testing.T) {
			command, pidFile := helperCommand(t, test.mode)
			_, err := probeWithTimeout(context.Background(), command, 100*time.Millisecond)
			if !errors.Is(err, test.want) {
				t.Fatalf("probe error = %v, want %v", err, test.want)
			}
			assertProcessStopped(t, pidFile)
		})
	}
}

func TestACPHelperProcess(t *testing.T) {
	if len(os.Args) < 3 || os.Args[len(os.Args)-3] != "NOEMA_ACP_HELPER" {
		return
	}
	mode := os.Args[len(os.Args)-2]
	pidFile := os.Args[len(os.Args)-1]
	if err := os.WriteFile(pidFile, []byte(strconv.Itoa(os.Getpid())), 0o600); err != nil {
		t.Fatal(err)
	}
	reader := bufio.NewReader(os.Stdin)
	request := readHelperRequest(t, reader)
	if request.Method != "initialize" || request.Params.ProtocolVersion != 1 ||
		request.Params.ClientInfo.Name != "noema" {
		t.Fatalf("initialize request = %#v", request)
	}
	if mode == "hang-probe" {
		hang()
	}
	if mode == "oversize" {
		fmt.Println(strings.Repeat("x", messageLimit+1))
		hang()
	}
	if mode == "rpc-error" {
		writeHelperResponse(t, map[string]any{
			"jsonrpc": "2.0", "id": 1,
			"error": map[string]any{"code": -32000, "message": "private child diagnostic"},
		})
		hang()
	}
	version := 1
	if mode == "wrong-version" {
		version = 2
	}
	writeHelperResponse(t, map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"result": map[string]any{
			"protocolVersion": version,
			"agentCapabilities": map[string]any{
				"loadSession": true,
				"vendor":      map[string]any{"level": 7},
			},
			"authMethods": []any{map[string]any{
				"id": "browser", "name": "Browser login", "description": "Open a browser",
				"vendor": map[string]any{"mode": "external"},
			}},
			"agentInfo": map[string]any{"name": "fake-acp", "version": "1.2.3"},
		},
	})
	if mode == "authenticate" || mode == "hang-authenticate" {
		auth := readHelperRequest(t, reader)
		if auth.Method != "authenticate" || auth.MethodParams.MethodID != "browser" {
			t.Fatalf("authenticate request = %#v", auth)
		}
		if mode == "hang-authenticate" {
			hang()
		}
		writeHelperResponse(t, map[string]any{"jsonrpc": "2.0", "id": 2, "result": map[string]any{}})
	}
	hang()
}

type helperRequest struct {
	Method string `json:"method"`
	Params struct {
		ProtocolVersion int            `json:"protocolVersion"`
		ClientInfo      implementation `json:"clientInfo"`
	} `json:"params"`
	MethodParams struct {
		MethodID string `json:"methodId"`
	} `json:"-"`
}

func readHelperRequest(t *testing.T, reader *bufio.Reader) helperRequest {
	t.Helper()
	line, err := reader.ReadBytes('\n')
	if err != nil {
		t.Fatal(err)
	}
	var raw struct {
		Method string          `json:"method"`
		Params json.RawMessage `json:"params"`
	}
	if err := json.Unmarshal(line, &raw); err != nil {
		t.Fatal(err)
	}
	request := helperRequest{Method: raw.Method}
	if raw.Method == "initialize" {
		if err := json.Unmarshal(raw.Params, &request.Params); err != nil {
			t.Fatal(err)
		}
	} else if err := json.Unmarshal(raw.Params, &request.MethodParams); err != nil {
		t.Fatal(err)
	}
	return request
}

func writeHelperResponse(t *testing.T, response any) {
	t.Helper()
	if err := json.NewEncoder(os.Stdout).Encode(response); err != nil {
		t.Fatal(err)
	}
}

func helperCommand(t *testing.T, mode string) (Command, string) {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	pidFile := t.TempDir() + string(os.PathSeparator) + "pid"
	return Command{
		Path: executable,
		Args: []string{"-test.run=^TestACPHelperProcess$", "--", "NOEMA_ACP_HELPER", mode, pidFile},
	}, pidFile
}

func assertProcessStopped(t *testing.T, pidFile string) {
	t.Helper()
	data, err := os.ReadFile(pidFile)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(string(data))
	if err != nil {
		t.Fatal(err)
	}
	for range 100 {
		if !processAlive(pid) {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("ACP process %d is still running", pid)
}

func stringValue(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}

func hang() {
	for {
		time.Sleep(time.Hour)
	}
}
