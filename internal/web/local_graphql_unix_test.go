//go:build !windows

package web

import (
	"context"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestLocalSocketIsPrivateAndRemoved(t *testing.T) {
	assets := t.TempDir()
	for name, body := range map[string]string{"index.html": "<html>app</html>", "app.js": "// app"} {
		if err := os.WriteFile(filepath.Join(assets, name), []byte(body), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("NOEMA_DEV_ASSET_DIR", assets)
	path := filepath.Join(t.TempDir(), "run", "graphql.sock")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	active, err := net.Listen("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := NewLocalGraphQLServer(path, http.NotFoundHandler(), nil); err == nil {
		t.Fatal("active local GraphQL socket was replaced")
	}
	if err := active.Close(); err != nil {
		t.Fatal(err)
	}
	server, err := NewLocalGraphQLServer(path, http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = io.WriteString(w, "graphql")
	}), []byte("type Query { value: String }"))
	if err != nil {
		t.Fatal(err)
	}
	for target, want := range map[string]os.FileMode{filepath.Dir(path): 0o700, path: 0o600} {
		info, statErr := os.Stat(target)
		if statErr != nil {
			t.Fatal(statErr)
		}
		if info.Mode().Perm() != want {
			t.Fatalf("%s mode = %v; want %v", target, info.Mode().Perm(), want)
		}
	}
	ctx, cancel := context.WithCancel(context.Background())
	result := make(chan error, 1)
	go func() { result <- server.Serve(ctx) }()
	transport := &http.Transport{DialContext: func(context.Context, string, string) (net.Conn, error) {
		return net.Dial("unix", path)
	}}
	client := &http.Client{Transport: transport}
	defer transport.CloseIdleConnections()
	for _, target := range []string{"/graphql", "/auth/recovery", "/oauth/authorize", "/graphql/ws?query=%7B__typename%7D"} {
		request, _ := http.NewRequest(http.MethodGet, "http://local"+target, nil)
		response, requestErr := client.Do(request)
		if requestErr != nil || response.StatusCode != http.StatusNotFound {
			t.Fatalf("GET %s = %v, %v", target, response, requestErr)
		}
		_ = response.Body.Close()
	}
	for target, want := range map[string]string{
		"/graphql/schema.graphql": "type Query { value: String }",
		"/auth/status":            `{"state":"authenticated"}`,
		"/graphql/ws":             "graphql",
		"/tasks":                  "<html>app</html>",
		"/assets/app.js":          "// app",
	} {
		request, _ := http.NewRequest(http.MethodGet, "http://local"+target, nil)
		if target == "/graphql/ws" {
			request.Header.Set("Upgrade", "websocket")
		}
		response, requestErr := client.Do(request)
		if requestErr != nil {
			t.Fatal(requestErr)
		}
		body, _ := io.ReadAll(response.Body)
		_ = response.Body.Close()
		if response.StatusCode != http.StatusOK || string(body) != want {
			t.Fatalf("GET %s = %d %q", target, response.StatusCode, body)
		}
	}
	for _, target := range []string{"/auth/status", "/tasks", "/graphql/ws"} {
		response, requestErr := client.Post("http://local"+target, "application/json", strings.NewReader("{}"))
		if requestErr != nil {
			t.Fatal(requestErr)
		}
		_ = response.Body.Close()
		if response.StatusCode != http.StatusNotFound {
			t.Fatalf("POST %s = %d", target, response.StatusCode)
		}
	}
	response, err := client.Post("http://local/graphql", "application/json", strings.NewReader("{}"))
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(response.Body)
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK || string(body) != "graphql" ||
		response.Header.Get("Cache-Control") != "no-store" {
		t.Fatalf("POST response = %d %q %#v", response.StatusCode, body, response.Header)
	}
	oversized, _ := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(strings.Repeat("x", localGraphQLBodyLimit+1)))
	response, err = client.Do(oversized)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusRequestEntityTooLarge {
		t.Fatalf("oversized POST status = %d", response.StatusCode)
	}
	cancel()
	select {
	case err := <-result:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("local GraphQL server did not stop")
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("socket remained after shutdown: %v", err)
	}
}
