//go:build unix

package web

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

const localGraphQLBodyLimit = 64 * 1024

// LocalGraphQLServer serves GraphQL and the web app through the private local socket.
type LocalGraphQLServer struct {
	listener net.Listener
	server   *http.Server
	path     string
}

// NewLocalGraphQLServer binds a private Unix socket before public HTTP starts.
func NewLocalGraphQLServer(path string, graphql http.Handler, schema []byte) (*LocalGraphQLServer, error) {
	parent := filepath.Dir(path)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return nil, fmt.Errorf("create local GraphQL directory: %w", err)
	}
	if err := os.Chmod(parent, 0o700); err != nil {
		return nil, fmt.Errorf("protect local GraphQL directory: %w", err)
	}
	if info, err := os.Lstat(path); err == nil {
		if info.Mode()&os.ModeSocket == 0 {
			return nil, errors.New("local GraphQL path exists and is not a socket")
		}
		connection, dialErr := net.DialTimeout("unix", path, 100*time.Millisecond)
		if dialErr == nil {
			_ = connection.Close()
			return nil, errors.New("local GraphQL socket is already active")
		}
		if !errors.Is(dialErr, syscall.ECONNREFUSED) && !errors.Is(dialErr, os.ErrNotExist) {
			return nil, fmt.Errorf("inspect local GraphQL socket: %w", dialErr)
		}
		if err := os.Remove(path); err != nil {
			return nil, fmt.Errorf("remove stale local GraphQL socket: %w", err)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, fmt.Errorf("inspect local GraphQL path: %w", err)
	}
	listener, err := net.Listen("unix", path)
	if err != nil {
		return nil, fmt.Errorf("listen for local GraphQL: %w", err)
	}
	if err := os.Chmod(path, 0o600); err != nil {
		_ = listener.Close()
		_ = os.Remove(path)
		return nil, fmt.Errorf("protect local GraphQL socket: %w", err)
	}
	assets := NewAssetHandler()
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		if r.Method == http.MethodGet {
			switch r.URL.Path {
			case "/graphql/schema.graphql":
				w.Header().Set("Content-Type", "text/plain; charset=utf-8")
				_, _ = w.Write(schema)
			case "/auth/status":
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`{"state":"authenticated"}`))
			case "/graphql/ws":
				if !strings.EqualFold(r.Header.Get("Upgrade"), "websocket") {
					http.NotFound(w, r)
					return
				}
				graphql.ServeHTTP(w, r)
			default:
				assets.ServeHTTP(w, r)
			}
			return
		}
		if r.Method != http.MethodPost || r.URL.Path != "/graphql" {
			http.NotFound(w, r)
			return
		}
		if r.ContentLength > localGraphQLBodyLimit {
			http.Error(w, "request body too large", http.StatusRequestEntityTooLarge)
			return
		}
		r.Body = http.MaxBytesReader(w, r.Body, localGraphQLBodyLimit)
		graphql.ServeHTTP(w, r)
	})
	return &LocalGraphQLServer{
		listener: listener,
		server: &http.Server{
			Handler:           handler,
			ReadHeaderTimeout: 10 * time.Second,
			ReadTimeout:       30 * time.Second,
		},
		path: path,
	}, nil
}

// Serve runs until the context ends or the socket server fails.
func (s *LocalGraphQLServer) Serve(ctx context.Context) error {
	defer os.Remove(s.path)
	result := make(chan error, 1)
	go func() { result <- s.server.Serve(s.listener) }()
	select {
	case err := <-result:
		if errors.Is(err, http.ErrServerClosed) {
			return nil
		}
		return err
	case <-ctx.Done():
		shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := s.server.Shutdown(shutdown); err != nil {
			return err
		}
		<-result
		return nil
	}
}
