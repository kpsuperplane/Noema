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

// Rust source: crates/noema-server/src/web/local_graphql/tests.rs::socket_serves_local_authority_with_private_permissions_and_cleanup.
func TestRustServer_socket_serves_local_authority_with_private_permissions_and_cleanup(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "run", "graphql.sock")
	graphql := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = io.WriteString(w, "Noema store is unavailable")
	})
	server, err := NewLocalGraphQLServer(path, graphql, []byte("type QueryRoot"))
	if err != nil {
		t.Fatal(err)
	}
	parent, err := os.Stat(filepath.Dir(path))
	if err != nil {
		t.Fatal(err)
	}
	if got := parent.Mode().Perm(); got != 0o700 {
		t.Fatalf("socket parent mode = %o, want 700", got)
	}
	socket, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := socket.Mode().Perm(); got != 0o600 {
		t.Fatalf("socket mode = %o, want 600", got)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	result := make(chan error, 1)
	go func() { result <- server.Serve(ctx) }()
	transport := &http.Transport{DialContext: func(context.Context, string, string) (net.Conn, error) {
		return net.Dial("unix", path)
	}}
	client := &http.Client{Transport: transport}
	defer transport.CloseIdleConnections()
	request, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(`{"query":"{ task { taskId } }"}`))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	body, err := io.ReadAll(response.Body)
	_ = response.Body.Close()
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK || !strings.Contains(string(body), "Noema store is unavailable") {
		t.Fatalf("local GraphQL response = %d %q", response.StatusCode, body)
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

// Rust source: crates/noema-server/src/web/local_graphql/tests.rs::router_has_local_authority_and_exposes_only_bounded_graphql_post.
func TestRustServer_router_has_local_authority_and_exposes_only_bounded_graphql_post(t *testing.T) {
	path := filepath.Join(t.TempDir(), "run", "graphql.sock")
	server, err := NewLocalGraphQLServer(path, http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = io.WriteString(w, "graphql")
	}), []byte("type QueryRoot"))
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	result := make(chan error, 1)
	go func() { result <- server.Serve(ctx) }()
	transport := &http.Transport{DialContext: func(context.Context, string, string) (net.Conn, error) {
		return net.Dial("unix", path)
	}}
	client := &http.Client{Transport: transport}
	defer transport.CloseIdleConnections()
	for _, target := range []string{"/graphql", "/auth/recovery"} {
		response, err := client.Get("http://local" + target)
		if err != nil {
			t.Fatal(err)
		}
		_ = response.Body.Close()
		if response.StatusCode != http.StatusNotFound {
			t.Fatalf("GET %s = %d", target, response.StatusCode)
		}
	}
	request, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(`{"query":"{ task { taskId } }"}`))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK || response.Header.Get("Cache-Control") != "no-store" {
		t.Fatalf("GraphQL POST = %d %#v", response.StatusCode, response.Header)
	}
	oversized, err := http.NewRequest(http.MethodPost, "http://local/graphql", strings.NewReader(strings.Repeat("x", localGraphQLBodyLimit+1)))
	if err != nil {
		t.Fatal(err)
	}
	response, err = client.Do(oversized)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusRequestEntityTooLarge {
		t.Fatalf("oversized GraphQL POST = %d", response.StatusCode)
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
}
