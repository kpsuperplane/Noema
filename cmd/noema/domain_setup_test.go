package main

import (
	"context"
	"io"
	"net"
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
)

func TestFreshHostConfirmsDomainBeforePasskeySetup(t *testing.T) {
	t.Setenv(home.EnvironmentName, t.TempDir())
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	address := listener.Addr().String()
	listener.Close()
	ctx, cancel := context.WithCancel(context.Background())
	output := &rustHostReadyWriter{ready: make(chan struct{}), setupReady: make(chan struct{}), observed: make(chan auth.Config, 1)}
	result := make(chan error, 1)
	go func() { result <- run(ctx, address, output, nil) }()
	defer func() {
		cancel()
		select {
		case err := <-result:
			if err != nil {
				t.Error(err)
			}
		case <-time.After(10 * time.Second):
			t.Error("host did not stop")
		}
	}()
	select {
	case <-output.setupReady:
	case <-time.After(10 * time.Second):
		t.Fatal("domain setup did not start")
	}
	client := &http.Client{Timeout: 10 * time.Second}
	request := func(method, path, body string) (int, string) {
		t.Helper()
		req, _ := http.NewRequest(method, "http://"+address+path, strings.NewReader(body))
		req.Host = "noema.example"
		req.Header.Set("Origin", "https://noema.example")
		res, err := client.Do(req)
		if err != nil {
			t.Fatal(err)
		}
		defer res.Body.Close()
		data, _ := io.ReadAll(res.Body)
		return res.StatusCode, string(data)
	}
	if status, body := request("GET", "/auth/status", ""); status != 200 || !strings.Contains(body, "domain_setup_required") {
		t.Fatalf("initial status = %d %s", status, body)
	}
	if status, body := request("POST", "/auth/domain", `{"origin":"https://noema.example"}`); status != 204 {
		t.Fatalf("confirm = %d %s", status, body)
	}
	select {
	case <-output.ready:
	case <-time.After(10 * time.Second):
		t.Fatal("configured host did not start")
	}
	config := <-output.observed
	if config.Origin != "https://noema.example" || config.RPID != "noema.example" || config.DomainSetupRequired {
		t.Fatal("services received the wrong origin")
	}
	if status, body := request("GET", "/auth/status", ""); status != 200 || !strings.Contains(body, "setup_ready") {
		t.Fatalf("passkey status = %d %s", status, body)
	}
	if status, body := request("POST", "/auth/passkey/register/start", ""); status != 200 || !strings.Contains(body, `"id":"noema.example"`) {
		t.Fatalf("registration = %d %s", status, body)
	}
	if status, _ := request("POST", "/auth/domain", `{"origin":"https://noema.example"}`); status < 400 {
		t.Fatal("domain setup remained available")
	}
}
