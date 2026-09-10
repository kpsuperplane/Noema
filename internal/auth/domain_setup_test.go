package auth

import (
	"bytes"
	"context"
	"net/http"
	"strings"
	"testing"
)

func TestDomainSetupPersistsAddressAndPreservesSettings(t *testing.T) {
	_, database, paths := newAuthTest(t, false)
	config, recovery, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil || !config.DomainSetupRequired {
		t.Fatalf("fresh configuration: %v", err)
	}
	document, _ := readConfigDocument(paths.Config())
	document["mcp"] = map[string]any{"stdio_enabled": true}
	if err := writeConfigDocument(paths.Config(), document); err != nil {
		t.Fatal(err)
	}
	code := readRecoveryCode(t, paths)
	ready := make(chan struct{})
	close(ready)
	handler, configured := DomainSetup(config, recovery, ready)
	request := authRequest(http.MethodPost, "/auth/domain", bytes.NewBufferString(`{"origin":"https://noema.example:8443"}`))
	request.Host = "noema.example:8443"
	request.Header.Set("Origin", "https://noema.example:8443")
	if response := serve(handler, request); response.Code != http.StatusNoContent {
		t.Fatalf("save = %d %s", response.Code, response.Body.String())
	}
	saved := <-configured
	reloaded, _, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil || reloaded.DomainSetupRequired || saved != reloaded || saved.RPID != "noema.example" || !saved.Secure {
		t.Fatalf("saved configuration differs: %v", err)
	}
	if readRecoveryCode(t, paths) != code {
		t.Fatal("recovery code changed")
	}
	document, _ = readConfigDocument(paths.Config())
	if document["mcp"].(map[string]any)["stdio_enabled"] != true {
		t.Fatal("unrelated setting changed")
	}
	server, err := New(paths, database, saved, recovery)
	if err != nil {
		t.Fatal(err)
	}
	request = authRequest(http.MethodPost, "/auth/passkey/register/start", nil)
	request.Host = saved.Authority
	request.Header.Set("Origin", saved.Origin)
	response := serve(server.Handler(http.NotFoundHandler()), request)
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"id":"noema.example"`) {
		t.Fatalf("passkey setup = %d %s", response.Code, response.Body.String())
	}
	if exists, err := database.HasPasskey(context.Background()); err != nil || exists {
		t.Fatal("address confirmation created a passkey")
	}
	request = authRequest(http.MethodGet, "/auth/status", nil)
	request.Host = "other.example"
	if response := serve(server.Handler(http.NotFoundHandler()), request); response.Code != http.StatusBadRequest {
		t.Fatal("configured host check was lost")
	}
}

func TestDomainSetupRejectsUnrelatedRequestsAndRepeatWrites(t *testing.T) {
	_, _, paths := newAuthTest(t, false)
	config, recovery, _ := LoadConfig(paths, "127.0.0.1:3737")
	ready := make(chan struct{})
	close(ready)
	handler, configured := DomainSetup(config, recovery, ready)
	for _, test := range []struct{ method, path, host, origin, body string }{
		{"POST", "/graphql", "noema.example", "https://noema.example", `{}`},
		{"GET", "/artifacts/versions/a", "noema.example", "", ""},
		{"POST", "/auth/passkey/register/start", "noema.example", "https://noema.example", `{}`},
		{"POST", "/auth/domain", "noema.example", "https://evil.example", `{"origin":"https://noema.example"}`},
		{"POST", "/auth/domain", "other.example", "https://noema.example", `{"origin":"https://noema.example"}`},
		{"POST", "/auth/domain", "noema.example", "http://noema.example", `{"origin":"http://noema.example"}`},
		{"POST", "/auth/domain", "127.0.0.1", "https://127.0.0.1", `{"origin":"https://127.0.0.1"}`},
	} {
		request := authRequest(test.method, test.path, bytes.NewBufferString(test.body))
		request.Host = test.host
		request.Header.Set("Origin", test.origin)
		if response := serve(handler, request); response.Code < 400 {
			t.Fatalf("accepted %s %s", test.method, test.path)
		}
	}
	select {
	case <-configured:
		t.Fatal("rejected request configured the server")
	default:
	}
	for index := range 2 {
		request := authRequest(http.MethodPost, "/auth/domain", bytes.NewBufferString(`{"origin":"http://localhost:3737"}`))
		response := serve(handler, request)
		want := http.StatusNoContent
		if index == 1 {
			want = http.StatusConflict
		}
		if response.Code != want {
			t.Fatalf("write %d = %d", index, response.Code)
		}
	}
	t.Setenv("NOEMA_WEB__PUBLIC_ORIGIN", "http://localhost:9999")
	overridden, _, err := LoadConfig(paths, "127.0.0.1:3737")
	if err != nil || overridden.DomainSetupRequired || overridden.Origin != "http://localhost:9999" {
		t.Fatal("explicit environment origin was not retained")
	}
}
